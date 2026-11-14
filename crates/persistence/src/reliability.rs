//! Recibos duráveis, histórico contratual e reserva de edição no PostgreSQL.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct OperationReceipt {
    pub actor_id: Uuid,
    pub scope_id: Uuid,
    pub fingerprint: Vec<u8>,
    pub result_id: Uuid,
    pub result_version: i64,
}

#[derive(sqlx::FromRow)]
pub struct StoredContractHistory {
    pub event_id: Uuid,
    pub contract_id: Uuid,
    pub version: i64,
    pub actor_id: Uuid,
    pub actor_email: String,
    pub action: String,
    pub origin: String,
    pub before_title: Option<String>,
    pub after_title: String,
    pub reason: String,
    pub occurred_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
}

pub struct EditLease {
    pub tenant: Uuid,
    pub contract: Uuid,
    pub actor: Uuid,
    pub session: Uuid,
    pub key: Uuid,
}

pub struct ContractRevision<'a> {
    pub lease: &'a EditLease,
    pub expected_version: i64,
    pub title: &'a str,
    pub reason: &'a str,
    pub occurred_at: Option<DateTime<Utc>>,
}

impl AuthTransaction {
    pub async fn operation_receipt(
        &mut self,
        tenant: Uuid,
        act: &str,
        key: Uuid,
    ) -> Result<Option<OperationReceipt>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT actor_id,scope_id,fingerprint,result_id,result_version FROM canteiro.operation_receipts
             WHERE tenant_id=$1 AND act=$2 AND operation_key=$3",
        )
        .bind(tenant)
        .bind(act)
        .bind(key)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    pub async fn insert_operation_receipt(
        &mut self,
        tenant: Uuid,
        act: &str,
        key: Uuid,
        receipt: &OperationReceipt,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query(
            "INSERT INTO canteiro.operation_receipts
             (tenant_id,act,operation_key,actor_id,scope_id,fingerprint,result_id,result_version)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (tenant_id,act,operation_key) DO NOTHING",
        )
        .bind(tenant)
        .bind(act)
        .bind(key)
        .bind(receipt.actor_id)
        .bind(receipt.scope_id)
        .bind(&receipt.fingerprint)
        .bind(receipt.result_id)
        .bind(receipt.result_version)
        .execute(&mut *self.tx)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn contract_history(
        &mut self,
        tenant: Uuid,
        contract: Uuid,
        after: i64,
        limit: i64,
    ) -> Result<Vec<StoredContractHistory>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT h.event_id,h.contract_id,h.version,h.actor_id,u.email AS actor_email,h.action,h.origin,
                    h.before_title,h.after_title,h.reason,h.occurred_at,h.recorded_at
             FROM canteiro.contract_history h JOIN canteiro_auth.users u ON u.id=h.actor_id
             WHERE h.tenant_id=$1 AND h.contract_id=$2 AND h.version>$3
             ORDER BY h.version LIMIT $4",
        )
        .bind(tenant)
        .bind(contract)
        .bind(after)
        .bind(limit)
        .fetch_all(&mut *self.tx)
        .await?)
    }

    pub async fn contract_history_event(
        &mut self,
        tenant: Uuid,
        contract: Uuid,
        event: Uuid,
    ) -> Result<StoredContractHistory, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT h.event_id,h.contract_id,h.version,h.actor_id,u.email AS actor_email,h.action,h.origin,
                    h.before_title,h.after_title,h.reason,h.occurred_at,h.recorded_at
             FROM canteiro.contract_history h JOIN canteiro_auth.users u ON u.id=h.actor_id
             WHERE h.tenant_id=$1 AND h.contract_id=$2 AND h.event_id=$3",
        )
        .bind(tenant)
        .bind(contract)
        .bind(event)
        .fetch_one(&mut *self.tx)
        .await?)
    }

    pub async fn contract_lease_until(
        &mut self,
        tenant: Uuid,
        contract: Uuid,
    ) -> Result<Option<DateTime<Utc>>, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "SELECT e.expires_at FROM canteiro.contract_edit_leases e JOIN canteiro_auth.sessions s
               ON s.id=e.session_id AND s.user_id=e.actor_id
             WHERE e.tenant_id=$1 AND e.contract_id=$2 AND e.expires_at>clock_timestamp()
               AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp()",
        )
        .bind(tenant)
        .bind(contract)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    pub async fn acquire_contract_lease(
        &mut self,
        l: &EditLease,
    ) -> Result<Option<DateTime<Utc>>, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO canteiro.contract_edit_leases AS e
               (tenant_id,contract_id,actor_id,session_id,lease_id,expires_at)
             VALUES($1,$2,$3,$4,$5,clock_timestamp()+interval '2 minutes')
             ON CONFLICT(tenant_id,contract_id) DO UPDATE
               SET actor_id=$3,session_id=$4,lease_id=$5,
                   expires_at=CASE WHEN e.actor_id=$3 AND e.session_id=$4 AND e.lease_id=$5
                       AND e.expires_at>clock_timestamp() THEN e.expires_at ELSE EXCLUDED.expires_at END
             WHERE (e.actor_id=$3 AND e.session_id=$4 AND e.lease_id=$5)
                OR e.expires_at<=clock_timestamp()
                OR NOT EXISTS (SELECT 1 FROM canteiro_auth.sessions s
                   WHERE s.id=e.session_id AND s.user_id=e.actor_id AND s.revoked_at IS NULL
                     AND s.expires_at>clock_timestamp())
             RETURNING expires_at",
        )
        .bind(l.tenant)
        .bind(l.contract)
        .bind(l.actor)
        .bind(l.session)
        .bind(l.key)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    pub async fn renew_contract_lease(
        &mut self,
        l: &EditLease,
    ) -> Result<Option<DateTime<Utc>>, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "UPDATE canteiro.contract_edit_leases SET expires_at=clock_timestamp()+interval '2 minutes'
             WHERE tenant_id=$1 AND contract_id=$2 AND actor_id=$3 AND session_id=$4 AND lease_id=$5
               AND expires_at>clock_timestamp() RETURNING expires_at",
        )
        .bind(l.tenant)
        .bind(l.contract)
        .bind(l.actor)
        .bind(l.session)
        .bind(l.key)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    pub async fn release_contract_lease(&mut self, l: &EditLease) -> Result<(), AuthStoreError> {
        sqlx::query(
            "DELETE FROM canteiro.contract_edit_leases
             WHERE tenant_id=$1 AND contract_id=$2 AND actor_id=$3 AND session_id=$4 AND lease_id=$5",
        )
        .bind(l.tenant)
        .bind(l.contract)
        .bind(l.actor)
        .bind(l.session)
        .bind(l.key)
        .execute(&mut *self.tx)
        .await?;
        Ok(())
    }

    pub async fn revise_contract(
        &mut self,
        revision: &ContractRevision<'_>,
    ) -> Result<Option<Uuid>, AuthStoreError> {
        let l = revision.lease;
        Ok(sqlx::query_scalar(
            "SELECT canteiro.rename_contract_reference($1,$2,$3,$4,$5,$6,$7,$8,$9)",
        )
        .bind(l.tenant)
        .bind(l.contract)
        .bind(l.actor)
        .bind(l.session)
        .bind(l.key)
        .bind(revision.expected_version)
        .bind(revision.title)
        .bind(revision.reason)
        .bind(revision.occurred_at)
        .fetch_one(&mut *self.tx)
        .await?)
    }
}
