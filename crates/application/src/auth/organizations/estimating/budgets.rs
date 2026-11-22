use super::*;
use canteiro_domain::budget_csv::{ImportDifference, NumberLocale, compare_details};

use canteiro_persistence::budget_imports::NewBudgetImport;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn preview_budget_csv(csv: &str, locale: NumberLocale) -> Result<BudgetRevision, AuthError> {
    preview_import(csv, locale).map_err(|error| invalid(error.to_string()))
}

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
                    snapshot.validate_no_implicit_removals(&old).map_err(|_| {
                        invalid(
                            "Exclusões exigem comando específico e conferência das referências.",
                        )
                    })?;
                }
                snapshot.clone()
            }
            BudgetChange::RemoveLine { line_id } => {
                let previous = current.as_ref().ok_or_else(record_denied)?;
                if previous.state == "approved" {
                    return Err(invalid("Uma revisão aprovada não pode ser excluída."));
                }
                let lease = input
                    .lease_key
                    .as_deref()
                    .ok_or_else(|| invalid("Reserve o orçamento nesta sessão antes de excluir."))?;
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
                        "A reserva venceu ou outra sessão alterou o orçamento.",
                    ));
                }
                let mut next = budget_view(previous.clone())?.snapshot;
                next.remove_unlinked_line(line_id).map_err(|_| {
                    invalid("A exclusão está restrita a serviço manual sem fonte vinculada.")
                })?;
                next.revision = u32::try_from(next_version)
                    .map_err(|_| invalid("Limite de revisões alcançado."))?;
                next.status = BudgetState::Draft;
                next
            }
            BudgetChange::CopyLine { .. } | BudgetChange::MoveLine { .. } => {
                let previous = current.as_ref().ok_or_else(record_denied)?;
                if previous.state == "approved" {
                    return Err(invalid(
                        "Uma revisão aprovada não admite cópia ou movimentação.",
                    ));
                }
                let lease = input
                    .lease_key
                    .as_deref()
                    .ok_or_else(|| invalid("Reserve o orçamento antes de alterar um serviço."))?;
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
                        "A reserva venceu ou outra sessão alterou o orçamento.",
                    ));
                }
                let mut next = budget_view(previous.clone())?.snapshot;
                let changed = match &input.change {
                    BudgetChange::CopyLine {
                        source_id,
                        new_id,
                        target_variation_id,
                    } => next.copy_line(source_id, new_id, target_variation_id),
                    BudgetChange::MoveLine {
                        line_id,
                        target_variation_id,
                    } => next.move_line(line_id, target_variation_id),
                    _ => return Err(AuthError::unavailable()),
                };
                changed.map_err(|_| {
                    invalid("Confira o serviço, a identificação e a variação de destino.")
                })?;
                next.revision = u32::try_from(next_version)
                    .map_err(|_| invalid("Limite de revisões alcançado."))?;
                next.status = BudgetState::Draft;
                next
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
        // O recibo de importação só se confirma junto com a revisão efetiva.
        // Caso a transação falhe depois, ambos permanecem não aplicados.
        if matches!(&input.change, BudgetChange::Save { .. })
            && tx
                .budget_import_exists(a.tenant, scope, a.actor.id, a.actor.session, key)
                .await?
            && !tx
                .budget_import_mark_applied(a.tenant, scope, a.actor.id, a.actor.session, key, key)
                .await?
        {
            return Err(record_conflict());
        }
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
        let mut snapshot = preview_budget_csv(&input.csv, input.locale)?;
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
        let (difference, change_details) = if let Some(old) = current {
            let before = budget_view(old)?;
            snapshot
                .validate_catalog_lineage(&before.snapshot)
                .map_err(|_| invalid("A referência histórica do serviço não pode ser alterada."))?;
            snapshot.validate_no_implicit_removals(&before.snapshot).map_err(|_| {
                invalid(
                    "O CSV não pode excluir serviços silenciosamente. Use a exclusão individual validada.",
                )
            })?;
            (
                compare(&before.snapshot, &snapshot),
                compare_details(Some(&before.snapshot), &snapshot),
            )
        } else {
            (
                ImportDifference {
                    included: snapshot.lines.len(),
                    changed: 0,
                    removed: 0,
                },
                compare_details(None, &snapshot),
            )
        };
        // Tanto a primeira importação quanto uma reimportação possuem identidade
        // durável vinculada ao conteúdo integral, à sessão e à revisão-base.
        let sha256: [u8; 32] = Sha256::digest(input.csv.as_bytes()).into();
        let payload = serde_json::to_string(&snapshot).map_err(|_| AuthError::unavailable())?;
        let count = |n: usize| {
            i32::try_from(n).map_err(|_| invalid("A quantidade de serviços excede o limite."))
        };
        let batch = NewBudgetImport {
            tenant: a.tenant,
            id: Uuid::new_v4(),
            scope,
            actor: a.actor.id,
            session: a.actor.session,
            base_version,
            locale: match input.locale {
                NumberLocale::Canonical => "canonical",
                NumberLocale::PtBr => "pt-BR",
            },
            sha256: &sha256,
            preview: &payload,
            included: count(difference.included)?,
            changed: count(difference.changed)?,
            removed: count(difference.removed)?,
        };
        let batch_id = Some(
            tx.budget_import_stage(&batch)
                .await?
                .ok_or_else(record_conflict)?
                .to_string(),
        );
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(BudgetImportPreviewReply {
            snapshot,
            difference,
            change_details,
            total,
            base_version,
            batch_id,
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

#[cfg(test)]
mod budget_import_diagnostics_tests {
    use super::*;

    #[test]
    fn malformed_budget_csv_retains_physical_error_coordinates() {
        let input = "CANTEIRO-ORCAMENTO-CSV-1\ncoluna_invalida";
        let error = match preview_budget_csv(input, NumberLocale::PtBr) {
            Ok(_) => panic!("O CSV incompleto deve ser rejeitado."),
            Err(error) => error,
        };
        assert!(error.message().contains("Linha 2, coluna 1"));
    }
}

#[cfg(test)]
mod budget_transfer_contract_tests {
    use super::*;

    #[test]
    fn copy_move_requests_cannot_inject_prices_or_entire_snapshots() {
        let copy = serde_json::json!({
            "action": "copy_line",
            "source_id": "existing",
            "new_id": "duplicate",
            "target_variation_id": "v2"
        });
        assert!(matches!(
            serde_json::from_value::<BudgetChange>(copy.clone()).unwrap(),
            BudgetChange::CopyLine {
                source_id,
                new_id,
                target_variation_id,
            } if source_id == "existing" && new_id == "duplicate" && target_variation_id == "v2"
        ));
        let mut forged = copy;
        forged["price"] = serde_json::json!("0.01");
        assert!(serde_json::from_value::<BudgetChange>(forged).is_err());

        let moved = serde_json::json!({
            "action": "move_line",
            "line_id": "existing",
            "target_variation_id": "v2"
        });
        assert!(matches!(
            serde_json::from_value::<BudgetChange>(moved.clone()).unwrap(),
            BudgetChange::MoveLine {
                line_id,
                target_variation_id,
            } if line_id == "existing" && target_variation_id == "v2"
        ));
        let mut forged = moved;
        forged["snapshot"] = serde_json::json!({"lines": []});
        assert!(serde_json::from_value::<BudgetChange>(forged).is_err());

        assert!(
            serde_json::from_value::<BudgetChange>(serde_json::json!({
                "action": "copy_line",
                "source_id": "existing",
                "target_variation_id": "v2"
            }))
            .is_err()
        );
    }
}
