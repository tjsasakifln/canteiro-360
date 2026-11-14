use canteiro_contracts::{
    forms::parse_date_pt_br,
    reliability::{ContractSummary, ReliabilityCommand, ReliabilityScope},
};
use chrono::{NaiveDate, NaiveTime, TimeZone};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Confirmation {
    Unconfirmed,
    Absent,
    KeyConflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Pending {
    pub key: String,
    pub confirmation: Confirmation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Draft {
    pub base_title: String,
    pub base_version: i64,
    pub title: String,
    pub reason: String,
    pub occurred_at: String,
    pub edited_at: Option<String>,
    pub pending: Option<Pending>,
}

impl Draft {
    pub fn new(contract: &ContractSummary) -> Self {
        Self {
            base_title: contract.title.clone(),
            base_version: contract.version,
            title: contract.title.clone(),
            reason: String::new(),
            occurred_at: String::new(),
            edited_at: None,
            pending: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.title != self.base_title
            || !self.reason.is_empty()
            || !self.occurred_at.is_empty()
            || self.pending.is_some()
    }

    pub fn rebase(&mut self, contract: &ContractSummary) {
        self.base_title.clone_from(&contract.title);
        self.base_version = contract.version;
    }

    pub fn save_command(
        &self,
        scope: ReliabilityScope,
        contract_id: String,
        lease_id: String,
        key: String,
    ) -> Result<ReliabilityCommand, FieldError> {
        let title = self.title.trim();
        let reason = self.reason.trim();
        if !valid_text(title, 160) {
            return Err(FieldError {
                field: "history-title",
                message: "Informe um título com 2 a 160 caracteres.",
            });
        }
        if !valid_text(reason, 200) {
            return Err(FieldError {
                field: "history-reason",
                message: "Informe um motivo com 2 a 200 caracteres, em uma linha.",
            });
        }
        let occurred_at = if self.occurred_at.trim().is_empty() {
            None
        } else {
            Some(local_instant(&self.occurred_at).ok_or(FieldError {
                field: "history-occurred",
                message: "Confira a data e a hora da ocorrência no horário de Brasília.",
            })?)
        };
        Ok(ReliabilityCommand::Save {
            scope,
            contract_id,
            lease_id,
            key,
            expected_version: self.base_version,
            title: title.to_owned(),
            reason: reason.to_owned(),
            occurred_at,
        })
    }
}

fn valid_text(value: &str, max: usize) -> bool {
    (2..=max).contains(&value.chars().count()) && !value.chars().any(char::is_control)
}

fn local_instant(value: &str) -> Option<String> {
    let (date, time) = value.trim().split_once(' ')?;
    let date = parse_date_pt_br(date)
        .ok()?
        .canonical
        .parse::<NaiveDate>()
        .ok()?;
    let pattern = match time.len() {
        5 => "%H:%M",
        8 => "%H:%M:%S",
        _ => return None,
    };
    let time = NaiveTime::parse_from_str(time, pattern).ok()?;
    chrono_tz::America::Sao_Paulo
        .from_local_datetime(&date.and_time(time))
        .single()
        .map(|date| date.to_rfc3339())
}

pub(super) fn instant(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%d/%m/%Y às %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| "Data indisponível".into())
}

#[derive(Debug)]
pub(super) struct FieldError {
    pub field: &'static str,
    pub message: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract(version: i64, title: &str) -> ContractSummary {
        ContractSummary {
            id: "contrato".into(),
            title: title.into(),
            version,
            can_edit: true,
        }
    }

    #[test]
    fn rebase_only_moves_the_base_of_a_preserved_draft() {
        let mut draft = Draft::new(&contract(1, "Original"));
        draft.title = "Meu rascunho".into();
        draft.reason = "Motivo registrado".into();
        draft.edited_at = Some("2026-11-14T12:00:00Z".into());

        draft.rebase(&contract(2, "Salvo pela outra sessão"));
        assert_eq!(draft.title, "Meu rascunho");
        assert_eq!(draft.reason, "Motivo registrado");
        assert_eq!(draft.edited_at.as_deref(), Some("2026-11-14T12:00:00Z"));
        assert_eq!(draft.base_title, "Salvo pela outra sessão");
        assert_eq!(draft.base_version, 2);
    }

    #[test]
    fn command_retries_can_keep_identity_across_context_and_lease_changes() {
        let mut draft = Draft::new(&contract(3, "Original"));
        draft.title = "  Novo título ".into();
        draft.reason = "  Registro retroativo ".into();
        draft.occurred_at = "06/11/2026 09:30".into();
        let send = |context_version, lease_id: &str| {
            draft
                .save_command(
                    ReliabilityScope {
                        organization_id: "empresa".into(),
                        context_version,
                    },
                    "contrato".into(),
                    lease_id.into(),
                    "chave-da-tentativa".into(),
                )
                .unwrap()
        };
        let first = serde_json::to_value(send(5, "reserva1")).unwrap();
        let second = serde_json::to_value(send(7, "reserva2")).unwrap();
        for field in [
            "contract_id",
            "key",
            "expected_version",
            "title",
            "reason",
            "occurred_at",
        ] {
            assert_eq!(first[field], second[field], "{field}");
        }
        assert_eq!(first["title"], "Novo título");
        assert_eq!(first["occurred_at"], "2026-11-06T09:30:00-03:00");
    }

    #[test]
    fn historical_times_keep_seconds_and_reject_ambiguous_local_times() {
        assert_eq!(
            local_instant("14/11/2026 09:30:21").as_deref(),
            Some("2026-11-14T09:30:21-03:00")
        );
        assert!(local_instant("04/11/2018 00:30").is_none());
        assert!(local_instant("30/02/2026 09:30").is_none());
        assert!(local_instant("11/14/2026 09:30").is_none());
        assert_eq!(instant("2026-11-14T12:30:21Z"), "14/11/2026 às 09:30:21");
    }
}
