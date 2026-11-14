//! Compatibilidade com dados da #8 e proteção de histórico no PostgreSQL real.
use super::*;

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn contract_history_backfills_original_authors_in_two_tenants_and_is_append_only() {
    let f = Fixture::new().await;
    let mut migrator = PgConnection::connect_with(&f.migrator.options())
        .await
        .unwrap();
    initialize_environment(&mut migrator, DatabaseEnvironment::Test)
        .await
        .unwrap();
    MIGRATOR.run_to(5, &mut migrator).await.unwrap();
    drop(migrator);
    let contract = Uuid::new_v4();
    let occurred = chrono::DateTime::parse_from_rfc3339("2025-10-01T13:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let mut companies = Vec::new();
    for label in ["Primeira empresa", "Segunda empresa"] {
        let tenant = Uuid::new_v4();
        let actor = Uuid::new_v4();
        let work = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO canteiro_auth.users(id,email,password_hash) VALUES($1,$2,'fixture')",
        )
        .bind(actor)
        .bind(format!("{actor}@example.test"))
        .execute(&f.admin)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO canteiro.organizations(tenant_id,name,created_by) VALUES($1,$2,$3)",
        )
        .bind(tenant)
        .bind(label)
        .bind(actor)
        .execute(&f.admin)
        .await
        .unwrap();
        sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by,created_at)
                     VALUES($1,$2,'work','Obra',NULL,$5,$6),($1,$3,'contract',$4,$2,$5,$6)")
            .bind(tenant).bind(work).bind(contract).bind(label).bind(actor).bind(occurred)
            .execute(&f.admin).await.unwrap();
        companies.push((tenant, actor, label));
    }
    for _ in 0..2 {
        migrate(&f.migrator, &f.runtime, DatabaseEnvironment::Test)
            .await
            .unwrap();
    }
    let db = f.database(limits()).await;
    for (tenant, actor, label) in companies {
        let mut tx = crate::auth::AuthStore::new(db.clone())
            .begin()
            .await
            .unwrap();
        tx.org_set_actor(actor).await.unwrap();
        tx.org_set_tenant(tenant).await.unwrap();
        let history = tx.contract_history(tenant, contract, 0, 10).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[0].actor_id, actor);
        assert_eq!(history[0].after_title, label);
        assert_eq!(history[0].occurred_at, occurred);
        assert!(history[0].recorded_at > occurred);
        assert_eq!(history[0].origin, "migração");
        tx.commit().await.unwrap();
    }
    let mut runtime = db.pool.acquire().await.unwrap();
    for sql in [
        "UPDATE canteiro.contract_history SET reason='ocultar' WHERE false",
        "DELETE FROM canteiro.contract_history WHERE false",
        "TRUNCATE canteiro.contract_history",
        "UPDATE canteiro.operation_receipts SET result_version=99 WHERE false",
        "DELETE FROM canteiro.operation_receipts WHERE false",
        "TRUNCATE canteiro.operation_receipts",
    ] {
        let error = sqlx::query(sql).execute(&mut *runtime).await.unwrap_err();
        assert_eq!(sql_state(&error), "42501", "{sql}");
    }
    drop(runtime);
    db.close().await;
    f.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn reliability_runtime_refuses_column_updates_or_dispatch_data_grants() {
    let f = Fixture::ready().await;
    let shortened = TransactionLimits::new(1, 180, 125, 70, 300).unwrap();
    let role = quoted_identifier(&f.runtime_role);
    for (object, permission) in [
        ("canteiro.contract_history", "UPDATE (reason)"),
        ("canteiro.operation_receipts", "UPDATE (fingerprint)"),
        ("canteiro.authority_history", "UPDATE (reason)"),
        ("canteiro.contract_notifications", "UPDATE (delivered_at)"),
        (
            "canteiro_meta.contract_notification_outbox",
            "SELECT (event_id)",
        ),
    ] {
        let grant = format!("GRANT {permission} ON {object} TO {role}");
        sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
            .execute(&f.admin)
            .await
            .unwrap();
        assert!(
            Database::connect(&f.runtime, DatabaseEnvironment::Test, shortened)
                .await
                .is_err(),
            "{object}"
        );
        let revoke = format!("REVOKE {permission} ON {object} FROM {role}");
        sqlx::raw_sql(sqlx::AssertSqlSafe(revoke))
            .execute(&f.admin)
            .await
            .unwrap();
    }
    let compatible = f.database(limits()).await;
    compatible.close().await;
    f.cleanup().await;
}
