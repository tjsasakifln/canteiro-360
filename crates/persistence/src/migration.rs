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
        GRANT USAGE ON SCHEMA canteiro, canteiro_meta, canteiro_auth TO {role}; \
        GRANT SELECT ON canteiro_meta.environment TO {role}; \
        GRANT SELECT (version, checksum, success) ON public._sqlx_migrations TO {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.organizations, canteiro.organization_units TO {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.organization_members, canteiro.organization_invites, canteiro.organization_access_requests TO {role}; \
        GRANT SELECT, INSERT ON canteiro.organization_audit TO {role}; \
        GRANT SELECT, INSERT ON canteiro.access_scopes, canteiro.access_grants, canteiro.access_requirements, canteiro.access_policies, canteiro.access_requests, canteiro.access_request_versions, canteiro.access_approvals TO {role}; \
        GRANT SELECT, INSERT ON canteiro.operation_receipts, canteiro.authority_history TO {role}; \
        GRANT SELECT, INSERT ON canteiro.documents, canteiro.document_versions, canteiro.document_uploads, canteiro.document_signatures, canteiro.document_download_tickets, canteiro.document_history TO {role}; \
        GRANT UPDATE (active_version) ON canteiro.documents TO {role}; \
        GRANT UPDATE (status, reason, content, result_document, result_version) ON canteiro.document_uploads TO {role}; \
        GRANT DELETE ON canteiro.document_download_tickets TO {role}; \
        GRANT SELECT, INSERT ON canteiro.parties, canteiro.party_history, canteiro.party_bank_versions, canteiro.registry_operations, canteiro.portfolio_works, canteiro.portfolio_history TO {role}; \
        GRANT UPDATE (kind, tax_id, name, customer, supplier, professional, active, council, registration, uf, valid_from, valid_until, data, version) ON canteiro.parties TO {role}; \
        GRANT SELECT, INSERT, DELETE ON canteiro.portfolio_contract_links TO {role}; \
        GRANT UPDATE (title, status, data, version) ON canteiro.portfolio_works TO {role}; \
        GRANT SELECT, INSERT ON canteiro.projects, canteiro.project_versions, canteiro.project_party_links, canteiro.project_work_links, canteiro.technical_obligations, canteiro.obligation_versions, canteiro.obligation_replacements TO {role}; \
        GRANT UPDATE (current_version) ON canteiro.projects, canteiro.technical_obligations TO {role}; \
        GRANT SELECT ON canteiro.contract_history, canteiro.contract_notifications TO {role}; \
        GRANT SELECT, INSERT, UPDATE ON canteiro.user_notice_preferences, canteiro.user_notices, canteiro.user_notice_outbox TO {role}; \
        GRANT SELECT, INSERT ON canteiro.user_notice_delivery_history TO {role}; \
        GRANT SELECT, INSERT ON canteiro.cost_catalogs, canteiro.cost_composition_versions, canteiro.cost_composition_catalog_bindings, canteiro.budget_versions TO {role}; \
        GRANT SELECT, INSERT, UPDATE ON canteiro.cost_catalog_editions, canteiro.cost_compositions, canteiro.budget_records, canteiro.budget_import_batches TO {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.cost_catalog_items, canteiro.budget_edit_leases TO {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON canteiro.contract_edit_leases TO {role}; \
        GRANT EXECUTE ON FUNCTION canteiro.rename_contract_reference(UUID,UUID,UUID,UUID,UUID,BIGINT,TEXT,TEXT,TIMESTAMPTZ), canteiro_meta.consume_contract_notification(), canteiro_meta.consume_user_notice() TO {role}; \
        GRANT UPDATE (revoked_at, version) ON canteiro.access_grants TO {role}; \
        GRANT UPDATE (min_amount) ON canteiro.access_requirements TO {role}; \
        GRANT UPDATE (threshold, approvals_above_threshold, segregate_proposer, version) ON canteiro.access_policies TO {role}; \
        GRANT UPDATE (current_version, status, valid_approvals, updated_at) ON canteiro.access_requests TO {role}; \
        REVOKE UPDATE, DELETE, TRUNCATE ON canteiro.organization_audit FROM {role}; \
        GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA canteiro_auth TO {role}; \
        REVOKE UPDATE, DELETE ON canteiro_auth.audit FROM {role}; \
        REVOKE INSERT, UPDATE ON canteiro_auth.users FROM {role}; \
        GRANT INSERT (id, email, password_hash) ON canteiro_auth.users TO {role}; \
        GRANT UPDATE (password_hash, password_version, verified, mfa_secret, mfa_pending, mfa_pending_until, mfa_pending_session, mfa_last_step) ON canteiro_auth.users TO {role}"
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
