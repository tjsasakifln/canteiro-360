//! Issue #17: o grafo persistido não pode ser truncado pela listagem visual.
use super::*;
use crate::auth::AuthStore;

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn composition_graph_includes_entries_beyond_the_list_limit() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let scope = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO canteiro_auth.users(id,email,password_hash,verified)
         VALUES($1,$2,'fixture-only',true)",
    )
    .bind(actor)
    .bind(format!("{actor}@example.invalid"))
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash)
         VALUES($1,'Empresa de teste',$2,$3)",
    )
    .bind(tenant)
    .bind(actor)
    .bind([17_u8; 32].as_slice())
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organization_members(tenant_id,user_id,role)
         VALUES($1,$2,'administrator')",
    )
    .bind(tenant)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by)
         VALUES($1,$2,'work','Escopo de teste',NULL,$3)",
    )
    .bind(tenant)
    .bind(scope)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();

    let mut seed = fixture.admin.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO canteiro.cost_compositions(tenant_id,scope_id,code,current_version)
         SELECT $1,$2,lpad(n::TEXT,4,'0'),1 FROM generate_series(1,501) n",
    )
    .bind(tenant)
    .bind(scope)
    .execute(&mut *seed)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.cost_composition_versions(
             tenant_id,scope_id,code,version,payload,policy,actor_id,
             operation_key,fingerprint,reason)
         SELECT $1,$2,lpad(n::TEXT,4,'0'),1,'{}'::JSONB,'C360-DECIMAL-1',$3,
             gen_random_uuid(),$4,'Teste de grafo' FROM generate_series(1,501) n",
    )
    .bind(tenant)
    .bind(scope)
    .bind(actor)
    .bind([17_u8; 32].as_slice())
    .execute(&mut *seed)
    .await
    .unwrap();
    seed.commit().await.unwrap();

    let mut tx = AuthStore::new(database.clone()).begin().await.unwrap();
    tx.org_set_actor(actor).await.unwrap();
    tx.org_set_tenant(tenant).await.unwrap();
    tx.composition_lock_scope(tenant, scope).await.unwrap();
    assert_eq!(
        tx.composition_list(tenant, &[scope]).await.unwrap().len(),
        500
    );
    let graph = tx.composition_graph(tenant, scope).await.unwrap();
    assert_eq!(graph.len(), 501);
    assert_eq!(graph.last().unwrap().code, "0501");
    assert!(
        tx.composition_graph(tenant, Uuid::new_v4())
            .await
            .unwrap()
            .is_empty()
    );
    let mut other = fixture.admin.begin().await.unwrap();
    let acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(
            hashtextextended('composition:' || $1::UUID::TEXT || ':' || $2::UUID::TEXT, 0)
         )",
    )
    .bind(tenant)
    .bind(scope)
    .fetch_one(&mut *other)
    .await
    .unwrap();
    assert!(!acquired);
    tx.commit().await.unwrap();
    let acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(
            hashtextextended('composition:' || $1::UUID::TEXT || ':' || $2::UUID::TEXT, 0)
         )",
    )
    .bind(tenant)
    .bind(scope)
    .fetch_one(&mut *other)
    .await
    .unwrap();
    assert!(acquired);
    other.rollback().await.unwrap();
}
