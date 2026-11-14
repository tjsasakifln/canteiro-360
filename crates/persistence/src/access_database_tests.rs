//! Casos independentes das rotas: privilégios SQL reais, isolamento e concorrência.
use super::*;
use crate::auth::{AuthStore, AuthTransaction};

const TABLES: [(&str, i64); 7] = [
    ("access_scopes", 2),
    ("access_grants", 1),
    ("access_requirements", 1),
    ("access_policies", 1),
    ("access_requests", 1),
    ("access_request_versions", 1),
    ("access_approvals", 1),
];

struct AccessGraph {
    tenant: Uuid,
    owner: Uuid,
    work: Uuid,
    contract: Uuid,
    grant: Uuid,
    request: Uuid,
}

async fn person(fixture: &Fixture, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro_auth.users(id,email,password_hash,verified) VALUES($1,$2,'fixture-only',true)")
        .bind(id).bind(format!("{label}-{id}@example.invalid"))
        .execute(&fixture.admin).await.unwrap();
    id
}

async fn organization(fixture: &Fixture, owner: Uuid, label: &str) -> Uuid {
    let tenant = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash) VALUES($1,$2,$3,$4)")
        .bind(tenant).bind(format!("Empresa fictícia {label}"))
        .bind(owner).bind(vec![7_u8;32]).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.organization_members(tenant_id,user_id,role) VALUES($1,$2,'administrator')")
        .bind(tenant).bind(owner).execute(&fixture.admin).await.unwrap();
    tenant
}

async fn add_member(fixture: &Fixture, tenant: Uuid, user: Uuid) {
    sqlx::query(
        "INSERT INTO canteiro.organization_members(tenant_id,user_id,role) VALUES($1,$2,'member')",
    )
    .bind(tenant)
    .bind(user)
    .execute(&fixture.admin)
    .await
    .unwrap();
}

async fn graph(fixture: &Fixture, label: &str, ids: Option<[Uuid; 4]>) -> AccessGraph {
    let owner = person(fixture, label).await;
    let tenant = organization(fixture, owner, label).await;
    let [work, contract, grant, request] =
        ids.unwrap_or_else(|| std::array::from_fn(|_| Uuid::new_v4()));
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,created_by) VALUES($1,$2,'work',$3,$4)")
        .bind(tenant).bind(work).bind(format!("Obra {label}"))
        .bind(owner).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES($1,$2,'contract',$3,$4,$5)")
        .bind(tenant).bind(contract).bind(format!("Contrato {label}"))
        .bind(work).bind(owner).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'approver',ARRAY['view','approve_change'],1000.00,clock_timestamp()-interval '1 hour',$4)")
        .bind(tenant).bind(grant).bind(contract).bind(owner).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES($1,$2,'approver','approve_change',100.00)")
        .bind(tenant).bind(contract).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_policies(tenant_id,scope_id,action,threshold,approvals_above_threshold,segregate_proposer) VALUES($1,$2,'approve_change',100.00,2,false)")
        .bind(tenant).bind(contract).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_requests(tenant_id,id,scope_id,created_by,valid_approvals) VALUES($1,$2,$3,$4,1)")
        .bind(tenant).bind(request).bind(contract).bind(owner).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_request_versions(tenant_id,request_id,version,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer) VALUES($1,$2,1,'Alteração fictícia','approve_change',150.00,0.00,1,100.00,2,false)")
        .bind(tenant).bind(request).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES($1,$2,1,$3,$4,1,true)")
        .bind(tenant).bind(request).bind(owner).bind(grant).execute(&fixture.admin).await.unwrap();
    AccessGraph {
        tenant,
        owner,
        work,
        contract,
        grant,
        request,
    }
}

async fn scoped(store: &AuthStore, tenant: Uuid, user: Uuid) -> AuthTransaction {
    let mut tx = store.begin().await.unwrap();
    tx.org_set_actor(user).await.unwrap();
    tx.org_set_tenant(tenant).await.unwrap();
    tx
}

