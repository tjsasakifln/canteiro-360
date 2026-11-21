//! Pendências são fatos persistidos. A outbox não transforma um registro em entregue.
use crate::{
    Database, PersistenceError, PersistenceErrorKind,
    auth::{AuthStoreError, AuthTransaction},
    error::operation_id,
    reliability_outbox::NotificationStep,
};
use chrono::{DateTime, NaiveDate, Utc};
use tokio::time::timeout;
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredNotice {
    pub event_id: Uuid,
    pub kind: String,
    pub scope_id: Uuid,
    pub target_id: Uuid,
    pub due_on: Option<NaiveDate>,
    pub essential: bool,
    pub state: String,
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredNoticePreferences {
    pub mute_optional_email: bool,
    pub group_by_day: bool,
    pub reminder_days: Vec<i16>,
    pub version: i64,
}
pub struct NewNotice<'a> {
    pub tenant: Uuid,
    pub recipient: Uuid,
    pub event: Uuid,
    pub kind: &'a str,
    pub scope: Uuid,
    pub target: Uuid,
    pub due: Option<NaiveDate>,
    pub essential: bool,
}
impl AuthTransaction {
    /// RLS separa empresas e destinatários. O serviço verifica os escopos antes e depois.
    pub async fn user_notice_list(
        &mut self,
        tenant: Uuid,
        recipient: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredNotice>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT event_id,kind,scope_id,target_id,due_on,essential,state,attempts,created_at
           FROM canteiro.user_notices
           WHERE tenant_id=$1 AND recipient_id=$2 AND scope_id=ANY($3)
           ORDER BY (state='resolved') ASC,due_on ASC NULLS LAST,created_at DESC
           LIMIT 200",
        )
        .bind(tenant)
        .bind(recipient)
        .bind(scopes)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    pub async fn user_notice_record(
        &mut self,
        notice: &NewNotice<'_>,
    ) -> Result<bool, AuthStoreError> {
        let inserted = sqlx::query(
            "INSERT INTO canteiro.user_notices
          (tenant_id,recipient_id,event_id,kind,scope_id,target_id,due_on,essential)
          VALUES($1,$2,$3,$4,$5,$6,$7,$8)
          ON CONFLICT(tenant_id,recipient_id,event_id,kind) DO NOTHING",
        )
        .bind(notice.tenant)
        .bind(notice.recipient)
        .bind(notice.event)
        .bind(notice.kind)
        .bind(notice.scope)
        .bind(notice.target)
        .bind(notice.due)
        .bind(notice.essential)
        .execute(&mut *self.tx)
        .await?
        .rows_affected()
            == 1;
        if inserted {
            sqlx::query(
                "INSERT INTO canteiro.user_notice_outbox
              (tenant_id,recipient_id,event_id,kind,channel)
              VALUES($1,$2,$3,$4,'internal')
              ON CONFLICT DO NOTHING",
            )
            .bind(notice.tenant)
            .bind(notice.recipient)
            .bind(notice.event)
            .bind(notice.kind)
            .execute(&mut *self.tx)
            .await?;
        }
        Ok(inserted)
    }
    /// Uma confirmação válida resolve os avisos da obrigação para o autor.
    pub async fn user_notice_resolve_for_target(
        &mut self,
        tenant: Uuid,
        recipient: Uuid,
        target: Uuid,
    ) -> Result<u64, AuthStoreError> {
        Ok(sqlx::query(
            "UPDATE canteiro.user_notices
           SET state='resolved',resolved_at=clock_timestamp()
           WHERE tenant_id=$1 AND recipient_id=$2 AND target_id=$3
             AND kind='technical_obligation' AND state<>'resolved'",
        )
        .bind(tenant)
        .bind(recipient)
        .bind(target)
        .execute(&mut *self.tx)
        .await?
        .rows_affected())
    }
    pub async fn user_notice_preferences(
        &mut self,
        tenant: Uuid,
        recipient: Uuid,
    ) -> Result<Option<StoredNoticePreferences>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT mute_optional_email,group_by_day,reminder_days,version
           FROM canteiro.user_notice_preferences
           WHERE tenant_id=$1 AND recipient_id=$2",
        )
        .bind(tenant)
        .bind(recipient)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    /// Comparação de versão impede que duas abas sobrescrevam preferências.
    pub async fn user_notice_set_preferences(
        &mut self,
        tenant: Uuid,
        recipient: Uuid,
        expected: Option<i64>,
        muted: bool,
        grouped: bool,
        days: &[i16],
    ) -> Result<Option<StoredNoticePreferences>, AuthStoreError> {
        if let Some(version) = expected {
            Ok(sqlx::query_as(
                "UPDATE canteiro.user_notice_preferences
               SET mute_optional_email=$4,group_by_day=$5,reminder_days=$6,version=version+1
               WHERE tenant_id=$1 AND recipient_id=$2 AND version=$3
               RETURNING mute_optional_email,group_by_day,reminder_days,version",
            )
            .bind(tenant)
            .bind(recipient)
            .bind(version)
            .bind(muted)
            .bind(grouped)
            .bind(days)
            .fetch_optional(&mut *self.tx)
            .await?)
        } else {
            Ok(sqlx::query_as(
                "INSERT INTO canteiro.user_notice_preferences
               (tenant_id,recipient_id,mute_optional_email,group_by_day,reminder_days)
               VALUES($1,$2,$3,$4,$5)
               ON CONFLICT DO NOTHING
               RETURNING mute_optional_email,group_by_day,reminder_days,version",
            )
            .bind(tenant)
            .bind(recipient)
            .bind(muted)
            .bind(grouped)
            .bind(days)
            .fetch_optional(&mut *self.tx)
            .await?)
        }
    }
}

impl Database {
    /// Um passo de entrega interna, atômico com a atualização do histórico.
    /// Falha de conexão durante o commit não gera dois recibos no reprocessamento.
    pub async fn consume_user_notice_delivery(
        &self,
    ) -> Result<NotificationStep, PersistenceError> {
        let operation = operation_id();
        let status = timeout(
            self.limits.transaction,
            sqlx::query_scalar::<_, String>("SELECT canteiro_meta.consume_user_notice()")
                .fetch_one(&self.pool),
        )
        .await
        .map_err(|_| PersistenceError::new(PersistenceErrorKind::Timeout, operation))?
        .map_err(|error| PersistenceError::from_sql(&error, operation))?;
        match status.as_str() {
            "idle" => Ok(NotificationStep::Idle),
            "delivered" => Ok(NotificationStep::Delivered),
            "deferred" => Ok(NotificationStep::Deferred),
            _ => Err(PersistenceError::new(PersistenceErrorKind::Internal, operation)),
        }
    }
}
