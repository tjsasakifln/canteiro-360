//! Pendências e comunicação são fatos distintos. Links nunca autorizam decisões.
use crate::{ActorId, CivilDate, EntityId, OperationId, TenantId, UtcInstant};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeKind {
    TechnicalObligation,
    Invitation,
    Approval,
    Deadline,
    AssignedTask,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoticeState {
    Pending,
    Communicated,
    Failed,
    Resolved,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationChannel {
    Internal,
    Email,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoticeIdentity {
    pub tenant: TenantId,
    pub recipient: ActorId,
    pub event: OperationId,
    pub kind: NoticeKind,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    pub identity: NoticeIdentity,
    pub scope: EntityId,
    pub target: EntityId,
    pub due_on: Option<CivilDate>,
    pub essential: bool,
    pub state: NoticeState,
    pub attempts: u32,
    pub last_attempt: Option<UtcInstant>,
    pub resolved_at: Option<UtcInstant>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoticePreferences {
    pub mute_optional_email: bool,
    pub group_by_day: bool,
    pub reminder_days: Vec<u16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum NoticeError {
    #[error("Este aviso pertence a outra empresa, pessoa ou destino.")]
    Unauthorized,
    #[error("Este ato requer autenticação e envio expresso; abrir um link não o executa.")]
    ReadOnlyLink,
    #[error("Um aviso resolvido não pode voltar a pendente.")]
    Resolved,
    #[error("Limite de tentativas excedido.")]
    Attempts,
    #[error("As preferências de lembrete estão fora do limite.")]
    Preferences,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryEffect {
    AlreadySeen,
    Created,
}
#[derive(Clone, Debug, Default)]
pub struct Inbox {
    entries: BTreeMap<NoticeIdentity, Notice>,
}
impl Inbox {
    /// Mesmo evento, empresa, destinatário e tipo nunca criam duas pendências.
    pub fn record(&mut self, notice: Notice) -> Result<DeliveryEffect, NoticeError> {
        if let Some(existing) = self.entries.get(&notice.identity) {
            if existing.scope != notice.scope
                || existing.target != notice.target
                || existing.due_on != notice.due_on
                || existing.essential != notice.essential
            {
                return Err(NoticeError::Unauthorized);
            }
            return Ok(DeliveryEffect::AlreadySeen);
        }
        self.entries.insert(notice.identity.clone(), notice);
        Ok(DeliveryEffect::Created)
    }
    pub fn for_recipient(&self, tenant: TenantId, recipient: ActorId) -> Vec<&Notice> {
        let mut rows: Vec<_> = self
            .entries
            .values()
            .filter(|row| row.identity.tenant == tenant && row.identity.recipient == recipient)
            .collect();
        rows.sort_by(|a, b| a.due_on.cmp(&b.due_on).then(a.identity.cmp(&b.identity)));
        rows
    }
    pub fn get_mut(&mut self, key: &NoticeIdentity) -> Option<&mut Notice> {
        self.entries.get_mut(key)
    }
}
impl NoticePreferences {
    pub fn validate(&self) -> Result<(), NoticeError> {
        if self.reminder_days.len() > 10
            || self.reminder_days.iter().any(|d| *d > 365)
            || self.reminder_days.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(NoticeError::Preferences);
        }
        Ok(())
    }
}
impl Notice {
    pub fn notify_by(
        &self,
        channel: NotificationChannel,
        preferences: &NoticePreferences,
    ) -> Result<bool, NoticeError> {
        preferences.validate()?;
        Ok(channel == NotificationChannel::Internal
            || self.essential
            || !preferences.mute_optional_email)
    }

    /// A falha do provedor jamais elimina o fato nem transforma "registrado" em "entregue".
    pub fn record_attempt(
        &mut self,
        when: UtcInstant,
        acknowledged: bool,
    ) -> Result<(), NoticeError> {
        if self.state == NoticeState::Resolved {
            return Err(NoticeError::Resolved);
        }
        self.attempts = self.attempts.checked_add(1).ok_or(NoticeError::Attempts)?;
        self.last_attempt = Some(when);
        self.state = if acknowledged {
            NoticeState::Communicated
        } else {
            NoticeState::Failed
        };
        Ok(())
    }

    pub fn resolve(&mut self, when: UtcInstant) {
        self.state = NoticeState::Resolved;
        self.resolved_at = Some(when);
    }

    /// Links carregam só identidade opaca, sem detalhes de orçamento ou obrigação.
    /// A revalidação é repetida no servidor antes de exibir ou alterar o alvo.
    pub fn authorize_target(
        &self,
        tenant: TenantId,
        recipient: ActorId,
        scope: EntityId,
        authenticated: bool,
        current_grant: bool,
        is_get: bool,
    ) -> Result<(), NoticeError> {
        if !authenticated
            || !current_grant
            || self.identity.tenant != tenant
            || self.identity.recipient != recipient
            || self.scope != scope
        {
            return Err(NoticeError::Unauthorized);
        }
        if !is_get {
            // Uma alteração exige comando autenticado e protegido contra repetição,
            // não é realizada por esta função de navegação.
            return Err(NoticeError::ReadOnlyLink);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn id(n: u128) -> String {
        format!("{n:032x}")
    }
    fn tenant(n: u128) -> TenantId {
        id(n).parse().unwrap()
    }
    fn actor(n: u128) -> ActorId {
        id(n).parse().unwrap()
    }
    fn entity(n: u128) -> EntityId {
        id(n).parse().unwrap()
    }
    fn event(n: u128) -> OperationId {
        id(n).parse().unwrap()
    }
    fn date() -> UtcInstant {
        "2026-11-21T15:00:00Z".parse().unwrap()
    }
    fn notice() -> Notice {
        Notice {
            identity: NoticeIdentity {
                tenant: tenant(1),
                recipient: actor(2),
                event: event(3),
                kind: NoticeKind::TechnicalObligation,
            },
            scope: entity(4),
            target: entity(5),
            due_on: Some("2026-12-01".parse().unwrap()),
            essential: true,
            state: NoticeState::Pending,
            attempts: 0,
            last_attempt: None,
            resolved_at: None,
        }
    }
    #[test]
    fn repeated_event_is_one_action_and_provider_failure_preserves_it() {
        let mut inbox = Inbox::default();
        assert_eq!(inbox.record(notice()).unwrap(), DeliveryEffect::Created);
        assert_eq!(inbox.record(notice()).unwrap(), DeliveryEffect::AlreadySeen);
        assert_eq!(inbox.for_recipient(tenant(1), actor(2)).len(), 1);
        let n = inbox.get_mut(&notice().identity).unwrap();
        n.record_attempt(date(), false).unwrap();
        assert_eq!(n.state, NoticeState::Failed);
        assert_eq!(n.attempts, 1);
        n.record_attempt(date(), true).unwrap();
        assert_eq!(n.state, NoticeState::Communicated);
        assert_eq!(n.attempts, 2);
    }
    #[test]
    fn get_does_not_approve_and_revocation_closes_link() {
        let n = notice();
        assert_eq!(
            n.authorize_target(tenant(1), actor(2), entity(4), true, true, true),
            Ok(())
        );
        assert_eq!(
            n.authorize_target(tenant(1), actor(2), entity(4), true, true, false),
            Err(NoticeError::ReadOnlyLink)
        );
        assert_eq!(
            n.authorize_target(tenant(1), actor(2), entity(4), true, false, true),
            Err(NoticeError::Unauthorized)
        );
        assert_eq!(
            n.authorize_target(tenant(2), actor(2), entity(4), true, true, true),
            Err(NoticeError::Unauthorized)
        );
    }
    #[test]
    fn optional_notice_can_be_muted_but_inbox_is_preserved() {
        let mut n = notice();
        n.essential = false;
        let pref = NoticePreferences {
            mute_optional_email: true,
            group_by_day: true,
            reminder_days: vec![1, 7],
        };
        assert!(!n.notify_by(NotificationChannel::Email, &pref).unwrap());
        assert!(n.notify_by(NotificationChannel::Internal, &pref).unwrap());
        n.essential = true;
        assert!(n.notify_by(NotificationChannel::Email, &pref).unwrap());
    }
}