async fn rejected_sql(tx: &mut AuthTransaction, sql: String, state: &str) {
    sqlx::raw_sql("SAVEPOINT denied_statement")
        .execute(&mut *tx.tx)
        .await
        .unwrap();
    // Somente identificadores da lista fixa e UUIDs gerados no teste compõem estas consultas.
    let error = sqlx::raw_sql(sqlx::AssertSqlSafe(sql.clone()))
        .execute(&mut *tx.tx)
        .await
        .unwrap_err();
    assert_eq!(sql_state(&error), state, "{sql}");
    sqlx::raw_sql("ROLLBACK TO SAVEPOINT denied_statement")
        .execute(&mut *tx.tx)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_seven_tables_enforce_rls_even_with_repeated_ids_and_reused_connection() {
    let fixture = Fixture::ready().await;
    let ids = std::array::from_fn(|_| Uuid::new_v4());
    let a = graph(&fixture, "Alfa", Some(ids)).await;
    let b = graph(&fixture, "Beta", Some(ids)).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let pid = assert_clean_pool(&database).await;
    for (current, other) in [(&a, &b), (&b, &a)] {
        let mut tx = scoped(&store, current.tenant, current.owner).await;
        for (table, expected) in TABLES {
            let sql = format!("SELECT count(*) FROM canteiro.{table}");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(&mut *tx.tx)
                .await
                .unwrap();
            assert_eq!(
                count, expected,
                "RLS deve proteger leitura sem filtro: {table}"
            );
            let sql = format!("SELECT count(*) FROM canteiro.{table} WHERE tenant_id=$1");
            let foreign: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .bind(other.tenant)
                .fetch_one(&mut *tx.tx)
                .await
                .unwrap();
            assert_eq!(
                foreign, 0,
                "contagem estrangeira deve permanecer invisível: {table}"
            );
        }
        for (table, change) in [
            ("access_grants", "revoked_at=clock_timestamp()"),
            ("access_policies", "threshold=0"),
            ("access_requests", "status='approved'"),
        ] {
            let sql = format!("UPDATE canteiro.{table} SET {change} WHERE tenant_id=$1");
            let changed = sqlx::query(sqlx::AssertSqlSafe(sql))
                .bind(other.tenant)
                .execute(&mut *tx.tx)
                .await
                .unwrap();
            assert_eq!(
                changed.rows_affected(),
                0,
                "nenhum registro estrangeiro alterado: {table}"
            );
        }
        for sql in [
            "UPDATE canteiro.access_scopes SET name='Alteração não permitida'",
            "UPDATE canteiro.access_requirements SET role='viewer'",
        ] {
            rejected_sql(&mut tx, sql.to_string(), "42501").await;
        }
        let tenant = other.tenant;
        let owner = other.owner;
        let contract = other.contract;
        let grant = other.grant;
        let request = other.request;
        let inserts = [
            format!(
                "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,created_by) VALUES('{tenant}','{}','work','Invasão fictícia','{owner}')",
                Uuid::new_v4()
            ),
            format!(
                "INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES('{tenant}','{}','{contract}','{owner}','viewer',ARRAY['view'],0,clock_timestamp(),'{owner}')",
                Uuid::new_v4()
            ),
            format!(
                "INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES('{tenant}','{contract}','viewer','view',0)"
            ),
            format!(
                "INSERT INTO canteiro.access_policies(tenant_id,scope_id,action,threshold,approvals_above_threshold,segregate_proposer) VALUES('{tenant}','{contract}','approve_budget',0,1,true)"
            ),
            format!(
                "INSERT INTO canteiro.access_requests(tenant_id,id,scope_id,created_by) VALUES('{tenant}','{}','{contract}','{owner}')",
                Uuid::new_v4()
            ),
            format!(
                "INSERT INTO canteiro.access_request_versions(tenant_id,request_id,version,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer) VALUES('{tenant}','{request}',2,'Invasão fictícia','approve_change',1,0,1,100,2,false)"
            ),
            format!(
                "INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES('{tenant}','{request}',1,'{owner}','{grant}',1,true)"
            ),
        ];
        for sql in inserts {
            rejected_sql(&mut tx, sql, "42501").await;
        }
        tx.commit().await.unwrap();
        assert_eq!(
            pid,
            assert_clean_pool(&database).await,
            "a mesma conexão não pode herdar empresa"
        );
        let mut connection = database.pool.acquire().await.unwrap();
        for (table, _) in TABLES {
            let sql = format!("SELECT count(*) FROM canteiro.{table}");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(count, 0, "sem contexto não há dados: {table}");
        }
    }
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_composite_foreign_keys_reject_cross_tenant_references() {
    let fixture = Fixture::ready().await;
    let a = graph(&fixture, "Alfa", None).await;
    let b = graph(&fixture, "Beta", None).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut tx = scoped(&store, a.tenant, a.owner).await;
    let tenant = a.tenant;
    let owner = a.owner;
    let contract = a.contract;
    let request = a.request;
    let new_id = Uuid::new_v4();
    let attempts = [
        format!(
            "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES('{tenant}','{new_id}','contract','Contrato fictício','{}','{owner}')",
            b.work
        ),
        format!(
            "INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES('{tenant}','{new_id}','{}','{owner}','viewer',ARRAY['view'],0,clock_timestamp(),'{owner}')",
            b.contract
        ),
        format!(
            "INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES('{tenant}','{new_id}','{contract}','{}','viewer',ARRAY['view'],0,clock_timestamp(),'{owner}')",
            b.owner
        ),
        format!(
            "INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,valid_until,source_grant_id,created_by,reason) VALUES('{tenant}','{new_id}','{contract}','{owner}','viewer',ARRAY['view'],0,clock_timestamp(),clock_timestamp()+interval '1 day','{}','{owner}','Cobertura temporária')",
            b.grant
        ),
        format!(
            "INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES('{tenant}','{}','viewer','view',0)",
            b.contract
        ),
        format!(
            "INSERT INTO canteiro.access_policies(tenant_id,scope_id,action,threshold,approvals_above_threshold,segregate_proposer) VALUES('{tenant}','{}','approve_budget',0,1,true)",
            b.contract
        ),
        format!(
            "INSERT INTO canteiro.access_requests(tenant_id,id,scope_id,created_by) VALUES('{tenant}','{new_id}','{}','{owner}')",
            b.contract
        ),
        format!(
            "INSERT INTO canteiro.access_requests(tenant_id,id,scope_id,created_by) VALUES('{tenant}','{new_id}','{contract}','{}')",
            b.owner
        ),
        format!(
            "INSERT INTO canteiro.access_request_versions(tenant_id,request_id,version,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer) VALUES('{tenant}','{}',1,'Alteração fictícia','approve_change',1,0,1,100,2,false)",
            b.request
        ),
        format!(
            "INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES('{tenant}','{request}',1,'{owner}','{}',1,true)",
            b.grant
        ),
        format!(
            "INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES('{tenant}','{}',1,'{owner}','{}',1,true)",
            b.request, a.grant
        ),
    ];
    // A decisão adicional usa outra versão para não colidir com a decisão já existente.
    sqlx::query("INSERT INTO canteiro.access_request_versions SELECT tenant_id,request_id,2,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer,conflicted_users,created_at FROM canteiro.access_request_versions WHERE tenant_id=$1 AND request_id=$2 AND version=1")
        .bind(tenant).bind(request).execute(&mut *tx.tx).await.unwrap();
    for (index, sql) in attempts.into_iter().enumerate() {
        let sql = if sql.starts_with("INSERT INTO canteiro.access_approvals")
            && sql.contains(&format!("'{request}',1"))
        {
            sql.replace(&format!("'{request}',1"), &format!("'{request}',2"))
        } else {
            sql
        };
        // A hierarquia é verificada antes da FK e usa erro de regra, também sem aceitar o pai estrangeiro.
        rejected_sql(&mut tx, sql, if index == 0 { "23514" } else { "23503" }).await;
    }
    rejected_sql(&mut tx, format!("INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES('{tenant}','{request}',2,'{}','{}',1,true)",b.owner,a.grant), "23503").await;
    drop(tx);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_request_history_is_insert_only_for_runtime() {
    let fixture = Fixture::ready().await;
    let a = graph(&fixture, "Histórico", None).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut tx = scoped(&store, a.tenant, a.owner).await;
    for sql in [
        "UPDATE canteiro.access_request_versions SET increase=0",
        "DELETE FROM canteiro.access_request_versions",
        "UPDATE canteiro.access_approvals SET approved=false",
        "DELETE FROM canteiro.access_approvals",
    ] {
        rejected_sql(&mut tx, sql.to_string(), "42501").await;
    }
    for (table, _) in TABLES {
        rejected_sql(
            &mut tx,
            format!("TRUNCATE canteiro.{table} CASCADE"),
            "42501",
        )
        .await;
    }
    tx.commit().await.unwrap();
    let amount: String = sqlx::query_scalar("SELECT increase::text FROM canteiro.access_request_versions WHERE tenant_id=$1 AND request_id=$2")
        .bind(a.tenant).bind(a.request).fetch_one(&fixture.admin).await.unwrap();
    assert_eq!(amount, "150.00");
    let approved: bool = sqlx::query_scalar(
        "SELECT approved FROM canteiro.access_approvals WHERE tenant_id=$1 AND request_id=$2",
    )
    .bind(a.tenant)
    .bind(a.request)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert!(approved);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_money_constraints_reject_excess_scale_range_and_nonfinite_without_rounding() {
    let fixture = Fixture::ready().await;
    let a = graph(&fixture, "Decimal", None).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut tx = scoped(&store, a.tenant, a.owner).await;
    for amount in [
        "0.001",
        "1.230",
        "-0.01",
        "1000000000000000000.00",
        "NaN",
        "Infinity",
        "-Infinity",
    ] {
        rejected_sql(&mut tx, format!("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES('{}','{}','{}','{}','viewer',ARRAY['view'],'{amount}'::numeric,clock_timestamp(),'{}')", a.tenant,Uuid::new_v4(),a.contract,a.owner,a.owner), "23514").await;
        rejected_sql(&mut tx, format!("INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES('{}','{}','approver','approve_budget','{amount}'::numeric)",a.tenant,a.contract), "23514").await;
        rejected_sql(
            &mut tx,
            format!("UPDATE canteiro.access_policies SET threshold='{amount}'::numeric"),
            "23514",
        )
        .await;
        for column in ["increase", "decrease", "threshold"] {
            let mut amounts = ["0.00", "0.00", "100.00"];
            amounts[match column {
                "increase" => 0,
                "decrease" => 1,
                _ => 2,
            }] = amount;
            rejected_sql(&mut tx, format!("INSERT INTO canteiro.access_request_versions(tenant_id,request_id,version,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer) VALUES('{}','{}',2,'Alteração fictícia','approve_change','{}'::numeric,'{}'::numeric,1,'{}'::numeric,2,false)",a.tenant,a.request,amounts[0],amounts[1],amounts[2]), "23514").await;
        }
    }
    let precise_grant = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'viewer',ARRAY['view'],999999999999999999.99,clock_timestamp(),$4)")
        .bind(a.tenant).bind(precise_grant).bind(a.contract).bind(a.owner).execute(&mut *tx.tx).await.unwrap();
    let amount: String = sqlx::query_scalar(
        "SELECT max_amount::text FROM canteiro.access_grants WHERE tenant_id=$1 AND id=$2",
    )
    .bind(a.tenant)
    .bind(precise_grant)
    .fetch_one(&mut *tx.tx)
    .await
    .unwrap();
    assert_eq!(amount, "999999999999999999.99");
    drop(tx);
    assert_clean_pool(&database).await;
    let original: String = sqlx::query_scalar(
        "SELECT max_amount::text FROM canteiro.access_grants WHERE tenant_id=$1 AND id=$2",
    )
    .bind(a.tenant)
    .bind(a.grant)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        original, "1000.00",
        "rollback conserva o valor exato anterior"
    );
    database.close().await;
    fixture.cleanup().await;
}

async fn responsibility(
    fixture: &Fixture,
    second_holder: bool,
) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let owner = person(fixture, "owner").await;
    let first = person(fixture, "first").await;
    let second = person(fixture, "second").await;
    let tenant = organization(fixture, owner, "Responsabilidade").await;
    add_member(fixture, tenant, first).await;
    add_member(fixture, tenant, second).await;
    let scope = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,created_by) VALUES($1,$2,'work','Obra fictícia',$3)")
        .bind(tenant).bind(scope).bind(owner).execute(&fixture.admin).await.unwrap();
    let first_grant = Uuid::new_v4();
    let second_grant = Uuid::new_v4();
    for (grant, user) in [(first_grant, first), (second_grant, second)]
        .into_iter()
        .take(if second_holder { 2 } else { 1 })
    {
        sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'technical_responsible',ARRAY['view','technical_accept'],1000,clock_timestamp()-interval '1 hour',$5)")
            .bind(tenant).bind(grant).bind(scope).bind(user).bind(owner).execute(&fixture.admin).await.unwrap();
    }
    sqlx::query("INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES($1,$2,'technical_responsible','technical_accept',100.00)")
        .bind(tenant).bind(scope).execute(&fixture.admin).await.unwrap();
    (
        tenant,
        owner,
        first,
        second,
        scope,
        first_grant,
        second_grant,
    )
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_last_required_holder_survives_revocation_member_end_and_failed_transfer() {
    let fixture = Fixture::ready().await;
    let (tenant, owner, first, second, scope, first_grant, second_grant) =
        responsibility(&fixture, false).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut tx = scoped(&store, tenant, owner).await;
    rejected_sql(&mut tx, format!("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp() WHERE tenant_id='{tenant}' AND id='{first_grant}'"), "23514").await;
    for change in [
        "valid_until=clock_timestamp()+interval '1 day'",
        "valid_from=clock_timestamp()+interval '1 day'",
        "role='viewer', actions=ARRAY['view']",
    ] {
        rejected_sql(&mut tx, format!("UPDATE canteiro.access_grants SET {change} WHERE tenant_id='{tenant}' AND id='{first_grant}'"), "42501").await;
    }
    rejected_sql(
        &mut tx,
        format!(
            "DELETE FROM canteiro.access_grants WHERE tenant_id='{tenant}' AND id='{first_grant}'"
        ),
        "42501",
    )
    .await;
    for change in [
        "active=false",
        "valid_until=clock_timestamp()+interval '1 day'",
        "valid_until=clock_timestamp()-interval '1 second'",
    ] {
        rejected_sql(&mut tx, format!("UPDATE canteiro.organization_members SET {change} WHERE tenant_id='{tenant}' AND user_id='{first}'"), "23514").await;
    }
    tx.commit().await.unwrap();
    // A transferência parecia válida, mas uma falha posterior deve desfazer as duas partes.
    let mut transfer = scoped(&store, tenant, owner).await;
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'technical_responsible',ARRAY['view','technical_accept'],1000,clock_timestamp()-interval '1 hour',$5)")
        .bind(tenant).bind(second_grant).bind(scope).bind(second).bind(owner).execute(&mut *transfer.tx).await.unwrap();
    sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp() WHERE tenant_id=$1 AND id=$2")
        .bind(tenant).bind(first_grant).execute(&mut *transfer.tx).await.unwrap();
    let failure = sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'viewer',ARRAY['view'],-1,clock_timestamp(),$5)")
        .bind(tenant).bind(Uuid::new_v4()).bind(scope).bind(second).bind(owner).execute(&mut *transfer.tx).await.unwrap_err();
    assert_eq!(sql_state(&failure), "23514");
    drop(transfer);
    assert_clean_pool(&database).await;
    let preserved: (i64, i64) = sqlx::query_as("SELECT count(*) FILTER(WHERE id=$2 AND revoked_at IS NULL),count(*) FILTER(WHERE id=$3) FROM canteiro.access_grants WHERE tenant_id=$1")
        .bind(tenant).bind(first_grant).bind(second_grant).fetch_one(&fixture.admin).await.unwrap();
    assert_eq!(preserved, (1, 0));
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_two_connections_cannot_remove_both_required_holders_and_rollback_releases_lock() {
    let fixture = Fixture::ready().await;
    let database = fixture
        .database(TransactionLimits::new(2, 2_000, 2_000, 1_000, 4_000).unwrap())
        .await;
    let store = AuthStore::new(database.clone());
    for (isolation, rollback_first, expected) in [
        (
            "SET TRANSACTION ISOLATION LEVEL READ COMMITTED",
            false,
            Some("23514"),
        ),
        (
            "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ",
            false,
            Some("40001"),
        ),
        ("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", true, None),
    ] {
        let (tenant, owner, _, second, _, first_grant, _) = responsibility(&fixture, true).await;
        let mut one = store.begin().await.unwrap();
        sqlx::raw_sql(isolation)
            .execute(&mut *one.tx)
            .await
            .unwrap();
        one.org_set_actor(owner).await.unwrap();
        one.org_set_tenant(tenant).await.unwrap();
        let mut two = store.begin().await.unwrap();
        sqlx::raw_sql(isolation)
            .execute(&mut *two.tx)
            .await
            .unwrap();
        two.org_set_actor(owner).await.unwrap();
        two.org_set_tenant(tenant).await.unwrap();
        for tx in [&mut one, &mut two] {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM canteiro.access_grants WHERE revoked_at IS NULL",
            )
            .fetch_one(&mut *tx.tx)
            .await
            .unwrap();
            assert_eq!(count, 2);
        }
        let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *two.tx)
            .await
            .unwrap();
        sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp() WHERE tenant_id=$1 AND id=$2")
            .bind(tenant).bind(first_grant).execute(&mut *one.tx).await.unwrap();
        let competitor = tokio::spawn(async move {
            match sqlx::query("UPDATE canteiro.organization_members SET active=false WHERE tenant_id=$1 AND user_id=$2")
                .bind(tenant).bind(second).execute(&mut *two.tx).await {
                Ok(_) => { two.commit().await.unwrap(); None },
                Err(error) => Some(sql_state(&error)),
            }
        });
        timeout(Duration::from_millis(700), async {
            loop {
                let waiting: bool = sqlx::query_scalar("SELECT coalesce(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1")
                    .bind(second_pid).fetch_one(&fixture.admin).await.unwrap();
                if waiting { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("a segunda conexão deve disputar o bloqueio da organização");
        if rollback_first {
            drop(one);
        } else {
            one.commit().await.unwrap();
        }
        assert_eq!(competitor.await.unwrap().as_deref(), expected);
        let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_grants g JOIN canteiro.organization_members m ON (m.tenant_id,m.user_id)=(g.tenant_id,g.user_id) WHERE g.tenant_id=$1 AND g.revoked_at IS NULL AND g.valid_until IS NULL AND m.active AND m.valid_until IS NULL")
            .bind(tenant).fetch_one(&fixture.admin).await.unwrap();
        assert_eq!(
            remaining, 1,
            "sempre há titular ativo após commit ou rollback concorrente"
        );
    }
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_successor_must_cover_required_action_amount_and_continuous_validity() {
    let fixture = Fixture::ready().await;
    let (tenant, owner, _, _, _, first_grant, second_grant) = responsibility(&fixture, true).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    for alteration in [
        "actions=ARRAY['view']",
        "max_amount=99.99",
        "valid_until=clock_timestamp()+interval '1 day'",
        "valid_from=clock_timestamp()+interval '1 day'",
    ] {
        // O primeiro titular conserva a responsabilidade enquanto preparamos um sucessor insuficiente.
        let sql = format!(
            "UPDATE canteiro.access_grants SET {alteration} WHERE tenant_id='{tenant}' AND id='{second_grant}'"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let mut tx = scoped(&store, tenant, owner).await;
        rejected_sql(&mut tx, format!("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp() WHERE tenant_id='{tenant}' AND id='{first_grant}'"), "23514").await;
        tx.commit().await.unwrap();
        sqlx::query("UPDATE canteiro.access_grants SET actions=ARRAY['view','technical_accept'],max_amount=1000,valid_until=NULL,valid_from=clock_timestamp()-interval '1 hour' WHERE tenant_id=$1 AND id=$2")
            .bind(tenant).bind(second_grant).execute(&fixture.admin).await.unwrap();
    }
    // A fronteira é inclusiva: o sucessor com o ato correto e exatamente R$100 basta.
    sqlx::query("UPDATE canteiro.access_grants SET max_amount=100.00 WHERE tenant_id=$1 AND id=$2")
        .bind(tenant)
        .bind(second_grant)
        .execute(&fixture.admin)
        .await
        .unwrap();
    let mut tx = scoped(&store, tenant, owner).await;
    sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp() WHERE tenant_id=$1 AND id=$2")
        .bind(tenant).bind(first_grant).execute(&mut *tx.tx).await.unwrap();
    tx.commit().await.unwrap();
    let preserved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.access_grants WHERE tenant_id=$1 AND revoked_at IS NULL",
    )
    .bind(tenant)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(preserved, 1);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_contract_parent_is_a_work_and_cannot_become_a_contract_cycle() {
    let fixture = Fixture::ready().await;
    let a = graph(&fixture, "Hierarquia", None).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut tx = scoped(&store, a.tenant, a.owner).await;
    rejected_sql(&mut tx, format!("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES('{}','{}','contract','Contrato sem obra','{}','{}')",a.tenant,Uuid::new_v4(),a.contract,a.owner), "23514").await;
    rejected_sql(&mut tx, format!("UPDATE canteiro.access_scopes SET kind='contract',parent_id='{}' WHERE tenant_id='{}' AND id='{}'",a.contract,a.tenant,a.work), "42501").await;
    drop(tx);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn access_runtime_connection_refuses_history_column_privileges_and_disabled_rls() {
    let fixture = Fixture::ready().await;
    let short = TransactionLimits::new(1, 120, 150, 50, 300).unwrap();
    let role = quoted_identifier(&fixture.runtime_role);
    let mut mutations = vec![
        (
            format!("GRANT UPDATE(increase) ON canteiro.access_request_versions TO {role}"),
            format!("REVOKE UPDATE(increase) ON canteiro.access_request_versions FROM {role}"),
        ),
        (
            format!("GRANT UPDATE(reason) ON canteiro.access_grants TO {role}"),
            format!("REVOKE UPDATE(reason) ON canteiro.access_grants FROM {role}"),
        ),
        (
            format!("GRANT DELETE ON canteiro.access_approvals TO {role}"),
            format!("REVOKE DELETE ON canteiro.access_approvals FROM {role}"),
        ),
        (
            format!("GRANT TRUNCATE ON canteiro.access_scopes TO {role}"),
            format!("REVOKE TRUNCATE ON canteiro.access_scopes FROM {role}"),
        ),
    ];
    for (table, _) in TABLES {
        mutations.push((
            format!("ALTER TABLE canteiro.{table} DISABLE ROW LEVEL SECURITY"),
            format!("ALTER TABLE canteiro.{table} ENABLE ROW LEVEL SECURITY"),
        ));
        mutations.push((
            format!("ALTER TABLE canteiro.{table} NO FORCE ROW LEVEL SECURITY"),
            format!("ALTER TABLE canteiro.{table} FORCE ROW LEVEL SECURITY"),
        ));
    }
    for (break_safety, restore) in mutations {
        sqlx::raw_sql(sqlx::AssertSqlSafe(break_safety.clone()))
            .execute(&fixture.admin)
            .await
            .unwrap();
        assert!(
            Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
                .await
                .is_err(),
            "configuração insegura deve impedir conexão: {break_safety}"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(restore))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let database = fixture.database(limits()).await;
        assert_clean_pool(&database).await;
        database.close().await;
    }
    fixture.cleanup().await;
}
