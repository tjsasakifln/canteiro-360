use super::*;
use canteiro_domain::budget_csv::{ImportDifference, NumberLocale};

use std::collections::BTreeMap;

/// Sempre confronta a referência informada com a edição publicada no mesmo escopo.
/// Impede substituir o preço ou o código por payload direto ou importação CSV.
async fn require_catalog_snapshot(
    tx: &mut AuthTransaction,
    tenant: Uuid,
    scope: Uuid,
    snapshot: &BudgetRevision,
) -> Result<(), AuthError> {
    let catalogs = snapshot
        .lines
        .iter()
        .filter_map(|item| {
            item.catalog_id
                .as_ref()
                .zip(item.catalog_edition)
                .map(|(catalog, edition)| (catalog.clone(), edition))
        })
        .collect::<BTreeSet<_>>();
    let mut official = BTreeMap::new();
    for (catalog, edition) in catalogs {
        if !tx
            .composition_catalog_published(tenant, scope, &catalog, edition)
            .await?
        {
            return Err(invalid(
                "A tabela histórica escolhida não está publicada neste orçamento.",
            ));
        }
        let rows = tx
            .catalog_lines(
                tenant,
                scope,
                &catalog,
                i32::try_from(edition).map_err(|_| invalid("Edição inválida."))?,
            )
            .await?;
        for row in rows {
            let unit: canteiro_domain::units::MeasureUnit =
                row.unit.parse().map_err(|_| AuthError::unavailable())?;
            let price: canteiro_domain::decimal::UnitPrice =
                row.price.parse().map_err(|_| AuthError::unavailable())?;
            official.insert((catalog.clone(), edition, row.code), (unit, price));
        }
    }
    snapshot
        .validate_catalog_references(&official)
        .map_err(|_| {
            invalid("O código, a unidade ou o preço não coincide com a edição histórica publicada.")
        })
}

