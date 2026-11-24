//! Caixa de entrada autorizada por sessão, empresa e concessão de cada escopo.
use super::{
    projects::{RecordAccess, record_conflict, record_denied},
    *,
};
use canteiro_contracts::{access::AccessAction, notifications::*};
use canteiro_persistence::notifications::StoredNotice;

fn notice_view(row: StoredNotice) -> Result<NoticeEntry, AuthError> {
    let (kind, destination) = match row.kind.as_str() {
        "technical_obligation" => (
            NoticeKind::TechnicalObligation,
            format!("/obrigacoes?obligation={}", row.target_id),
        ),
        "invitation" => (NoticeKind::Invitation, "/organizacoes".into()),
        "approval" => (NoticeKind::Approval, "/permissoes".into()),
        "deadline" => (NoticeKind::Deadline, "/carteira".into()),
        "assigned_task" => (NoticeKind::AssignedTask, "/projetos".into()),
        _ => return Err(AuthError::unavailable()),
    };
    let state = match row.state.as_str() {
        "pending" => NoticeState::Pending,
        "communicated" => NoticeState::Communicated,
        "failed" => NoticeState::Failed,
        "resolved" => NoticeState::Resolved,
        _ => return Err(AuthError::unavailable()),
    };
    Ok(NoticeEntry {
        event_id: row.event_id.to_string(),
        kind,
        scope_id: row.scope_id.to_string(),
        due_on: row.due_on.map(|d| d.to_string()),
        state,
        attempts: row
            .attempts
            .try_into()
            .map_err(|_| AuthError::unavailable())?,
        created_at: row.created_at.to_rfc3339(),
        destination,
    })
}
fn preference_view(
    value: Option<canteiro_persistence::notifications::StoredNoticePreferences>,
) -> Result<NoticePreferencesView, AuthError> {
    match value {
        Some(value) => Ok(NoticePreferencesView {
            version: Some(value.version),
            settings: NoticePreferences {
                mute_optional_email: value.mute_optional_email,
                group_by_day: value.group_by_day,
                reminder_days: value
                    .reminder_days
                    .into_iter()
                    .map(|day| u16::try_from(day).map_err(|_| AuthError::unavailable()))
                    .collect::<Result<_, _>>()?,
            },
        }),
        None => Ok(NoticePreferencesView {
            version: None,
            settings: NoticePreferences {
                mute_optional_email: false,
                group_by_day: true,
                reminder_days: vec![1, 7],
            },
        }),
    }
}
fn permitted_scopes(a: &RecordAccess, now: DateTime<Utc>) -> Vec<Uuid> {
    a.snapshot
        .scopes
        .iter()
        .filter(|scope| a.permits(scope.id, AccessAction::View, now))
        .map(|scope| scope.id)
        .collect()
}
impl AuthService {
    pub async fn notices_context(&self, token: &str) -> Result<NoticesContext, AuthError> {
        self.limit("notices-context", token, 120, 60).await?;
        let (mut tx, a) = self.record_access(token, None).await?;
        let mut result = NoticesContext {
            context: None,
            organization_name: String::new(),
            csrf: self.csrf(token),
            preferences: preference_view(None)?,
            notices: vec![],
        };
        if let Some(a) = a {
            let scopes = permitted_scopes(&a, tx.now().await?);
            let notices = tx.user_notice_list(a.tenant, a.actor.id, &scopes).await?;
            result.notices = notices
                .into_iter()
                .map(notice_view)
                .collect::<Result<_, _>>()?;
            result.preferences =
                preference_view(tx.user_notice_preferences(a.tenant, a.actor.id).await?)?;
            self.locked(&mut tx, token).await?;
            let final_now = tx.now().await?;
            if a.member_until.is_some_and(|until| until <= final_now)
                || scopes
                    .iter()
                    .any(|scope| !a.permits(*scope, AccessAction::View, final_now))
            {
                return Err(record_denied());
            }
            result.context = Some(a.context);
            result.organization_name = a.organization_name;
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn notices_set_preferences(
        &self,
        token: &str,
        input: NoticePreferencesCommand,
    ) -> Result<NoticePreferencesView, AuthError> {
        self.limit("notices-preferences", token, 30, 60).await?;
        input.settings.validate().map_err(|_| {
            invalid("Confira os dias de antecedência em ordem crescente, entre zero e 365.")
        })?;
        let allowed = [0, 1, 2, 3, 5, 7, 10, 14, 21, 30, 45, 60, 90, 180, 365];
        if input
            .settings
            .reminder_days
            .iter()
            .any(|day| !allowed.contains(day))
        {
            return Err(invalid(
                "Escolha dias de lembrete entre as opções disponíveis.",
            ));
        }
        let (mut tx, a) = self.record_access(token, Some(&input.context)).await?;
        let a = a.ok_or_else(record_denied)?;
        let now = tx.now().await?;
        if a.member_until.is_some_and(|until| until <= now) {
            return Err(record_denied());
        }
        let days = input
            .settings
            .reminder_days
            .iter()
            .map(|day| *day as i16)
            .collect::<Vec<_>>();
        let saved = tx
            .user_notice_set_preferences(
                a.tenant,
                a.actor.id,
                input.expected_version,
                input.settings.mute_optional_email,
                input.settings.group_by_day,
                &days,
            )
            .await?
            .ok_or_else(record_conflict)?;
        tx.org_audit(
            a.tenant,
            a.actor.id,
            "notice_preferences_changed",
            Some(a.actor.id),
            "Preferências pessoais atualizadas; avisos essenciais continuam ativos",
        )
        .await?;
        self.locked(&mut tx, token).await?;
        let now = tx.now().await?;
        if a.member_until.is_some_and(|until| until <= now) {
            return Err(record_denied());
        }
        let view = preference_view(Some(saved))?;
        tx.commit().await?;
        Ok(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn technical_notice_links_to_the_target_not_the_event_or_scope() {
        let target = Uuid::new_v4();
        let entry = notice_view(StoredNotice {
            event_id: Uuid::new_v4(),
            kind: "technical_obligation".into(),
            scope_id: Uuid::new_v4(),
            target_id: target,
            due_on: None,
            essential: true,
            state: "pending".into(),
            attempts: 0,
            created_at: Utc::now(),
        })
        .unwrap_or_else(|_| panic!("aviso válido"));
        assert_eq!(
            entry.destination,
            format!("/obrigacoes?obligation={target}")
        );
        assert_eq!(entry.state, NoticeState::Pending);
    }
}
