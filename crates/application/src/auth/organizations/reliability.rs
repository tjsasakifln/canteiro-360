//! A sessão, a organização e o vínculo são revalidados antes de ler ou gravar.
use super::access::{Snapshot, zero};
use super::{
    Actor, AuthError, AuthErrorKind, AuthService, authorize, context_changed, crypto, id, invalid,
};
use canteiro_contracts::access::{AccessAction, ScopeKind};
use canteiro_contracts::reliability::*;
use canteiro_domain::{EntityVersion, UtcInstant};
use canteiro_persistence::{
    access::StoredScope,
    auth::AuthTransaction,
    reliability::{ContractRevision, EditLease, OperationReceipt, StoredContractHistory},
};
use chrono::{DateTime, Utc};
use uuid::{Uuid, Version};

const RENAME: &str = "contract_reference_rename";
const HISTORY_PAGE: usize = 50;

pub(in crate::auth) fn idempotency_conflict() -> AuthError {
    conflict(
        "idempotency_conflict",
        "Esta identificação já foi usada em outra gravação. Confira se a alteração anterior foi salva antes de enviar seu rascunho novamente.",
    )
}

fn conflict(code: &'static str, message: &'static str) -> AuthError {
    AuthError::new(AuthErrorKind::Conflict, code, message)
}

fn unavailable_contract() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "contract_unavailable",
        "Esta referência não está disponível. Confira a empresa ativa e peça acesso ao responsável pelo contrato.",
    )
}

fn expired_lease() -> AuthError {
    conflict(
        "edit_expired",
        "Sua reserva de edição terminou. Confira a versão salva e reserve a edição novamente. Seu rascunho foi preservado.",
    )
}

fn random_id(value: &str) -> Result<Uuid, AuthError> {
    let value = id(value)?;
    if value.get_version() != Some(Version::Random) || value.get_variant() != uuid::Variant::RFC4122
    {
        return Err(invalid(
            "A identificação da tentativa é inválida. Atualize a página antes de continuar.",
        ));
    }
    Ok(value)
}

fn view(scope: &StoredScope, can_edit: bool) -> ContractSummary {
    ContractSummary {
        id: scope.id.to_string(),
        title: scope.name.clone(),
        version: scope.version,
        can_edit,
    }
}

fn permits(
    snapshot: &Snapshot,
    actor: &Actor,
    scope: Uuid,
    action: AccessAction,
    at: DateTime<Utc>,
) -> bool {
    snapshot.permits(actor.id, scope, action, zero(), at)
}

fn history_entry(row: StoredContractHistory) -> HistoryEntry {
    HistoryEntry {
        sequence: row.version,
        event_id: row.event_id.to_string(),
        actor_id: row.actor_id.to_string(),
        actor_label: row.actor_email,
        version: row.version,
        origin: row.origin,
        action: row.action,
        before: row.before_title,
        after: row.after_title,
        reason: row.reason,
        occurred_at: row.occurred_at.to_rfc3339(),
        recorded_at: row.recorded_at.to_rfc3339(),
        late: row.occurred_at < row.recorded_at,
    }
}

fn receipt(row: StoredContractHistory) -> SaveReceipt {
    SaveReceipt {
        event_id: row.event_id.to_string(),
        contract_id: row.contract_id.to_string(),
        title: row.after_title,
        version: row.version,
        recorded_at: row.recorded_at.to_rfc3339(),
    }
}

struct ContractAccess {
    tenant: Uuid,
    actor: Actor,
    contract: Uuid,
    summary: ContractSummary,
    now: DateTime<Utc>,
}

impl ContractAccess {
    fn editing(&self, lease: &str) -> Result<EditLease, AuthError> {
        Ok(EditLease {
            tenant: self.tenant,
            contract: self.contract,
            actor: self.actor.id,
            session: self.actor.session,
            key: random_id(lease)?,
        })
    }

    async fn saved(
        &self,
        tx: &mut AuthTransaction,
        stored: &OperationReceipt,
    ) -> Result<SaveReceipt, AuthError> {
        let event = tx
            .contract_history_event(self.tenant, self.contract, stored.result_id)
            .await?;
        if event.actor_id != self.actor.id || event.version != stored.result_version {
            return Err(AuthError::unavailable());
        }
        Ok(receipt(event))
    }
}