impl AuthService {
    pub async fn estimating_budget_lease(
        &self,
        token: &str,
        input: BudgetLeaseCommand,
    ) -> Result<BudgetLeaseReply, AuthError> {
        self.limit("estimating-budget-lease", token, 30, 60).await?;
        let key = operation_id(&input.lease_key)?;
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(&input.scope_id)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        let current = tx.budget_current(a.tenant, scope).await?;
        if current.as_ref().is_some_and(|v| v.state == "approved") {
            return Err(invalid(
                "Este orçamento já foi aprovado. Abra uma alteração formal para uma nova revisão.",
            ));
        }
        let next = current.as_ref().map_or(1, |r| r.version.saturating_add(1));
        let revision = u32::try_from(next)
            .map_err(|_| invalid("O limite de revisões deste orçamento foi alcançado."))?;
        let expires=tx.budget_acquire_lease(
            a.tenant,scope,a.actor.id,a.actor.session,key,next
        ).await?.ok_or_else(||AuthError::new(
            AuthErrorKind::Conflict,"edit_busy",
            "Outra sessão está editando este orçamento. Confira a reserva antes de continuar.",
        ))?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(BudgetLeaseReply {
            scope_id: scope.to_string(),
            revision,
            lease_key: key.to_string(),
            valid_until: expires.to_rfc3339(),
        })
    }
    pub async fn estimating_budget_execute(
        &self,
        token: &str,
        input: BudgetCommand,
    ) -> Result<BudgetReply, AuthError> {
        self.limit("estimating-budget-write", token, 60, 60).await?;
        let key = operation_id(&input.operation_key)?;
        let reason = reason(&input.reason)?;
        let approving = matches!(&input.change, BudgetChange::Approve);
        let action = if approving {
            AccessAction::ApproveBudget
        } else {
            AccessAction::EditBudget
        };
        let fingerprint = crypto::hash(
            "budget-command",
            &serde_json::to_string(&input).map_err(|_| AuthError::unavailable())?,
        );
        let scope = id(&input.scope_id)?;
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        require_budget(&a, scope, action, tx.now().await?)?;
        if let Some(saved) = tx.budget_operation(a.tenant, key).await? {
            if saved.actor_id != a.actor.id
                || saved.scope_id != scope
                || saved.fingerprint != fingerprint
            {
                return Err(repeated_conflict());
            }
            self.record_revalidate(&mut tx, token, &a, scope, action)
                .await?;
            let budget = budget_view(saved)?;
            let total = budget
                .snapshot
                .summarize()
                .map_err(|_| AuthError::unavailable())?;
            tx.commit().await?;
            return Ok(BudgetReply {
                budget,
                total,
                repeated: true,
            });
        }
        let current = tx.budget_current(a.tenant, scope).await?;
        if current.as_ref().map(|r| r.version) != input.expected_version {
            return Err(record_conflict());
        }
        let next_version = input
            .expected_version
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(record_conflict)?;
        let snapshot = match &input.change {
            BudgetChange::Save { snapshot } => {
                if current.as_ref().is_some_and(|r| r.state == "approved")
                    || snapshot.status == BudgetState::Approved
                {
                    return Err(invalid(
                        "Não edite diretamente uma revisão definitiva ou aprovada.",
                    ));
                }
                let lease = input
                    .lease_key
                    .as_deref()
                    .ok_or_else(|| invalid("Reserve o orçamento nesta sessão antes de salvar."))?;
                let lease = operation_id(lease)?;
                if !tx
                    .budget_lease_usable(
                        a.tenant,
                        scope,
                        a.actor.id,
                        a.actor.session,
                        lease,
                        next_version,
                    )
                    .await?
                {
                    return Err(AuthError::new(
                        AuthErrorKind::Conflict,
                        "edit_expired",
                        "Sua reserva de edição expirou ou pertence a outra sessão. O rascunho deve ser preservado.",
                    ));
                }
                if let Some(previous) = &current {
                    let old = budget_view(previous.clone())?.snapshot;
                    snapshot.validate_catalog_lineage(&old).map_err(|_| {
                        invalid("A referência histórica do serviço não pode ser alterada.")
                    })?;
                }
                snapshot.clone()
            }
            BudgetChange::Approve => {
                let previous = current.as_ref().ok_or_else(record_denied)?;
                if previous.state != "review" || previous.actor_id == a.actor.id {
                    return Err(AuthError::new(
                        AuthErrorKind::Forbidden,
                        "self_approval",
                        "A aprovação exige conferência anterior e pessoa diferente de quem preparou a revisão.",
                    ));
                }
                let mut prior = budget_view(previous.clone())?.snapshot;
                prior.revision = u32::try_from(next_version)
                    .map_err(|_| invalid("Limite de revisões alcançado."))?;
                prior.status = BudgetState::Approved;
                prior
            }
        };
        if snapshot.budget_id != scope.to_string() || i64::from(snapshot.revision) != next_version {
            return Err(invalid(
                "O identificador e a revisão devem corresponder ao orçamento selecionado.",
            ));
        }
        snapshot
            .validate()
            .map_err(|_| invalid("Confira as variações, grupos, serviços e cronograma."))?;
        let summary = snapshot.summarize().map_err(|_| {
            invalid("Não foi possível reconciliar o orçamento com os valores informados.")
        })?;
        if !approving {
            for variation in &snapshot.variations {
                let work = variation.work_id.as_uuid();
                a.require(work, AccessAction::View, tx.now().await?)?;
                if !tx.project_work_exists(a.tenant, work).await? {
                    return Err(invalid(
                        "Uma variação está vinculada a obra inexistente nesta empresa.",
                    ));
                }
            }
            require_catalog_snapshot(&mut tx, a.tenant, scope, &snapshot).await?;
        }
        let state = match snapshot.status {
            BudgetState::Draft => "draft",
            BudgetState::Review => "review",
            BudgetState::Approved => "approved",
        };
        let payload = serde_json::to_string(&snapshot).map_err(|_| AuthError::unavailable())?;
        let saved = tx
            .budget_write(&BudgetWrite {
                tenant: a.tenant,
                scope,
                expected: input.expected_version,
                actor: a.actor.id,
                operation_key: key,
                fingerprint: &fingerprint,
                reason,
                payload: &payload,
                state,
            })
            .await?
            .ok_or_else(record_conflict)?;
        if !approving && let Some(lease) = input.lease_key.as_deref() {
            let lease = operation_id(lease)?;
            tx.budget_release_lease(a.tenant, scope, a.actor.id, a.actor.session, lease)
                .await?;
        }
        tx.org_audit(
            a.tenant,
            a.actor.id,
            if approving {
                "budget_approved"
            } else {
                "budget_revision"
            },
            Some(scope),
            &format!("version:{};state:{}", next_version, state),
        )
        .await?;
        self.record_revalidate(&mut tx, token, &a, scope, action)
            .await?;
        let budget = budget_view(saved)?;
        tx.commit().await?;
        Ok(BudgetReply {
            budget,
            total: summary,
            repeated: false,
        })
    }
    pub async fn estimating_budget_import_preview(
        &self,
        token: &str,
        input: BudgetImportPreviewCommand,
    ) -> Result<BudgetImportPreviewReply, AuthError> {
        self.limit("estimating-budget-import", token, 10, 60)
            .await?;
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(&input.scope_id)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        let current = tx.budget_current(a.tenant, scope).await?;
        if current.as_ref().is_some_and(|r| r.state == "approved") {
            return Err(invalid(
                "O orçamento definitivo exige uma alteração formal; não é editável por importação.",
            ));
        }
        let mut snapshot = preview_import(&input.csv, input.locale).map_err(|_| {
            invalid("Confira a linha e a coluna do arquivo CSV conforme o formato oficial.")
        })?;
        if snapshot.budget_id != scope.to_string() {
            return Err(invalid("O CSV não pertence ao orçamento selecionado."));
        }
        let base_version = current.as_ref().map(|r| r.version);
        snapshot.revision = u32::try_from(base_version.unwrap_or(0).saturating_add(1))
            .map_err(|_| invalid("Limite de revisões alcançado."))?;
        snapshot.status = BudgetState::Draft;
        let total = snapshot
            .summarize()
            .map_err(|_| invalid("Valores ou grupos inválidos no CSV."))?;
        require_catalog_snapshot(&mut tx, a.tenant, scope, &snapshot).await?;
        let difference = if let Some(old) = current {
            let before = budget_view(old)?;
            snapshot
                .validate_catalog_lineage(&before.snapshot)
                .map_err(|_| invalid("A referência histórica do serviço não pode ser alterada."))?;
            compare(&before.snapshot, &snapshot)
        } else {
            ImportDifference {
                included: snapshot.lines.len(),
                changed: 0,
                removed: 0,
            }
        };
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(BudgetImportPreviewReply {
            snapshot,
            difference,
            total,
            base_version,
            format: FORMAT_VERSION,
        })
    }
    pub async fn estimating_budget_export_csv(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        locale: NumberLocale,
    ) -> Result<String, AuthError> {
        self.limit("estimating-budget-export", token, 30, 60)
            .await?;
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        let row = tx
            .budget_current(a.tenant, scope)
            .await?
            .ok_or_else(record_denied)?;
        let snapshot = budget_view(row)?.snapshot;
        let csv = export_csv(&snapshot, locale)
            .map_err(|_| invalid("O tamanho deste orçamento excede o limite de exportação CSV."))?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        tx.commit().await?;
        Ok(csv)
    }
}
