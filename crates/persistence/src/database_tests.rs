//! Testes de integração com PostgreSQL real. Cada caso cria banco e papéis descartáveis.
//! Execute explicitamente os ignorados; configuração ausente ou insegura faz o teste falhar.
use crate::{
    Database, DatabaseEnvironment, DatabaseSettings, MIGRATOR, NewUnit, PersistenceErrorKind,
    TransactionContext, TransactionLimits, UnitOfWork, migrate,
    migration::{initialize_environment, quoted_identifier},
};
use canteiro_domain::{ActorId, CivilDate, EntityId, EntityVersion, TenantId};
use sqlx::{
    ConnectOptions, Connection, PgConnection, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::time::{Instant, timeout};
use uuid::Uuid;

#[path = "organization_database_tests.rs"]
mod organizations;

#[path = "access_database_tests.rs"]
mod access;

#[path = "reliability_database_tests.rs"]
mod reliability;
#[path = "reliability_outbox_tests.rs"]
mod reliability_outbox;

#[path = "notice_delivery_tests.rs"]
mod notice_delivery;

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4()).unwrap()
}
fn entity() -> EntityId {
    EntityId::from_uuid(Uuid::new_v4()).unwrap()
}
fn actor() -> ActorId {
    ActorId::from_uuid(Uuid::new_v4()).unwrap()
}
fn context(tenant_id: TenantId) -> TransactionContext {
    TransactionContext::new(tenant_id, actor())
}
fn limits() -> TransactionLimits {
    TransactionLimits::new(1, 2_000, 750, 250, 3_000).unwrap()
}
fn settings(options: PgConnectOptions) -> DatabaseSettings {
    DatabaseSettings::from_external_url(options.to_url_lossy().as_str()).unwrap()
}
fn sql_state(error: &sqlx::Error) -> String {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .expect("erro deve vir do PostgreSQL")
        .into_owned()
}

struct Fixture {
    bootstrap: PgPool,
    admin: PgPool,
    migrator: DatabaseSettings,
    runtime: DatabaseSettings,
    database_name: String,
    owner_role: String,
    runtime_role: String,
}

