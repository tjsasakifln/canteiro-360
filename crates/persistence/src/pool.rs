use crate::{
    DatabaseEnvironment, DatabaseSettings, MIGRATOR, PersistenceError, TransactionLimits,
    error::operation_id,
};
use sqlx::{PgConnection, PgPool, postgres::PgPoolOptions};

/// Compartilhável entre serviços, sem expor pool/executor ou credencial.
#[derive(Clone)]
pub struct Database {
    pub(crate) pool: PgPool,
    pub(crate) limits: TransactionLimits,
}

impl Database {
    pub async fn connect(
        settings: &DatabaseSettings,
        environment: DatabaseEnvironment,
        limits: TransactionLimits,
    ) -> Result<Self, PersistenceError> {
        let operation = operation_id();
        let pool = PgPoolOptions::new()
            .max_connections(limits.max_connections)
            .min_connections(0)
            .acquire_timeout(limits.acquire)
            .after_connect(move |connection, _| {
                Box::pin(async move {
                    clean_session(connection)
                        .await
                        .map_err(redacted_pool_error)?;
                    validate_runtime(connection, environment)
                        .await
                        .map_err(redacted_pool_error)
                })
            })
            // Limpa também antes de emprestar: SET LOCAL restaura um possível SET SESSION
            // anterior. RESET e valores vazios impedem reaparecimento de empresa/ator.
            .before_acquire(move |connection, _| {
                Box::pin(async move {
                    clean_session(connection)
                        .await
                        .map_err(redacted_pool_error)?;
                    validate_runtime(connection, environment)
                        .await
                        .map_err(redacted_pool_error)?;
                    Ok(true)
                })
            })
            .after_release(|connection, _| {
                Box::pin(async move {
                    clean_session(connection)
                        .await
                        .map_err(redacted_pool_error)?;
                    Ok(true)
                })
            })
            .connect_with(settings.options())
            .await
            .map_err(|e| PersistenceError::from_sql(&e, operation))?;
        Ok(Self { pool, limits })
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Database([configuração privada])")
    }
}

fn redacted_pool_error(_: sqlx::Error) -> sqlx::Error {
    // SQLx registra erros dos callbacks internamente. Nunca lhe devolver mensagem
    // do servidor: pode conter a entrada de uma consulta cancelada anteriormente.
    sqlx::Error::Protocol("Conexão de execução indisponível ou incompatível".into())
}

async fn clean_session(connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    sqlx::raw_sql("RESET ROLE; RESET ALL; SET search_path = pg_catalog; SET TIME ZONE 'UTC'; SET row_security = on; SET client_min_messages = 'ERROR'; \
        SET statement_timeout = '5s'; SET lock_timeout = '1s'; SET idle_in_transaction_session_timeout = '20s'; \
        SELECT set_config('canteiro.tenant_id', '', false), set_config('canteiro.actor_id', '', false), \
        set_config('canteiro.operation_id', '', false)").execute(connection).await?;
    Ok(())
}

async fn validate_runtime(
    connection: &mut PgConnection,
    environment: DatabaseEnvironment,
) -> Result<(), sqlx::Error> {
    let safe: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM pg_roles r WHERE pg_has_role(current_user, r.oid, 'MEMBER') \
          AND (r.rolsuper OR r.rolbypassrls OR r.rolcreaterole OR r.rolcreatedb OR r.rolreplication \
            OR r.oid IN (SELECT relowner FROM pg_class WHERE relnamespace IN ('canteiro'::regnamespace, 'canteiro_auth'::regnamespace)) \
            OR r.oid IN (SELECT nspowner FROM pg_namespace WHERE nspname IN ('canteiro', 'canteiro_meta', 'canteiro_auth')) \
            OR r.oid = (SELECT datdba FROM pg_database WHERE datname = current_database()) \
            OR r.rolname IN ('pg_read_all_data', 'pg_write_all_data', 'pg_execute_server_program', 'pg_read_server_files', 'pg_write_server_files'))) \
         AND NOT has_schema_privilege(current_user, 'canteiro', 'CREATE') \
         AND NOT has_schema_privilege(current_user, 'canteiro_meta', 'CREATE') \
         AND NOT has_schema_privilege(current_user, 'public', 'CREATE') \
         AND NOT has_schema_privilege(current_user, 'canteiro_auth', 'CREATE') \
         AND NOT has_column_privilege(current_user, 'canteiro_auth.users', 'privileged', 'INSERT,UPDATE') \
         AND NOT has_table_privilege(current_user, 'canteiro_auth.audit', 'UPDATE,DELETE,TRUNCATE') \
         AND NOT has_any_column_privilege(current_user, 'canteiro_auth.audit', 'UPDATE') \
         AND NOT EXISTS (SELECT 1 FROM pg_class WHERE relnamespace = 'canteiro_auth'::regnamespace \
             AND relkind = 'r' AND has_table_privilege(current_user, oid, 'TRUNCATE')) \
         AND NOT has_table_privilege(current_user, 'canteiro.organizations', 'TRUNCATE') \
         AND NOT has_table_privilege(current_user, 'canteiro.organization_units', 'TRUNCATE') \
         AND NOT has_table_privilege(current_user, 'canteiro_meta.environment', 'UPDATE') \
         AND NOT has_table_privilege(current_user, 'canteiro_meta.environment', 'INSERT,DELETE,TRUNCATE') \
         AND NOT has_any_column_privilege(current_user, 'canteiro_meta.environment', 'INSERT,UPDATE') \
         AND NOT has_table_privilege(current_user, 'public._sqlx_migrations', 'INSERT,UPDATE,DELETE,TRUNCATE') \
         AND NOT has_any_column_privilege(current_user, 'public._sqlx_migrations', 'INSERT,UPDATE') \
         AND NOT EXISTS (SELECT 1 FROM pg_class WHERE relnamespace = 'canteiro'::regnamespace \
             AND relkind = 'r' AND (NOT relrowsecurity OR NOT relforcerowsecurity))")
        .fetch_one(&mut *connection).await?;
    let actual: String =
        sqlx::query_scalar("SELECT name FROM canteiro_meta.environment WHERE singleton")
            .fetch_one(&mut *connection)
            .await?;
    if !safe || actual != environment.as_str() {
        return Err(sqlx::Error::Protocol(
            "Configuração de execução incompatível".into(),
        ));
    }
    let applied: Vec<(i64, Vec<u8>, bool)> = sqlx::query_as(
        "SELECT version, checksum, success FROM public._sqlx_migrations ORDER BY version",
    )
    .fetch_all(&mut *connection)
    .await?;
    if applied.len() != MIGRATOR.iter().count()
        || applied.iter().zip(MIGRATOR.iter()).any(|(row, expected)| {
            row.0 != expected.version || row.1.as_slice() != expected.checksum.as_ref() || !row.2
        })
    {
        return Err(sqlx::Error::Protocol(
            "Versão de banco incompatível com esta aplicação".into(),
        ));
    }
    Ok(())
}
