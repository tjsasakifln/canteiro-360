//! Preços, composições e orçamento pertencem a escopo orçamentário explícito.
use super::{
    projects::{
        RecordAccess, operation_id, reason, record_conflict, record_denied, repeated_conflict,
    },
    *,
};
use canteiro_contracts::{access::AccessAction, estimating::*, reliability::ReliabilityScope};
use canteiro_domain::{
    budget_csv::{FORMAT_VERSION, compare, export_csv, preview_import},
    budgets::{BudgetRevision, BudgetState},
    catalogs::{CatalogEdition, CatalogItem, CatalogLibrary, CostCategory, CostSource},
    compositions::{ComponentSource, Composition, CompositionBook},
};
use canteiro_persistence::{
    budgets::{BudgetWrite, StoredBudgetVersion},
    catalogs::{NewCatalogEdition, NewCatalogLine, StoredCatalogHeader},
    compositions::{CompositionWrite, StoredCompositionVersion},
};
use std::collections::BTreeSet;

mod budgets;
mod catalogs;
mod compositions;

fn require_budget(
    access: &RecordAccess,
    scope: Uuid,
    action: AccessAction,
    now: DateTime<Utc>,
) -> Result<(), AuthError> {
    if !access
        .snapshot
        .scopes
        .iter()
        .any(|s| s.id == scope && s.kind == "budget")
    {
        return Err(record_denied());
    }
    access.require(scope, action, now)
}
fn budget_view(row: StoredBudgetVersion) -> Result<BudgetRecord, AuthError> {
    let snapshot: BudgetRevision =
        serde_json::from_str(&row.payload).map_err(|_| AuthError::unavailable())?;
    if snapshot.budget_id != row.scope_id.to_string() || i64::from(snapshot.revision) != row.version
    {
        return Err(AuthError::unavailable());
    }
    Ok(BudgetRecord {
        scope_id: row.scope_id.to_string(),
        version: row.version,
        snapshot,
        operation_key: row.operation_key.to_string(),
        actor_id: row.actor_id.to_string(),
        reason: row.reason,
        recorded_at: row.created_at.to_rfc3339(),
    })
}
fn composition_view(row: StoredCompositionVersion) -> Result<CompositionRecord, AuthError> {
    Ok(CompositionRecord {
        scope_id: row.scope_id.to_string(),
        code: row.code,
        version: row.version,
        composition: serde_json::from_str(&row.payload).map_err(|_| AuthError::unavailable())?,
        actor_id: row.actor_id.to_string(),
        reason: row.reason,
        recorded_at: row.recorded_at.to_rfc3339(),
    })
}
fn catalog_header(row: StoredCatalogHeader) -> CatalogHeader {
    CatalogHeader {
        scope_id: row.scope_id.to_string(),
        catalog_id: row.catalog_id,
        edition: row.edition as u32,
        published_on: row.published_on.to_string(),
        source: CostSource {
            title: row.source_title,
            organization: row.source_organization,
            license: row.license,
            locality: row.locality,
            tax_regime: row.tax_regime,
            competence: row.competence.format("%Y-%m").to_string(),
        },
    }
}
impl AuthService {
    pub async fn estimating_context(&self, token: &str) -> Result<EstimatingContext, AuthError> {
        self.limit("estimating-context", token, 60, 60).await?;
        let (mut tx, access) = self.record_access(token, None).await?;
        let mut out = EstimatingContext {
            csrf: self.csrf(token),
            context: None,
            organization_name: String::new(),
            scopes: vec![],
            catalogs: vec![],
            budgets: vec![],
            compositions: vec![],
        };
        if let Some(a) = access {
            let now = tx.now().await?;
            out.scopes = a
                .snapshot
                .scopes
                .iter()
                .filter(|scope| {
                    scope.kind == "budget" && a.permits(scope.id, AccessAction::View, now)
                })
                .map(|scope| EstimatingScope {
                    id: scope.id.to_string(),
                    name: scope.name.clone(),
                    can_edit: a.permits(scope.id, AccessAction::EditBudget, now),
                    can_review: a.permits(scope.id, AccessAction::ReviewBudget, now),
                    can_approve: a.permits(scope.id, AccessAction::ApproveBudget, now),
                })
                .collect();
            let scopes = out
                .scopes
                .iter()
                .map(|scope| id(&scope.id))
                .collect::<Result<Vec<_>, _>>()?;
            out.catalogs = tx
                .catalog_headers(a.tenant, &scopes)
                .await?
                .into_iter()
                .map(catalog_header)
                .collect();
            out.budgets = tx
                .budget_list(a.tenant, &scopes)
                .await?
                .into_iter()
                .map(budget_view)
                .collect::<Result<_, _>>()?;
            out.compositions = tx
                .composition_list(a.tenant, &scopes)
                .await?
                .into_iter()
                .map(composition_view)
                .collect::<Result<_, _>>()?;
            self.locked(&mut tx, token).await?;
            let final_now = tx.now().await?;
            if a.member_until.is_some_and(|until| until <= final_now)
                || scopes
                    .iter()
                    .any(|scope| !a.permits(*scope, AccessAction::View, final_now))
            {
                return Err(record_denied());
            }
            out.context = Some(a.context);
            out.organization_name = a.organization_name;
        }
        tx.commit().await?;
        Ok(out)
    }
    pub async fn estimating_budget_history(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
    ) -> Result<Vec<BudgetRecord>, AuthError> {
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        let rows = tx.budget_history(a.tenant, scope).await?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        let result = rows
            .into_iter()
            .map(budget_view)
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(result)
    }
}
