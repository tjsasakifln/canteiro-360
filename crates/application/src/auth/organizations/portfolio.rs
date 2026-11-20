//! Busca, painel, mapa e consulta direta usam o mesmo conjunto de permissões.
use super::access::{Snapshot, zero};
use super::parties::{decode, encode, registry_conflict, registry_key, registry_reason};
use super::*;
use canteiro_contracts::{
    access::{AccessAction, ScopeKind},
    portfolio::*,
};
use canteiro_persistence::portfolio::StoredWork;

fn can(
    snapshot: &Snapshot,
    actor: Uuid,
    scope: Uuid,
    action: AccessAction,
    now: DateTime<Utc>,
) -> bool {
    snapshot.permits(actor, scope, action, zero(), now)
}
fn link_visible(snapshot: &Snapshot, actor: Uuid, value: &str, now: DateTime<Utc>) -> bool {
    id(value).is_ok_and(|scope| can(snapshot, actor, scope, AccessAction::View, now))
}
fn visible_input(
    mut input: WorkInput,
    snapshot: &Snapshot,
    actor: Uuid,
    now: DateTime<Utc>,
) -> WorkInput {
    input
        .contract_ids
        .retain(|s| link_visible(snapshot, actor, s, now));
    input
}
fn work_view(
    row: StoredWork,
    snapshot: &Snapshot,
    actor: Uuid,
    now: DateTime<Utc>,
) -> Result<WorkView, AuthError> {
    let input: WorkInput = decode(&row.data)?;
    let can_edit = can(snapshot, actor, row.id, AccessAction::ManagePortfolio, now)
        && input
            .contract_ids
            .iter()
            .all(|s| link_visible(snapshot, actor, s, now));
    Ok(WorkView {
        id: row.id.to_string(),
        version: row.version,
        input: visible_input(input, snapshot, actor, now),
        can_edit,
    })
}
fn work_scope(
    snapshot: &Snapshot,
    actor: Uuid,
    work: Uuid,
    edit: bool,
    now: DateTime<Utc>,
) -> Result<(), AuthError> {
    if !snapshot
        .scopes
        .iter()
        .any(|s| s.id == work && s.kind == ScopeKind::Work.as_str())
        || !can(snapshot, actor, work, AccessAction::View, now)
        || (edit && !can(snapshot, actor, work, AccessAction::ManagePortfolio, now))
    {
        return Err(denied());
    }
    Ok(())
}
fn valid_input(input: WorkInput) -> Result<WorkInput, AuthError> {
    input.validated().map_err(|e|match e{
    canteiro_domain::portfolio::PortfolioError::Invalid(message)=>invalid(message),
    canteiro_domain::portfolio::PortfolioError::Percentage(_)=>invalid("O rateio municipal não fecha 100%. Confira a diferença apresentada no formulário."),
    canteiro_domain::portfolio::PortfolioError::Dimension(_)=>invalid("As dimensões dos municípios não fecham a dimensão total da obra. Confira a diferença apresentada no formulário."),
})
}
impl AuthService {
    pub async fn portfolio_context(
        &self,
        token: &str,
        query: &str,
        status: Option<WorkStatus>,
    ) -> Result<PortfolioContext, AuthError> {
        self.limit("portfolio-context", token, 120, 60).await?;
        let query = query.trim();
        if query.chars().count() > 160 || query.chars().any(char::is_control) {
            return Err(invalid("Use até 160 caracteres na busca de obras."));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let selected = tx.org_selection(actor.session).await?;
        let mut response = PortfolioContext {
            csrf: self.csrf(token),
            scope: None,
            organization_name: String::new(),
            available_scopes: vec![],
            contracts: vec![],
            rows: vec![],
            total: 0,
            located: 0,
            query: query.to_owned(),
            status,
        };
        if let Some(tenant) = selected.tenant_id {
            let (org, member) = authorize(&mut tx, &actor, tenant).await?;
            let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
            self.locked(&mut tx, token).await?;
            let now = tx.now().await?;
            if member.valid_until.is_some_and(|end| end <= now) {
                return Err(denied());
            }
            response.scope = Some(OrganizationScope {
                organization_id: tenant.to_string(),
                context_version: selected.generation,
                organization_version: org.version,
            });
            response.organization_name = org.name;
            let rows = tx.portfolio_list(tenant).await?;
            let now = self
                .registry_revalidate(&mut tx, token, &actor, tenant, false)
                .await?;
            let existing: std::collections::BTreeSet<_> = rows.iter().map(|r| r.id).collect();
            response.available_scopes = snapshot
                .scopes
                .iter()
                .filter(|s| {
                    s.kind == "work"
                        && !existing.contains(&s.id)
                        && can(&snapshot, actor.id, s.id, AccessAction::View, now)
                        && can(
                            &snapshot,
                            actor.id,
                            s.id,
                            AccessAction::ManagePortfolio,
                            now,
                        )
                })
                .map(|s| WorkScope {
                    id: s.id.to_string(),
                    name: s.name.clone(),
                })
                .collect();
            response.contracts = snapshot
                .scopes
                .iter()
                .filter(|s| {
                    s.kind == "contract" && can(&snapshot, actor.id, s.id, AccessAction::View, now)
                })
                .map(|s| WorkScope {
                    id: s.id.to_string(),
                    name: s.name.clone(),
                })
                .collect();
            let query_lower = query.to_lowercase();
            for row in rows {
                if !can(&snapshot, actor.id, row.id, AccessAction::View, now) {
                    continue;
                }
                let view = work_view(row, &snapshot, actor.id, now)?;
                if status.is_some_and(|s| s != view.input.status) {
                    continue;
                }
                if !query_lower.is_empty()
                    && !format!(
                        "{} {} {} {} {}",
                        view.input.title,
                        view.input.development,
                        view.input.cost_center,
                        view.input.external_reference,
                        view.input
                            .localities
                            .iter()
                            .map(|l| l.municipality.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                    .to_lowercase()
                    .contains(&query_lower)
                {
                    continue;
                }
                response.rows.push(view);
            }
            response.total = response.rows.len();
            response.located = response
                .rows
                .iter()
                .filter(|w| w.input.latitude.is_some())
                .count();
        }
        tx.commit().await?;
        Ok(response)
    }
    pub async fn portfolio_detail(
        &self,
        token: &str,
        context: &OrganizationScope,
        work: &str,
    ) -> Result<WorkView, AuthError> {
        let (mut tx, actor) = self.registry_access(token, context, false).await?;
        let tenant = id(&context.organization_id)?;
        let work = id(work)?;
        let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
        let now = tx.now().await?;
        work_scope(&snapshot, actor.id, work, false, now)?;
        let row = tx.portfolio_get(tenant, work).await?.ok_or_else(denied)?;
        let now = self
            .registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        work_scope(&snapshot, actor.id, work, false, now)?;
        let result = work_view(row, &snapshot, actor.id, now)?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn portfolio_revisions(
        &self,
        token: &str,
        context: &OrganizationScope,
        work: &str,
    ) -> Result<Vec<WorkRevision>, AuthError> {
        let (mut tx, actor) = self.registry_access(token, context, false).await?;
        let tenant = id(&context.organization_id)?;
        let work = id(work)?;
        let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
        let now = tx.now().await?;
        work_scope(&snapshot, actor.id, work, false, now)?;
        tx.portfolio_get(tenant, work).await?.ok_or_else(denied)?;
        let rows = tx.portfolio_history(tenant, work).await?;
        let now = self
            .registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        work_scope(&snapshot, actor.id, work, false, now)?;
        let mut result = vec![];
        for row in rows {
            result.push(WorkRevision {
                version: row.version,
                before: row
                    .before_data
                    .as_deref()
                    .map(|s| decode(s).map(|v| visible_input(v, &snapshot, actor.id, now)))
                    .transpose()?,
                after: visible_input(decode(&row.after_data)?, &snapshot, actor.id, now),
                actor: row.actor,
                recorded_at: row.recorded_at.to_rfc3339(),
                reason: row.reason,
            });
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn portfolio_execute(
        &self,
        token: &str,
        command: PortfolioCommand,
    ) -> Result<PortfolioReceipt, AuthError> {
        let (mut tx, actor) = self.registry_access(token, &command.context, false).await?;
        let tenant = id(&command.context.organization_id)?;
        let work = id(&command.work_id)?;
        let key = registry_key(&command.key)?;
        let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
        let now = tx.now().await?;
        work_scope(&snapshot, actor.id, work, true, now)?;
        let fingerprint = crypto::hash("portfolio_save", &encode(&command.idempotency_payload())?);
        if let Some(saved) = tx.registry_receipt(tenant, "portfolio_save", key).await? {
            if saved.actor_id != actor.id || saved.fingerprint != fingerprint {
                return Err(registry_conflict());
            }
            let response = decode(&saved.result)?;
            let final_now = self
                .registry_revalidate(&mut tx, token, &actor, tenant, false)
                .await?;
            work_scope(&snapshot, actor.id, work, true, final_now)?;
            tx.commit().await?;
            return Ok(response);
        }
        let input = valid_input(command.input)?;
        let reason = registry_reason(&command.reason)?;
        let current = tx.portfolio_get(tenant, work).await?;
        if current.as_ref().map(|r| r.version) != command.expected_version
            || command.expected_version.is_some_and(|v| v < 1)
        {
            return Err(stale());
        }
        if let Some(current) = &current {
            let old: WorkInput = decode(&current.data)?;
            if old
                .contract_ids
                .iter()
                .any(|s| !link_visible(&snapshot, actor.id, s, now))
            {
                return Err(denied());
            }
        }
        let contracts = input
            .contract_ids
            .iter()
            .map(|s| id(s))
            .collect::<Result<Vec<_>, _>>()?;
        for contract in &contracts {
            if !snapshot
                .scopes
                .iter()
                .any(|s| s.id == *contract && s.kind == "contract")
                || !can(&snapshot, actor.id, *contract, AccessAction::View, now)
            {
                return Err(denied());
            }
        }
        let version = command
            .expected_version
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(stale)?;
        let data = encode(&input)?;
        let status = match input.status {
            WorkStatus::Planned => "planned",
            WorkStatus::Active => "active",
            WorkStatus::Paused => "paused",
            WorkStatus::Completed => "completed",
        };
        if !tx
            .portfolio_write(
                tenant,
                work,
                actor.id,
                command.expected_version,
                &input.title,
                status,
                &data,
            )
            .await?
        {
            return Err(stale());
        }
        tx.portfolio_links(tenant, work, &contracts).await?;
        tx.portfolio_record_revision(
            tenant,
            work,
            version,
            current.as_ref().map(|r| r.data.as_str()),
            &data,
            actor.id,
            reason,
        )
        .await?;
        tx.org_audit(
            tenant,
            actor.id,
            "portfolio_saved",
            Some(work),
            &format!("version:{version};links:{}", contracts.len()),
        )
        .await?;
        let final_now = self
            .registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        work_scope(&snapshot, actor.id, work, true, final_now)?;
        for contract in &contracts {
            if !can(
                &snapshot,
                actor.id,
                *contract,
                AccessAction::View,
                final_now,
            ) {
                return Err(denied());
            }
        }
        if let Some(previous) = &current {
            let old: WorkInput = decode(&previous.data)?;
            if old
                .contract_ids
                .iter()
                .any(|s| !link_visible(&snapshot, actor.id, s, final_now))
            {
                return Err(denied());
            }
        }
        let response = PortfolioReceipt {
            id: work.to_string(),
            version,
            message: "Obra salva na carteira. Os vínculos mantêm uma única identificação da obra."
                .into(),
        };
        tx.registry_record(
            tenant,
            "portfolio_save",
            key,
            actor.id,
            &fingerprint,
            &encode(&response)?,
        )
        .await?;
        tx.commit().await?;
        Ok(response)
    }
}