impl Fixture {
    async fn new() -> Self {
        let external = std::env::var("CANTEIRO_TEST_DATABASE_URL")
            .expect("CANTEIRO_TEST_DATABASE_URL é obrigatória para a suíte PostgreSQL real");
        let bootstrap_settings = DatabaseSettings::from_external_url(&external)
            .expect("configuração externa de teste inválida");
        let options = bootstrap_settings.options();
        assert!(
            matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1")
                && options.get_socket().is_none(),
            "a suíte aceita somente PostgreSQL local dedicado a testes"
        );
        assert!(
            options
                .get_database()
                .is_some_and(|name| name.to_ascii_lowercase().contains("test")),
            "o banco de bootstrap precisa ser identificado como teste"
        );
        let bootstrap = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(3))
            .connect_with(options.clone())
            .await
            .expect("PostgreSQL local de teste precisa estar disponível");
        let is_admin: bool =
            sqlx::query_scalar("SELECT rolsuper FROM pg_roles WHERE rolname = current_user")
                .fetch_one(&bootstrap)
                .await
                .unwrap();
        assert!(is_admin, "bootstrap deve criar bancos e papéis de teste");
        let suffix = Uuid::new_v4().simple().to_string();
        let database_name = format!("c360_test_{suffix}");
        let owner_role = format!("c360_m_{suffix}");
        let runtime_role = format!("c360_r_{suffix}");
        // Credenciais fictícias transitórias, necessárias também no serviço de CI com SCRAM.
        let owner_credential = Uuid::new_v4().simple().to_string();
        let runtime_credential = Uuid::new_v4().simple().to_string();
        for (role, credential) in [
            (&owner_role, &owner_credential),
            (&runtime_role, &runtime_credential),
        ] {
            assert!(credential.bytes().all(|value| value.is_ascii_hexdigit()));
            let ddl = format!(
                "CREATE ROLE {} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION PASSWORD '{}'",
                quoted_identifier(role),
                credential
            );
            sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
                .execute(&bootstrap)
                .await
                .unwrap();
        }
        let ddl = format!(
            "CREATE DATABASE {} OWNER {}",
            quoted_identifier(&database_name),
            quoted_identifier(&owner_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
            .execute(&bootstrap)
            .await
            .unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(3)
            .connect_with(options.clone().database(&database_name))
            .await
            .unwrap();
        let migrator = settings(
            options
                .clone()
                .database(&database_name)
                .username(&owner_role)
                .password(&owner_credential),
        );
        let runtime = settings(
            options
                .database(&database_name)
                .username(&runtime_role)
                .password(&runtime_credential),
        );
        Self {
            bootstrap,
            admin,
            migrator,
            runtime,
            database_name,
            owner_role,
            runtime_role,
        }
    }

    async fn ready() -> Self {
        let fixture = Self::new().await;
        migrate(
            &fixture.migrator,
            &fixture.runtime,
            DatabaseEnvironment::Test,
        )
        .await
        .unwrap();
        fixture
    }

    async fn database(&self, limits: TransactionLimits) -> Database {
        Database::connect(&self.runtime, DatabaseEnvironment::Test, limits)
            .await
            .unwrap()
    }

    async fn seed_tenant(&self, tenant_id: TenantId) {
        sqlx::query(
            "INSERT INTO canteiro.organizations (tenant_id, name, created_by) VALUES ($1, $2, $3)",
        )
        .bind(tenant_id.as_uuid())
        .bind("Organização fictícia de teste")
        .bind(actor().as_uuid())
        .execute(&self.admin)
        .await
        .unwrap();
    }

    async fn seed_unit(&self, tenant_id: TenantId, id: EntityId) {
        sqlx::query(
            "INSERT INTO canteiro.organization_units (tenant_id, id, name, created_by, updated_by) \
             VALUES ($1, $2, 'Unidade fictícia de teste', $3, $3)",
        )
        .bind(tenant_id.as_uuid())
        .bind(id.as_uuid())
        .bind(actor().as_uuid())
        .execute(&self.admin)
        .await
        .unwrap();
    }

    async fn count_units(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_units")
            .fetch_one(&self.admin)
            .await
            .unwrap()
    }

    async fn cleanup(self) {
        self.admin.close().await;
        let ddl = format!(
            "DROP DATABASE {} WITH (FORCE)",
            quoted_identifier(&self.database_name)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
            .execute(&self.bootstrap)
            .await
            .unwrap();
        for role in [&self.runtime_role, &self.owner_role] {
            let ddl = format!("DROP ROLE {}", quoted_identifier(role));
            sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
                .execute(&self.bootstrap)
                .await
                .unwrap();
        }
        self.bootstrap.close().await;
    }
}

async fn work_pid(work: &mut UnitOfWork) -> i32 {
    sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut **work.transaction.as_mut().unwrap())
        .await
        .unwrap()
}

/// Inspeção deliberadamente interna: prova limpeza física além de filtros do repositório.
async fn assert_clean_pool(database: &Database) -> i32 {
    let mut connection = database.pool.acquire().await.unwrap();
    let (pid, tenant_id, actor_id, operation_id): (i32, String, String, String) = sqlx::query_as(
        "SELECT pg_backend_pid(), current_setting('canteiro.tenant_id', true), \
         current_setting('canteiro.actor_id', true), current_setting('canteiro.operation_id', true)",
    )
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    assert!(tenant_id.is_empty() && actor_id.is_empty() && operation_id.is_empty());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_units")
        .fetch_one(&mut *connection)
        .await
        .unwrap();
    assert_eq!(count, 0, "sem contexto nenhum registro deve ser visível");
    pid
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn blank_database_migrates_reproducibly_and_runtime_has_no_ddl_privileges() {
    let fixture = Fixture::ready().await;
    migrate(
        &fixture.migrator,
        &fixture.runtime,
        DatabaseEnvironment::Test,
    )
    .await
    .unwrap();
    let versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations WHERE success ORDER BY version")
            .fetch_all(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(versions, (1..=20).collect::<Vec<_>>());
    let database = fixture.database(limits()).await;
    let mut connection = database.pool.acquire().await.unwrap();
    let (privileged, owns_schema): (bool, bool) = sqlx::query_as(
        "SELECT r.rolsuper OR r.rolbypassrls OR r.rolcreatedb OR r.rolcreaterole, \
         EXISTS(SELECT 1 FROM pg_namespace n WHERE n.nspname = 'canteiro' AND n.nspowner = r.oid) \
         FROM pg_roles r WHERE r.rolname = current_user",
    )
    .fetch_one(&mut *connection)
    .await
    .unwrap();
    assert!(!privileged && !owns_schema);
    for ddl in [
        "TRUNCATE canteiro.organization_units",
        "ALTER TABLE canteiro.organization_units DISABLE ROW LEVEL SECURITY",
        "CREATE TABLE canteiro.unwanted (id INTEGER)",
        "UPDATE canteiro_meta.environment SET name = 'development'",
    ] {
        let error = sqlx::raw_sql(ddl)
            .execute(&mut *connection)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error), "42501");
    }
    drop(connection);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn version_one_data_survives_upgrade_with_composite_and_date_constraints() {
    let fixture = Fixture::new().await;
    let mut migrator_connection = PgConnection::connect_with(&fixture.migrator.options())
        .await
        .unwrap();
    initialize_environment(&mut migrator_connection, DatabaseEnvironment::Test)
        .await
        .unwrap();
    MIGRATOR.run_to(1, &mut migrator_connection).await.unwrap();
    let tenant_id = tenant();
    let parent = entity();
    fixture.seed_tenant(tenant_id).await;
    fixture.seed_unit(tenant_id, parent).await;
    let before: (String, i64, chrono::DateTime<chrono::Utc>, Uuid) = sqlx::query_as(
        "SELECT name, version, created_at, created_by FROM canteiro.organization_units WHERE id = $1",
    )
    .bind(parent.as_uuid())
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    drop(migrator_connection);
    // O processo antigo possui grants válidos, mas seu esquema ainda é incompatível.
    let grant = format!(
        "GRANT USAGE ON SCHEMA canteiro, canteiro_meta TO {role}; \
         GRANT SELECT ON canteiro_meta.environment TO {role}; \
         GRANT SELECT(version, checksum, success) ON public._sqlx_migrations TO {role}; \
         GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.organizations, canteiro.organization_units TO {role}",
        role = quoted_identifier(&fixture.runtime_role)
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
        .execute(&fixture.admin)
        .await
        .unwrap();
    let short = TransactionLimits::new(1, 120, 150, 50, 300).unwrap();
    assert!(
        Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
            .await
            .is_err(),
        "versão anterior não deve servir tráfego novo"
    );
    migrate(
        &fixture.migrator,
        &fixture.runtime,
        DatabaseEnvironment::Test,
    )
    .await
    .unwrap();
    let after: (String, i64, chrono::DateTime<chrono::Utc>, Uuid) = sqlx::query_as(
        "SELECT name, version, created_at, created_by FROM canteiro.organization_units WHERE id = $1",
    )
    .bind(parent.as_uuid())
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(before, after);
    let effective_on: Option<chrono::NaiveDate> =
        sqlx::query_scalar("SELECT effective_on FROM canteiro.organization_units WHERE id = $1")
            .bind(parent.as_uuid())
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert!(effective_on.is_none());
    for invalid_update in [
        "UPDATE canteiro.organization_units SET effective_on = DATE '10000-01-01'",
        "UPDATE canteiro.organization_units SET version = 0",
        "UPDATE canteiro.organization_units SET name = repeat(' ', 160) || 'A'",
        "UPDATE canteiro.organization_units SET parent_id = id",
        "UPDATE canteiro.organization_units SET updated_at = created_at - INTERVAL '1 second'",
    ] {
        let error = sqlx::raw_sql(invalid_update)
            .execute(&fixture.admin)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error), "23514");
    }
    let database = fixture.database(limits()).await;
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    let civil = "2024-02-29".parse::<CivilDate>().unwrap();
    let child = work
        .insert_unit(NewUnit {
            id: entity(),
            parent_id: Some(parent),
            name: "Filial fictícia",
            effective_on: Some(civil),
        })
        .await
        .unwrap();
    work.commit(()).await.unwrap().into_inner();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    let stored = work.unit(child.id).await.unwrap();
    assert_eq!(stored.effective_on, Some(civil));
    assert_eq!(stored.parent_id, Some(parent));
    work.rollback().await.unwrap();
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn edited_or_future_migration_history_is_rejected_without_erasing_existing_data() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    fixture.seed_tenant(tenant_id).await;
    fixture.seed_unit(tenant_id, entity()).await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum = $1 WHERE version = 1")
        .bind(vec![0_u8; 48])
        .execute(&fixture.admin)
        .await
        .unwrap();
    let error = migrate(
        &fixture.migrator,
        &fixture.runtime,
        DatabaseEnvironment::Test,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::Internal);
    let short = TransactionLimits::new(1, 120, 150, 50, 300).unwrap();
    assert!(
        Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
            .await
            .is_err()
    );
    sqlx::query("UPDATE public._sqlx_migrations SET checksum = $1 WHERE version = 1")
        .bind(MIGRATOR.iter().next().unwrap().checksum.as_ref())
        .execute(&fixture.admin)
        .await
        .unwrap();
    let future_version = MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap()
        .checked_add(1)
        .unwrap();
    sqlx::query("INSERT INTO public._sqlx_migrations (version,description,success,checksum,execution_time) VALUES ($1,'Versão futura fictícia',true,$2,0)")
        .bind(future_version).bind(vec![1_u8; 48]).execute(&fixture.admin).await.unwrap();
    assert!(
        Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
            .await
            .is_err()
    );
    assert!(
        migrate(
            &fixture.migrator,
            &fixture.runtime,
            DatabaseEnvironment::Test
        )
        .await
        .is_err()
    );
    assert_eq!(fixture.count_units().await, 1);
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn cross_tenant_foreign_keys_reject_insert_and_update_even_for_superuser() {
    let fixture = Fixture::ready().await;
    let first = tenant();
    let second = tenant();
    let parent = entity();
    let child = entity();
    fixture.seed_tenant(first).await;
    fixture.seed_tenant(second).await;
    fixture.seed_unit(first, parent).await;
    fixture.seed_unit(second, child).await;
    // O administrador ignora RLS: o erro precisa ser a FK composta, não a política de linha.
    let error = sqlx::query(
        "INSERT INTO canteiro.organization_units \
         (tenant_id, id, parent_id, name, created_by, updated_by) VALUES ($1,$2,$3,'Teste',$4,$4)",
    )
    .bind(second.as_uuid())
    .bind(entity().as_uuid())
    .bind(parent.as_uuid())
    .bind(actor().as_uuid())
    .execute(&fixture.admin)
    .await
    .unwrap_err();
    assert_eq!(sql_state(&error), "23503");
    let error = sqlx::query(
        "UPDATE canteiro.organization_units SET parent_id = $1 WHERE tenant_id = $2 AND id = $3",
    )
    .bind(parent.as_uuid())
    .bind(second.as_uuid())
    .bind(child.as_uuid())
    .execute(&fixture.admin)
    .await
    .unwrap_err();
    assert_eq!(sql_state(&error), "23503");
    assert_eq!(fixture.count_units().await, 2);
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn runtime_rls_blocks_unscoped_reads_writes_and_wrong_tenant_mutations() {
    let fixture = Fixture::ready().await;
    let first = tenant();
    let second = tenant();
    let foreign_unit = entity();
    fixture.seed_tenant(first).await;
    fixture.seed_tenant(second).await;
    fixture.seed_unit(second, foreign_unit).await;
    let database = fixture.database(limits()).await;
    assert_clean_pool(&database).await;
    let mut work = database.begin(context(first)).await.unwrap();
    let rows = work
        .execute(sqlx::query(
            "UPDATE canteiro.organization_units SET name = 'Indevido'",
        ))
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let error = work
        .execute(
            sqlx::query(
                "INSERT INTO canteiro.organization_units (tenant_id,id,name,created_by,updated_by) \
                 VALUES ($1,$2,'Indevido',$3,$3)",
            )
            .bind(second.as_uuid())
            .bind(entity().as_uuid())
            .bind(actor().as_uuid()),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::Internal);
    assert!(work.commit(()).await.is_err());
    let mut work = database.begin(context(first)).await.unwrap();
    assert_eq!(
        work.unit(foreign_unit).await.unwrap_err().kind(),
        PersistenceErrorKind::NotFound
    );
    assert!(work.commit(()).await.is_err());
    let mut connection = database.pool.acquire().await.unwrap();
    let error = sqlx::query(
        "INSERT INTO canteiro.organization_units (tenant_id,id,name,created_by,updated_by) \
         VALUES ($1,$2,'Sem contexto',$3,$3)",
    )
    .bind(first.as_uuid())
    .bind(entity().as_uuid())
    .bind(actor().as_uuid())
    .execute(&mut *connection)
    .await
    .unwrap_err();
    assert_eq!(sql_state(&error), "42501");
    drop(connection);
    assert_eq!(fixture.count_units().await, 1);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn one_connection_pool_clears_context_after_commit_rollback_failure_timeout_and_drop() {
    let fixture = Fixture::ready().await;
    let first = tenant();
    let second = tenant();
    fixture.seed_tenant(first).await;
    fixture.seed_tenant(second).await;
    let database = fixture
        .database(TransactionLimits::new(1, 2_000, 120, 60, 1_000).unwrap())
        .await;
    let pid = assert_clean_pool(&database).await;
    let mut work = database.begin(context(first)).await.unwrap();
    assert_eq!(work_pid(&mut work).await, pid);
    let created = work
        .insert_unit(NewUnit {
            id: entity(),
            parent_id: None,
            name: "Confirmado",
            effective_on: None,
        })
        .await
        .unwrap();
    let operation = work.operation_id();
    let committed = work.commit(created.clone()).await.unwrap();
    assert_eq!(committed.operation_id(), operation);
    assert_eq!(committed.into_inner(), created);
    assert_eq!(assert_clean_pool(&database).await, pid);
    let mut work = database.begin(context(second)).await.unwrap();
    assert_eq!(work_pid(&mut work).await, pid);
    work.insert_unit(NewUnit {
        id: entity(),
        parent_id: None,
        name: "Revertido",
        effective_on: None,
    })
    .await
    .unwrap();
    work.rollback().await.unwrap();
    assert_eq!(assert_clean_pool(&database).await, pid);
    let mut work = database.begin(context(first)).await.unwrap();
    assert_eq!(work_pid(&mut work).await, pid);
    assert!(work.execute(sqlx::query("SELECT 1 / 0")).await.is_err());
    assert!(work.commit(()).await.is_err());
    assert_eq!(assert_clean_pool(&database).await, pid);
    let mut work = database.begin(context(second)).await.unwrap();
    assert_eq!(work_pid(&mut work).await, pid);
    assert_eq!(
        work.execute(sqlx::query("SELECT pg_sleep(1)"))
            .await
            .unwrap_err()
            .kind(),
        PersistenceErrorKind::Timeout
    );
    assert!(work.commit(()).await.is_err());
    assert_eq!(assert_clean_pool(&database).await, pid);
    let mut work = database.begin(context(second)).await.unwrap();
    work.insert_unit(NewUnit {
        id: entity(),
        parent_id: None,
        name: "Descartado",
        effective_on: None,
    })
    .await
    .unwrap();
    drop(work);
    assert_eq!(assert_clean_pool(&database).await, pid);
    // SET SESSION acidental é limpo, embora SET LOCAL normalmente o restaurasse ao terminar.
    let mut connection = database.pool.acquire().await.unwrap();
    sqlx::query("SELECT set_config('canteiro.tenant_id', $1, false)")
        .bind(first.to_string())
        .execute(&mut *connection)
        .await
        .unwrap();
    drop(connection);
    assert_eq!(assert_clean_pool(&database).await, pid);
    assert_eq!(fixture.count_units().await, 1);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn ignored_second_write_and_raw_sql_errors_cannot_commit_partial_work() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    fixture.seed_tenant(tenant_id).await;
    let database = fixture.database(limits()).await;
    let id = entity();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    work.insert_unit(NewUnit {
        id,
        parent_id: None,
        name: "Primeira gravação",
        effective_on: None,
    })
    .await
    .unwrap();
    let duplicate = work
        .insert_unit(NewUnit {
            id,
            parent_id: None,
            name: "Segunda gravação",
            effective_on: None,
        })
        .await
        .unwrap_err();
    assert_eq!(duplicate.kind(), PersistenceErrorKind::Conflict);
    assert_eq!(work.commit(()).await.unwrap_err(), duplicate);
    assert_eq!(fixture.count_units().await, 0);
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    work.insert_unit(NewUnit {
        id,
        parent_id: None,
        name: "Antes de falha SQL",
        effective_on: None,
    })
    .await
    .unwrap();
    // Simula adaptador interno incorreto que ignorou o Result e não envenenou a unidade.
    assert!(
        sqlx::query("SELECT 1 / 0")
            .execute(&mut **work.transaction.as_mut().unwrap())
            .await
            .is_err()
    );
    assert!(
        work.commit(()).await.is_err(),
        "COMMIT de transação abortada não é sucesso"
    );
    assert_eq!(fixture.count_units().await, 0);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn cancelled_borrowed_write_cannot_later_be_committed() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    let parent = entity();
    let child = entity();
    fixture.seed_tenant(tenant_id).await;
    fixture.seed_unit(tenant_id, parent).await;
    let database = fixture.database(limits()).await;
    let mut blocker = fixture.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM canteiro.organization_units WHERE id = $1 FOR UPDATE")
        .bind(parent.as_uuid())
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    let interrupted = timeout(
        Duration::from_millis(40),
        work.insert_unit(NewUnit {
            id: child,
            parent_id: Some(parent),
            name: "Cancelado pelo chamador",
            effective_on: None,
        }),
    )
    .await;
    assert!(
        interrupted.is_err(),
        "a FK precisa aguardar o bloqueio real"
    );
    blocker.rollback().await.unwrap();
    assert!(
        work.commit(()).await.is_err(),
        "cancelar apenas o futuro não pode permitir gravação posterior"
    );
    assert_clean_pool(&database).await;
    let child_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_units WHERE id = $1")
            .bind(child.as_uuid())
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(child_count, 0);
    assert_eq!(fixture.count_units().await, 1);
    let mut blocker = fixture.admin.begin().await.unwrap();
    sqlx::query("LOCK TABLE canteiro.organization_units IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    assert!(
        timeout(Duration::from_millis(40), work.unit(parent))
            .await
            .is_err()
    );
    blocker.rollback().await.unwrap();
    assert!(
        work.commit(()).await.is_err(),
        "leitura interrompida também invalida a unidade"
    );
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn cancelled_task_rolls_back_pending_transaction_and_releases_clean_connection() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    fixture.seed_tenant(tenant_id).await;
    let database = fixture
        .database(TransactionLimits::new(1, 2_000, 300, 100, 1_000).unwrap())
        .await;
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    work.insert_unit(NewUnit {
        id: entity(),
        parent_id: None,
        name: "Tarefa interrompida",
        effective_on: None,
    })
    .await
    .unwrap();
    let pid = work_pid(&mut work).await;
    let pending =
        tokio::spawn(async move { work.execute(sqlx::query("SELECT pg_sleep(2)")).await });
    timeout(Duration::from_secs(1), async {
        loop {
            let sleeping: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid = $1 AND wait_event = 'PgSleep')",
            ).bind(pid).fetch_one(&fixture.admin).await.unwrap();
            if sleeping { break; }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await.expect("a consulta deve estar em execução antes de cancelar a tarefa");
    pending.abort();
    assert!(pending.await.unwrap_err().is_cancelled());
    // Uma conexão irrecuperável pode ser descartada pelo driver; contexto nunca pode sobreviver.
    assert_clean_pool(&database).await;
    assert_eq!(fixture.count_units().await, 0);
    let mut next = database.begin(context(tenant_id)).await.unwrap();
    next.insert_unit(NewUnit {
        id: entity(),
        parent_id: None,
        name: "Após cancelamento",
        effective_on: None,
    })
    .await
    .unwrap();
    next.commit(()).await.unwrap().into_inner();
    assert_eq!(fixture.count_units().await, 1);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn lock_pool_and_global_transaction_deadlines_abort_without_partial_writes() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    let id = entity();
    fixture.seed_tenant(tenant_id).await;
    fixture.seed_unit(tenant_id, id).await;
    let database = fixture
        .database(TransactionLimits::new(1, 100, 500, 60, 600).unwrap())
        .await;
    let held = database.begin(context(tenant_id)).await.unwrap();
    let unavailable = database.begin(context(tenant_id)).await;
    assert!(matches!(unavailable, Err(error) if error.kind() == PersistenceErrorKind::Timeout));
    held.rollback().await.unwrap();
    let mut blocker = fixture.admin.begin().await.unwrap();
    sqlx::query("SELECT id FROM canteiro.organization_units WHERE id = $1 FOR UPDATE")
        .bind(id.as_uuid())
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    let started = Instant::now();
    let error = work
        .rename_unit(id, EntityVersion::INITIAL, "Bloqueado")
        .await
        .unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(work.commit(()).await.is_err());
    blocker.rollback().await.unwrap();
    assert_clean_pool(&database).await;
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    work.insert_unit(NewUnit {
        id: entity(),
        parent_id: None,
        name: "Prazo global",
        effective_on: None,
    })
    .await
    .unwrap();
    work.execute(sqlx::query("SELECT pg_sleep(0.35)"))
        .await
        .unwrap();
    assert_eq!(
        work.execute(sqlx::query("SELECT pg_sleep(0.35)"))
            .await
            .unwrap_err()
            .kind(),
        PersistenceErrorKind::Timeout
    );
    assert!(work.commit(()).await.is_err());
    assert_clean_pool(&database).await;
    assert_eq!(fixture.count_units().await, 1);
    let version: i64 =
        sqlx::query_scalar("SELECT version FROM canteiro.organization_units WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(version, 1);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn concurrent_expected_version_updates_have_one_winner_and_never_overflow() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    let id = entity();
    fixture.seed_tenant(tenant_id).await;
    fixture.seed_unit(tenant_id, id).await;
    let database = fixture
        .database(TransactionLimits::new(2, 2_000, 1_000, 500, 3_000).unwrap())
        .await;
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let mut handles = Vec::new();
    for name in ["Revisão concorrente A", "Revisão concorrente B"] {
        let database = database.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let mut work = database.begin(context(tenant_id)).await?;
            barrier.wait().await;
            let row = work.rename_unit(id, EntityVersion::INITIAL, name).await?;
            work.commit(row)
                .await
                .map(|committed| committed.into_inner())
        }));
    }
    let mut won = 0;
    let mut conflicted = 0;
    for handle in handles {
        match handle.await.unwrap() {
            Ok(record) => {
                won += 1;
                assert_eq!(record.version.get(), 2);
            }
            Err(error) => {
                conflicted += 1;
                assert_eq!(error.kind(), PersistenceErrorKind::VersionConflict);
            }
        }
    }
    assert_eq!((won, conflicted), (1, 1));
    let stored_version: i64 =
        sqlx::query_scalar("SELECT version FROM canteiro.organization_units WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(stored_version, 2);
    sqlx::query("UPDATE canteiro.organization_units SET version = $1 WHERE id = $2")
        .bind(i64::MAX)
        .bind(id.as_uuid())
        .execute(&fixture.admin)
        .await
        .unwrap();
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    assert_eq!(work.unit(id).await.unwrap().version.get(), i64::MAX);
    let error = work
        .rename_unit(
            id,
            EntityVersion::new(i64::MAX).unwrap(),
            "Não deve transbordar",
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::InvalidInput);
    assert!(work.commit(()).await.is_err());
    let stored_version: i64 =
        sqlx::query_scalar("SELECT version FROM canteiro.organization_units WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(stored_version, i64::MAX);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn environment_owner_and_inherited_privilege_mismatches_fail_closed() {
    let fixture = Fixture::ready().await;
    let short = TransactionLimits::new(1, 120, 150, 50, 300).unwrap();
    assert!(
        Database::connect(&fixture.runtime, DatabaseEnvironment::Homologation, short)
            .await
            .is_err()
    );
    assert!(
        Database::connect(&fixture.migrator, DatabaseEnvironment::Test, short)
            .await
            .is_err()
    );
    assert!(
        migrate(
            &fixture.migrator,
            &fixture.migrator,
            DatabaseEnvironment::Test
        )
        .await
        .is_err()
    );
    assert!(
        migrate(
            &fixture.migrator,
            &fixture.runtime,
            DatabaseEnvironment::Development
        )
        .await
        .is_err()
    );
    let environment: String =
        sqlx::query_scalar("SELECT name FROM canteiro_meta.environment WHERE singleton")
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(environment, "test");
    // Conexões válidas verificam identidade e privilégios, não prazo de aquisição.
    let database = fixture.database(limits()).await;
    let ddl = format!(
        "GRANT {} TO {}",
        quoted_identifier(&fixture.owner_role),
        quoted_identifier(&fixture.runtime_role)
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
        .execute(&fixture.admin)
        .await
        .unwrap();
    assert!(
        database.begin(context(tenant())).await.is_err(),
        "privilégio posterior exige nova validação do pool"
    );
    assert!(
        Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
            .await
            .is_err()
    );
    let ddl = format!(
        "REVOKE {} FROM {}",
        quoted_identifier(&fixture.owner_role),
        quoted_identifier(&fixture.runtime_role)
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(ddl))
        .execute(&fixture.admin)
        .await
        .unwrap();
    database.close().await;
    let database = fixture.database(limits()).await;
    assert_clean_pool(&database).await;
    database.close().await;
    for permission in ["INSERT", "DELETE", "TRUNCATE"] {
        let grant = format!(
            "GRANT {permission} ON canteiro_meta.environment TO {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
            .execute(&fixture.admin)
            .await
            .unwrap();
        assert!(
            Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
                .await
                .is_err()
        );
        let revoke = format!(
            "REVOKE {permission} ON canteiro_meta.environment FROM {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(revoke))
            .execute(&fixture.admin)
            .await
            .unwrap();
    }
    // Privilégios por coluna não aparecem em has_table_privilege. Provar essa
    // diferença evita que o teste passe apenas pela proteção de tabela existente.
    for (relation, permission, columns) in [
        ("canteiro_meta.environment", "UPDATE", "name"),
        ("canteiro_meta.environment", "INSERT", "singleton, name"),
        ("public._sqlx_migrations", "UPDATE", "checksum"),
        (
            "public._sqlx_migrations",
            "INSERT",
            "version, description, success, checksum, execution_time",
        ),
    ] {
        let grant = format!(
            "GRANT {permission} ({columns}) ON {relation} TO {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let privileges: (bool, bool) = sqlx::query_as(
            "SELECT has_table_privilege($1::text, $2::text, $3::text), \
             has_any_column_privilege($1::text, $2::text, $3::text)",
        )
        .bind(&fixture.runtime_role)
        .bind(relation)
        .bind(permission)
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
        assert_eq!(
            privileges,
            (false, true),
            "probe exige concessão somente por coluna"
        );
        assert!(
            Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
                .await
                .is_err()
        );
        let revoke = format!(
            "REVOKE {permission} ({columns}) ON {relation} FROM {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(revoke))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let restored = fixture.database(limits()).await;
        assert_clean_pool(&restored).await;
        restored.close().await;
    }
    for schema in ["canteiro_meta", "public"] {
        let grant = format!(
            "GRANT CREATE ON SCHEMA {schema} TO {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
            .execute(&fixture.admin)
            .await
            .unwrap();
        assert!(
            Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
                .await
                .is_err()
        );
        let revoke = format!(
            "REVOKE CREATE ON SCHEMA {schema} FROM {}",
            quoted_identifier(&fixture.runtime_role)
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(revoke))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let restored = fixture.database(limits()).await;
        assert_clean_pool(&restored).await;
        restored.close().await;
    }
    let different_database = settings(fixture.runtime.options().database("another_test_database"));
    assert_eq!(
        migrate(
            &fixture.migrator,
            &different_database,
            DatabaseEnvironment::Test
        )
        .await
        .unwrap_err()
        .kind(),
        PersistenceErrorKind::InvalidInput
    );
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn migration_advisory_lock_has_a_bounded_wait_and_rolls_back_bootstrap() {
    let fixture = Fixture::new().await;
    let mut blocker = fixture.admin.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(360, 2)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let started = Instant::now();
    let error = migrate(
        &fixture.migrator,
        &fixture.runtime,
        DatabaseEnvironment::Test,
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "espera administrativa deve ser limitada"
    );
    blocker.rollback().await.unwrap();
    let schema_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = 'canteiro_meta')",
    )
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert!(
        !schema_exists,
        "bootstrap bloqueado não pode deixar estado parcial"
    );
    migrate(
        &fixture.migrator,
        &fixture.runtime,
        DatabaseEnvironment::Test,
    )
    .await
    .unwrap();
    let database = fixture.database(limits()).await;
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn diagnostic_capture() -> Capture {
    // Um único subscriber também observa callbacks SQLx em tarefas auxiliares.
    // A instalação global evita a disputa de interesse dos callsites entre testes paralelos.
    static CAPTURE: OnceLock<Capture> = OnceLock::new();
    CAPTURE
        .get_or_init(|| {
            let captured = Capture(Arc::new(Mutex::new(Vec::new())));
            let writer = captured.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_max_level(tracing::Level::TRACE)
                .with_ansi(false)
                .without_time()
                .with_writer(move || writer.clone())
                .finish();
            tracing::subscriber::set_global_default(subscriber)
                .expect("subscriber global exclusivo da suíte de persistência");
            captured
        })
        .clone()
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn actual_database_error_is_redacted_in_diagnostics_and_public_persistence_error() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    fixture.seed_tenant(tenant_id).await;
    let database = fixture.database(limits()).await;
    let captured = diagnostic_capture();
    let marker = format!("fixture-confidential-{}", Uuid::new_v4().simple());
    let transaction_context = context(tenant_id);
    let operation = transaction_context.operation_id();
    let mut work = database.begin(transaction_context).await.unwrap();
    let error = work
        .execute(sqlx::query("SELECT CAST($1 AS INTEGER)").bind(&marker))
        .await
        .unwrap_err();
    assert_eq!(error.operation_id(), operation);
    assert_eq!(error.kind(), PersistenceErrorKind::Internal);
    assert!(work.commit(()).await.is_err());
    assert_clean_pool(&database).await;
    database.close().await;
    let diagnostic = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
    let printed = format!("{error:?} {error}");
    assert!(!diagnostic.is_empty(), "captura de diagnóstico vazia");
    assert!(
        diagnostic.contains("Falha de persistência"),
        "evento de persistência ausente"
    );
    for value in [&diagnostic, &printed] {
        assert!(!value.contains(&marker));
        assert!(!value.contains("SELECT CAST"));
        assert!(!value.contains("invalid input syntax"));
        assert!(!value.contains(&tenant_id.to_string()));
        assert!(value.contains(&operation.to_string()));
    }
    assert!(diagnostic.contains("Internal"));
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn failure_delayed_until_commit_is_never_reported_as_confirmed_or_safe_to_retry() {
    let fixture = Fixture::ready().await;
    let tenant_id = tenant();
    fixture.seed_tenant(tenant_id).await;
    // Falha determinística somente no COMMIT, após INSERT e leitura provisória aprovados.
    sqlx::raw_sql(
        "CREATE FUNCTION canteiro.test_reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture delayed failure' USING ERRCODE = '23514'; END $$; \
         CREATE CONSTRAINT TRIGGER test_commit_failure AFTER INSERT ON canteiro.organization_units \
         DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION canteiro.test_reject_commit()",
    )
    .execute(&fixture.admin)
    .await
    .unwrap();
    let database = fixture.database(limits()).await;
    let mut work = database.begin(context(tenant_id)).await.unwrap();
    let operation = work.operation_id();
    let provisional = work
        .insert_unit(NewUnit {
            id: entity(),
            parent_id: None,
            name: "Sem confirmação",
            effective_on: None,
        })
        .await
        .unwrap();
    let error = work.commit(provisional).await.unwrap_err();
    assert_eq!(error.kind(), PersistenceErrorKind::OutcomeUnknown);
    assert_eq!(error.operation_id(), operation);
    assert!(!format!("{error:?} {error}").contains("fixture delayed failure"));
    assert_eq!(fixture.count_units().await, 0);
    assert_clean_pool(&database).await;
    database.close().await;
    fixture.cleanup().await;
}
