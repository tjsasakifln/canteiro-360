//! Entrega interna vinculada ao evento original e à sua empresa.
use crate::{
    Database, PersistenceError, PersistenceErrorKind,
    auth::{AuthStoreError, AuthTransaction},
    error::operation_id,
};
use chrono::{DateTime, Utc};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct StoredContractNotification {
    pub event_id: Uuid,
    pub version: i64,
    pub recorded_at: DateTime<Utc>,
}

impl AuthTransaction {
    /// Chamado depois de a aplicação conferir a permissão atual no contrato.
    pub async fn contract_notifications(
        &mut self,
        tenant: Uuid,
        actor: Uuid,
        contract: Uuid,
    ) -> Result<Vec<StoredContractNotification>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT h.event_id,h.version,h.recorded_at FROM canteiro.contract_history h \
             JOIN canteiro.contract_notifications n ON n.tenant_id=h.tenant_id AND n.event_id=h.event_id \
             WHERE h.tenant_id=$1 AND h.actor_id=$2 AND h.contract_id=$3 \
             ORDER BY h.version DESC LIMIT 20",
        )
        .bind(tenant)
        .bind(actor)
        .bind(contract)
        .fetch_all(&mut *self.tx)
        .await?)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationStep {
    Idle,
    Delivered,
    Deferred,
}

impl Database {
    /// A chamada SQL é uma transação implícita única. Uma perda de conexão
    /// durante o commit é recuperada pela próxima consulta à fila persistida.
    pub async fn consume_contract_notification(
        &self,
    ) -> Result<NotificationStep, PersistenceError> {
        let id = operation_id();
        let status = timeout(
            self.limits.transaction,
            sqlx::query_scalar::<_, String>("SELECT canteiro_meta.consume_contract_notification()")
                .fetch_one(&self.pool),
        )
        .await
        .map_err(|_| PersistenceError::new(PersistenceErrorKind::Timeout, id))?
        .map_err(|error| PersistenceError::from_sql(&error, id))?;
        match status.as_str() {
            "idle" => Ok(NotificationStep::Idle),
            "delivered" => Ok(NotificationStep::Delivered),
            "deferred" => Ok(NotificationStep::Deferred),
            _ => Err(PersistenceError::new(PersistenceErrorKind::Internal, id)),
        }
    }
}