impl AuthService {
    pub async fn reliability_context(&self, token: &str) -> Result<ReliabilityContext, AuthError> {
        self.limit("reliability-context", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let selection = tx.org_selection(actor.session).await?;
        let organization = if let Some(tenant) = selection.tenant_id {
            match authorize(&mut tx, &actor, tenant).await {
                Ok((org, member)) => {
                    let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
                    self.locked(&mut tx, token).await?;
                    let now = tx.now().await?;
                    if member.valid_until.is_some_and(|end| end <= now) {
                        return Err(unavailable_contract());
                    }
                    let contracts = snapshot
                        .scopes
                        .iter()
                        .filter(|s| s.kind == ScopeKind::Contract.as_str())
                        .filter(|s| permits(&snapshot, &actor, s.id, AccessAction::View, now))
                        .map(|s| {
                            view(
                                s,
                                permits(
                                    &snapshot,
                                    &actor,
                                    s.id,
                                    AccessAction::EditContractRecord,
                                    now,
                                ),
                            )
                        })
                        .collect();
                    Some(ReliabilityOrganization {
                        scope: ReliabilityScope {
                            organization_id: tenant.to_string(),
                            context_version: selection.generation,
                        },
                        name: org.name,
                        contracts,
                    })
                }
                Err(e) if e.kind() == AuthErrorKind::Forbidden => None,
                Err(e) => return Err(e),
            }
        } else {
            None
        };
        tx.commit().await?;
        Ok(ReliabilityContext {
            csrf: self.csrf(token),
            actor_id: actor.id.to_string(),
            actor_label: actor.email,
            organization,
        })
    }

    async fn contract_access(
        &self,
        token: &str,
        scope: &ReliabilityScope,
        contract: &str,
        edit: bool,
    ) -> Result<(AuthTransaction, ContractAccess), AuthError> {
        self.limit("reliability-contract", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let tenant = id(&scope.organization_id)?;
        let contract = id(contract)?;
        let active = tx.org_selection(actor.session).await?;
        if active.tenant_id != Some(tenant) || active.generation != scope.context_version {
            return Err(context_changed());
        }
        // Mesmo bloqueio de alteração de membros, concessões, delegações e substituições.
        let (_, member) = authorize(&mut tx, &actor, tenant).await?;
        let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
        self.locked(&mut tx, token).await?;
        let now = tx.now().await?;
        let selected = snapshot
            .scopes
            .iter()
            .find(|s| s.id == contract && s.kind == ScopeKind::Contract.as_str())
            .ok_or_else(unavailable_contract)?;
        let can_edit = permits(
            &snapshot,
            &actor,
            contract,
            AccessAction::EditContractRecord,
            now,
        );
        if member.valid_until.is_some_and(|end| end <= now)
            || !permits(&snapshot, &actor, contract, AccessAction::View, now)
            || (edit && !can_edit)
        {
            return Err(unavailable_contract());
        }
        Ok((
            tx,
            ContractAccess {
                tenant,
                actor,
                contract,
                summary: view(selected, can_edit),
                now,
            },
        ))
    }

    pub async fn reliability_detail(
        &self,
        token: &str,
        scope: &ReliabilityScope,
        contract: &str,
        after: Option<i64>,
    ) -> Result<ContractDetail, AuthError> {
        let (mut tx, access) = self.contract_access(token, scope, contract, false).await?;
        let after = after.unwrap_or(0);
        if after < 0 || after > access.summary.version {
            return Err(invalid("Atualize o histórico para continuar a consulta."));
        }
        let mut rows = tx
            .contract_history(
                access.tenant,
                access.contract,
                after,
                HISTORY_PAGE as i64 + 1,
            )
            .await?;
        let next_after = if rows.len() > HISTORY_PAGE {
            rows.pop();
            rows.last().map(|row| row.version)
        } else {
            None
        };
        let lease_until = tx
            .contract_lease_until(access.tenant, access.contract)
            .await?
            .map(|value| value.to_rfc3339());
        let notifications = tx
            .contract_notifications(access.tenant, access.actor.id, access.contract)
            .await?
            .into_iter()
            .map(|n| HistoryNotification {
                event_id: n.event_id.to_string(),
                version: n.version,
                recorded_at: n.recorded_at.to_rfc3339(),
            })
            .collect();
        tx.commit().await?;
        Ok(ContractDetail {
            contract: access.summary,
            lease_until,
            history: rows.into_iter().map(history_entry).collect(),
            next_after,
            notifications,
        })
    }

    pub async fn reliability_history(
        &self,
        token: &str,
        scope: &ReliabilityScope,
        contract: &str,
    ) -> Result<Vec<HistoryEntry>, AuthError> {
        let (mut tx, access) = self.contract_access(token, scope, contract, false).await?;
        let rows = tx
            .contract_history(access.tenant, access.contract, 0, access.summary.version)
            .await?;
        tx.commit().await?;
        Ok(rows.into_iter().map(history_entry).collect())
    }

    pub async fn reliability_operation(
        &self,
        token: &str,
        scope: &ReliabilityScope,
        contract: &str,
        key: &str,
    ) -> Result<Option<SaveReceipt>, AuthError> {
        let (mut tx, access) = self.contract_access(token, scope, contract, true).await?;
        let stored = tx
            .operation_receipt(access.tenant, RENAME, random_id(key)?)
            .await?;
        let result = match stored {
            Some(row) if row.scope_id == access.contract => {
                Some(access.saved(&mut tx, &row).await?)
            }
            _ => None,
        };
        tx.commit().await?;
        Ok(result)
    }

    pub async fn reliability_execute(
        &self,
        token: &str,
        command: ReliabilityCommand,
    ) -> Result<ReliabilityReply, AuthError> {
        use ReliabilityCommand::*;
        let (scope, contract, lease) = match &command {
            Acquire {
                scope,
                contract_id,
                lease_id,
            }
            | Renew {
                scope,
                contract_id,
                lease_id,
            }
            | Release {
                scope,
                contract_id,
                lease_id,
            }
            | Save {
                scope,
                contract_id,
                lease_id,
                ..
            } => (scope, contract_id, lease_id),
        };
        let (mut tx, access) = self.contract_access(token, scope, contract, true).await?;
        let lease = access.editing(lease)?;
        let result = match command {
            Acquire { .. } => {
                let until = tx.acquire_contract_lease(&lease).await?.ok_or_else(|| conflict(
                    "edit_reserved",
                    "Outra sessão reservou a edição. Você pode manter seu rascunho e tentar novamente quando a reserva terminar.",
                ))?;
                ReliabilityReply::Lease {
                    expires_at: Some(until.to_rfc3339()),
                }
            }
            Renew { .. } => {
                let until = tx
                    .renew_contract_lease(&lease)
                    .await?
                    .ok_or_else(expired_lease)?;
                ReliabilityReply::Lease {
                    expires_at: Some(until.to_rfc3339()),
                }
            }
            Release { .. } => {
                tx.release_contract_lease(&lease).await?;
                ReliabilityReply::Lease { expires_at: None }
            }
            Save {
                key,
                expected_version,
                title,
                reason,
                occurred_at,
                ..
            } => {
                let input = ContractSave::new(
                    random_id(&key)?,
                    expected_version,
                    &title,
                    &reason,
                    occurred_at.as_deref(),
                )?;
                save(&mut tx, &access, &lease, input).await?
            }
        };
        tx.commit().await?;
        Ok(result)
    }
}

struct ContractSave<'a> {
    key: Uuid,
    expected: EntityVersion,
    title: &'a str,
    reason: &'a str,
    occurred_at: Option<DateTime<Utc>>,
}

impl<'a> ContractSave<'a> {
    fn new(
        key: Uuid,
        expected: i64,
        title: &'a str,
        reason: &'a str,
        occurred_at: Option<&str>,
    ) -> Result<Self, AuthError> {
        fn valid_text(value: &str, limit: usize) -> bool {
            (2..=limit).contains(&value.chars().count()) && !value.chars().any(char::is_control)
        }
        let title = title.trim();
        let reason = reason.trim();
        if !valid_text(title, 160) {
            return Err(invalid(
                "Informe um título entre 2 e 160 caracteres, sem quebras de linha.",
            ));
        }
        if !valid_text(reason, 200) {
            return Err(invalid(
                "Informe o motivo da alteração em 2 a 200 caracteres, sem quebras de linha.",
            ));
        }
        let expected = EntityVersion::new(expected)
            .map_err(|_| invalid("Atualize a referência para obter a versão atual."))?;
        let occurred_at = occurred_at
            .map(|value| {
                value
                    .parse::<UtcInstant>()
                    .map(UtcInstant::as_datetime)
                    .map_err(|_| invalid("Informe a data e a hora do fato com um fuso válido."))
            })
            .transpose()?;
        Ok(Self {
            key,
            expected,
            title,
            reason,
            occurred_at,
        })
    }

