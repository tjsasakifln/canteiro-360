//! Orçamentos são snapshots com CAS, reserva ligada à sessão e histórico imutável.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredBudgetVersion {
    pub scope_id: Uuid,
    pub version: i64,
    pub payload: String,
    pub state: String,
    pub actor_id: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: Vec<u8>,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
pub struct BudgetWrite<'a> {
    pub tenant: Uuid,
    pub scope: Uuid,
    pub expected: Option<i64>,
    pub actor: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: &'a [u8],
    pub reason: &'a str,
    pub payload: &'a str,
    pub state: &'a str,
}
impl AuthTransaction {
    pub async fn budget_current(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
    ) -> Result<Option<StoredBudgetVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.version,v.payload::TEXT AS payload,v.state,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.created_at
             FROM canteiro.budget_records b JOIN canteiro.budget_versions v
               ON v.tenant_id=b.tenant_id AND v.scope_id=b.scope_id
               AND v.version=b.current_version
             WHERE b.tenant_id=$1 AND b.scope_id=$2 FOR UPDATE OF b",
        )
        .bind(tenant)
        .bind(scope)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn budget_list(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredBudgetVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.version,v.payload::TEXT AS payload,v.state,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.created_at
             FROM canteiro.budget_records b JOIN canteiro.budget_versions v
             ON v.tenant_id=b.tenant_id AND v.scope_id=b.scope_id
             AND v.version=b.current_version
             WHERE b.tenant_id=$1 AND b.scope_id=ANY($2)
             ORDER BY b.scope_id LIMIT 100",
        )
        .bind(tenant)
        .bind(scopes)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    pub async fn budget_history(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
    ) -> Result<Vec<StoredBudgetVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT scope_id,version,payload::TEXT AS payload,state,actor_id,
             operation_key,fingerprint,reason,created_at
             FROM canteiro.budget_versions WHERE tenant_id=$1 AND scope_id=$2
             ORDER BY version DESC LIMIT 200",
        )
        .bind(tenant)
        .bind(scope)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    pub async fn budget_operation(
        &mut self,
        tenant: Uuid,
        key: Uuid,
    ) -> Result<Option<StoredBudgetVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT scope_id,version,payload::TEXT AS payload,state,actor_id,
             operation_key,fingerprint,reason,created_at
             FROM canteiro.budget_versions
             WHERE tenant_id=$1 AND operation_key=$2",
        )
        .bind(tenant)
        .bind(key)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn budget_acquire_lease(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        key: Uuid,
        next_version: i64,
    ) -> Result<Option<DateTime<Utc>>, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO canteiro.budget_edit_leases AS e
             (tenant_id,scope_id,actor_id,session_id,lease_id,version,expires_at)
             VALUES($1,$2,$3,$4,$5,$6,clock_timestamp()+interval '2 minutes')
             ON CONFLICT (tenant_id,scope_id) DO UPDATE
             SET actor_id=$3,session_id=$4,lease_id=$5,version=$6,
                 expires_at=CASE
                  WHEN e.actor_id=$3 AND e.session_id=$4 AND e.lease_id=$5
                     AND e.expires_at>clock_timestamp() THEN e.expires_at
                  ELSE EXCLUDED.expires_at END
             WHERE (e.actor_id=$3 AND e.session_id=$4 AND e.lease_id=$5 AND e.version=$6)
                OR e.expires_at<=clock_timestamp()
                OR NOT EXISTS (
                  SELECT 1 FROM canteiro_auth.sessions s
                  WHERE s.id=e.session_id AND s.user_id=e.actor_id
                  AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp())
             RETURNING expires_at",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(key)
        .bind(next_version)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn budget_lease_usable(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        key: Uuid,
        next_version: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM canteiro.budget_edit_leases e
             JOIN canteiro_auth.sessions s ON s.id=e.session_id AND s.user_id=e.actor_id
             WHERE e.tenant_id=$1 AND e.scope_id=$2 AND e.actor_id=$3
             AND e.session_id=$4 AND e.lease_id=$5 AND e.version=$6
             AND e.expires_at>clock_timestamp()
             AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp())",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(key)
        .bind(next_version)
        .fetch_one(&mut *self.tx)
        .await?)
    }
    pub async fn budget_release_lease(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
        session: Uuid,
        key: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            "DELETE FROM canteiro.budget_edit_leases
             WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3 AND session_id=$4
             AND lease_id=$5",
        )
        .bind(tenant)
        .bind(scope)
        .bind(actor)
        .bind(session)
        .bind(key)
        .execute(&mut *self.tx)
        .await?;
        Ok(())
    }
    pub async fn budget_write(
        &mut self,
        w: &BudgetWrite<'_>,
    ) -> Result<Option<StoredBudgetVersion>, AuthStoreError> {
        let next = if let Some(expected) = w.expected {
            match expected.checked_add(1) {
                Some(n) => n,
                None => return Ok(None),
            }
        } else {
            1
        };
        let count = if let Some(previous) = w.expected {
            sqlx::query(
                "UPDATE canteiro.budget_records SET current_version=$4
                 WHERE tenant_id=$1 AND scope_id=$2 AND current_version=$3",
            )
            .bind(w.tenant)
            .bind(w.scope)
            .bind(previous)
            .bind(next)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "INSERT INTO canteiro.budget_records(tenant_id,scope_id,current_version)
                 VALUES($1,$2,1) ON CONFLICT DO NOTHING",
            )
            .bind(w.tenant)
            .bind(w.scope)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
        };
        if count != 1 {
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO canteiro.budget_versions(
             tenant_id,scope_id,version,payload,state,policy,actor_id,
             operation_key,fingerprint,reason)
             VALUES($1,$2,$3,$4::JSONB,$5,'C360-DECIMAL-1',$6,$7,$8,$9)",
        )
        .bind(w.tenant)
        .bind(w.scope)
        .bind(next)
        .bind(w.payload)
        .bind(w.state)
        .bind(w.actor)
        .bind(w.operation_key)
        .bind(w.fingerprint)
        .bind(w.reason)
        .execute(&mut *self.tx)
        .await?;
        self.budget_current(w.tenant, w.scope).await
    }
}
