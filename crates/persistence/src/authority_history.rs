//! Fatos de alçada ligados às versões imutáveis dos pedidos.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct StoredAuthorityEvent {
    pub event_id: Uuid,
    pub scope_id: Uuid,
    pub request_id: Uuid,
    pub request_version: i64,
    pub actor_id: Uuid,
    pub event_kind: String,
    pub after_status: String,
    pub after_valid_approvals: i16,
    pub grant_id: Option<Uuid>,
    pub recorded_at: DateTime<Utc>,
}

pub struct NewAuthorityEvent<'a> {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub scope_id: Uuid,
    pub request_id: Uuid,
    pub request_version: i64,
    pub event_kind: &'static str,
    pub before: Option<AuthorityBefore<'a>>,
    pub after_status: &'a str,
    pub after_valid_approvals: i16,
    pub grant_id: Option<Uuid>,
    pub reason: &'static str,
}

pub struct AuthorityBefore<'a> {
    pub version: i64,
    pub status: &'a str,
    pub valid_approvals: i16,
}

impl AuthTransaction {
    pub async fn authority_event(
        &mut self,
        tenant: Uuid,
        event_id: Uuid,
        actor: Uuid,
    ) -> Result<Option<StoredAuthorityEvent>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT event_id,scope_id,request_id,request_version,actor_id,event_kind,after_status,after_valid_approvals,grant_id,recorded_at
            FROM canteiro.authority_history WHERE tenant_id=$1 AND event_id=$2 AND actor_id=$3")
            .bind(tenant).bind(event_id).bind(actor).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn append_authority_event(
        &mut self,
        event: NewAuthorityEvent<'_>,
    ) -> Result<StoredAuthorityEvent, AuthStoreError> {
        Ok(sqlx::query_as("INSERT INTO canteiro.authority_history
            (tenant_id,event_id,actor_id,scope_id,request_id,request_version,event_kind,before_version,
             before_status,before_valid_approvals,after_status,after_valid_approvals,grant_id,reason)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            RETURNING event_id,scope_id,request_id,request_version,actor_id,event_kind,after_status,after_valid_approvals,grant_id,recorded_at")
            .bind(event.tenant_id).bind(Uuid::new_v4()).bind(event.actor_id).bind(event.scope_id)
            .bind(event.request_id).bind(event.request_version).bind(event.event_kind)
            .bind(event.before.as_ref().map(|before| before.version))
            .bind(event.before.as_ref().map(|before| before.status))
            .bind(event.before.as_ref().map(|before| before.valid_approvals))
            .bind(event.after_status).bind(event.after_valid_approvals).bind(event.grant_id)
            .bind(event.reason).fetch_one(&mut *self.tx).await?)
    }
}
