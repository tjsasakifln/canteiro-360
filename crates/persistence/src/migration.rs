use crate::{
    DatabaseEnvironment, DatabaseSettings, PersistenceError, PersistenceErrorKind,
    error::operation_id,
};
use sqlx::{Connection, PgConnection, migrate::Migrator};
use std::time::Duration;

pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Executado somente pelo processo de migração, nunca pelo servidor HTTP.
/// As identidades são provisionadas externamente e devem apontar ao mesmo banco.
pub async fn migrate(
    migrator: &DatabaseSettings,
    runtime: &DatabaseSettings,
    environment: DatabaseEnvironment,
) -> Result<(), PersistenceError> {
    let operation = operation_id();
    if !migrator.same_database(runtime) || migrator.username() == runtime.username() {
        return Err(PersistenceError::new(
            PersistenceErrorKind::InvalidInput,
            operation,
        ));
    }
    let mut connection = tokio::time::timeout(
        Duration::from_secs(5),
        PgConnection::connect_with(&migrator.options()),
    )
    .await
    .map_err(|_| PersistenceError::new(PersistenceErrorKind::Timeout, operation))?
    .map_err(|e| PersistenceError::from_sql(&e, operation))?;
    // pg_catalog implícito é pesquisado antes de public, sem mudar o schema de criação.
    sqlx::raw_sql("SET search_path = public; SET lock_timeout = '5s'; SET statement_timeout = '60s'; SET idle_in_transaction_session_timeout = '20s'")
        .execute(&mut connection).await.map_err(|e| PersistenceError::from_sql(&e, operation))?;
    // Identificar primeiro o ambiente, em transação. Nunca relabelar um banco existente.
    initialize_environment(&mut connection, environment)
        .await
        .map_err(|e| PersistenceError::from_sql(&e, operation))?;
    MIGRATOR
        .run(&mut connection)
        .await
        .map_err(|_| PersistenceError::new(PersistenceErrorKind::Internal, operation))?;
    let role = quoted_identifier(runtime.username());
    let database = quoted_identifier(
        runtime
            .options()
            .get_database()
            .expect("configuração validada"),
    );
    let grants = format!(
        "GRANT CONNECT ON DATABASE {database} TO {role}; \
        GRANT USAGE ON SCHEMA canteiro, canteiro_meta TO {role}; \
        GRANT SELECT ON canteiro_meta.environment TO {role}; \
        GRANT SELECT (version, checksum, success) ON public._sqlx_migrations TO {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.organizations, canteiro.organization_units TO {role}"
    );
    // Identificadores vêm da configuração administrativa e são sempre citados.
    sqlx::raw_sql(sqlx::AssertSqlSafe(grants))
        .execute(&mut connection)
        .await
        .map_err(|e| PersistenceError::from_sql(&e, operation))?;
    Ok(())
}

pub(crate) fn quoted_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub(crate) async fn initialize_environment(
    connection: &mut PgConnection,
    environment: DatabaseEnvironment,
) -> Result<(), sqlx::Error> {
    let mut tx = connection.begin().await?;
    // Serializa a criação do marcador, inclusive duas chamadas concorrentes com ambientes diferentes.
    sqlx::query("SELECT pg_advisory_xact_lock(360, 2)")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql("CREATE SCHEMA IF NOT EXISTS canteiro_meta; REVOKE ALL ON SCHEMA canteiro_meta FROM PUBLIC; \
        CREATE TABLE IF NOT EXISTS canteiro_meta.environment (singleton BOOLEAN PRIMARY KEY CHECK(singleton), \
        name TEXT NOT NULL CHECK(name IN ('development', 'homologation', 'test'))); \
        REVOKE CREATE ON SCHEMA public FROM PUBLIC").execute(&mut *tx).await?;
    sqlx::query("INSERT INTO canteiro_meta.environment(singleton, name) VALUES(true, $1) ON CONFLICT(singleton) DO NOTHING")
        .bind(environment.as_str()).execute(&mut *tx).await?;
    let actual: String =
        sqlx::query_scalar("SELECT name FROM canteiro_meta.environment WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    if actual != environment.as_str() {
        tx.rollback().await?;
        return Err(sqlx::Error::Protocol("Ambiente incompatível".into()));
    }
    tx.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn administrative_identifiers_are_quoted() {
        assert_eq!(quoted_identifier("a\"b"), "\"a\"\"b\"");
    }
}