    fn fingerprint(&self, contract: Uuid) -> Result<Vec<u8>, AuthError> {
        // Uma tupla de valores tipados define a ordem dos campos. Datas equivalentes
        // em fusos diferentes e os valores normalizados no banco têm o mesmo hash.
        let canonical = serde_json::to_string(&(
            contract,
            self.expected.get(),
            self.title,
            self.reason,
            self.occurred_at.map(|value| value.timestamp_micros()),
        ))
        .map_err(|_| AuthError::unavailable())?;
        Ok(crypto::hash(RENAME, &canonical))
    }
}

async fn save(
    tx: &mut AuthTransaction,
    access: &ContractAccess,
    lease: &EditLease,
    input: ContractSave<'_>,
) -> Result<ReliabilityReply, AuthError> {
    let fingerprint = input.fingerprint(access.contract)?;
    if let Some(stored) = tx
        .operation_receipt(access.tenant, RENAME, input.key)
        .await?
    {
        if stored.scope_id != access.contract || stored.fingerprint != fingerprint {
            return Err(idempotency_conflict());
        }
        return Ok(ReliabilityReply::Saved {
            receipt: access.saved(tx, &stored).await?,
        });
    }
    if access.summary.version != input.expected.get() {
        return Ok(ReliabilityReply::VersionConflict {
            current: access.summary.clone(),
        });
    }
    if input.occurred_at.is_some_and(|value| value > access.now) {
        return Err(invalid(
            "A data do fato não pode ser posterior ao momento da gravação.",
        ));
    }
    let next_version = input
        .expected
        .next()
        .map_err(|_| AuthError::unavailable())?
        .get();
    let event = tx
        .revise_contract(&ContractRevision {
            lease,
            expected_version: input.expected.get(),
            title: input.title,
            reason: input.reason,
            occurred_at: input.occurred_at,
        })
        .await?
        .ok_or_else(expired_lease)?;
    let stored = OperationReceipt {
        actor_id: access.actor.id,
        scope_id: access.contract,
        fingerprint,
        result_id: event,
        result_version: next_version,
    };
    if !tx
        .insert_operation_receipt(access.tenant, RENAME, input.key, &stored)
        .await?
    {
        return Err(idempotency_conflict());
    }
    tx.release_contract_lease(lease).await?;
    Ok(ReliabilityReply::Saved {
        receipt: access.saved(tx, &stored).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_input_uses_normalized_text_time_scope_and_version() {
        let key = Uuid::new_v4();
        let contract = Uuid::new_v4();
        let a = ContractSave::new(
            key,
            1,
            " Título ",
            " ajuste ",
            Some("2026-10-31T10:15:00.123456-03:00"),
        )
        .unwrap();
        let b = ContractSave::new(
            key,
            1,
            "Título",
            "ajuste",
            Some("2026-10-31T13:15:00.123456Z"),
        )
        .unwrap();
        let c = ContractSave::new(
            key,
            2,
            "Título",
            "ajuste",
            Some("2026-10-31T13:15:00.123456Z"),
        )
        .unwrap();
        assert!(
            ContractSave::new(
                key,
                1,
                "Título",
                "ajuste",
                Some("2026-10-31T13:15:00.123456999Z")
            )
            .is_err()
        );
        assert_eq!(
            a.fingerprint(contract).unwrap(),
            b.fingerprint(contract).unwrap()
        );
        assert_ne!(
            a.fingerprint(contract).unwrap(),
            c.fingerprint(contract).unwrap()
        );
        assert_ne!(
            a.fingerprint(contract).unwrap(),
            b.fingerprint(Uuid::new_v4()).unwrap()
        );
    }
}
