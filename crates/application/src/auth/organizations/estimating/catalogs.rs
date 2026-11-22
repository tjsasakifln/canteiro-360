use super::*;
use canteiro_domain::units::MeasureUnit;
use chrono::NaiveDate;

fn cost_category(value: &str) -> Result<CostCategory, AuthError> {
    match value {
        "material" => Ok(CostCategory::Material),
        "labor" => Ok(CostCategory::Labor),
        "equipment" => Ok(CostCategory::Equipment),
        "service" => Ok(CostCategory::Service),
        "transport" => Ok(CostCategory::Transport),
        _ => Err(AuthError::unavailable()),
    }
}
fn category_name(value: CostCategory) -> &'static str {
    match value {
        CostCategory::Material => "material",
        CostCategory::Labor => "labor",
        CostCategory::Equipment => "equipment",
        CostCategory::Service => "service",
        CostCategory::Transport => "transport",
    }
}
pub(super) async fn load_catalog(
    tx: &mut AuthTransaction,
    tenant: Uuid,
    scope: Uuid,
    catalog: &str,
    edition: u32,
) -> Result<CatalogEdition, AuthError> {
    let header = tx
        .catalog_headers(tenant, &[scope])
        .await?
        .into_iter()
        .find(|h| h.catalog_id == catalog && h.edition as u32 == edition)
        .ok_or_else(record_denied)?;
    let rows = tx
        .catalog_lines(
            tenant,
            scope,
            catalog,
            i32::try_from(edition).map_err(|_| invalid("Edição inválida."))?,
        )
        .await?;
    let items = rows
        .into_iter()
        .map(|r| {
            Ok(CatalogItem {
                code: r.code,
                description: r.description,
                category: cost_category(&r.category)?,
                unit: r
                    .unit
                    .parse::<MeasureUnit>()
                    .map_err(|_| AuthError::unavailable())?,
                price: r.price.parse().map_err(|_| AuthError::unavailable())?,
            })
        })
        .collect::<Result<Vec<_>, AuthError>>()?;
    CatalogEdition::assemble(
        header.catalog_id,
        edition,
        header
            .published_on
            .to_string()
            .parse()
            .map_err(|_| AuthError::unavailable())?,
        CostSource {
            title: header.source_title,
            organization: header.source_organization,
            license: header.license,
            locality: header.locality,
            tax_regime: header.tax_regime,
            competence: header.competence.format("%Y-%m").to_string(),
        },
        items,
    )
    .map_err(|_| AuthError::unavailable())
}

fn edition_from_csv(input: &CatalogCsvImportCommand) -> Result<CatalogEdition, AuthError> {
    let published_on: canteiro_domain::CivilDate = input
        .published_on
        .parse()
        .map_err(|_| invalid("Informe a data de publicação no formato AAAA-MM-DD."))?;
    let items = canteiro_domain::catalog_csv::preview_import(&input.csv, input.locale)
        .map_err(|error| invalid(error.to_string()))?;
    CatalogEdition::assemble(
        input.catalog_id.clone(),
        input.edition,
        published_on,
        input.source.clone(),
        items,
    )
    .map_err(|_| invalid("Confira a identificação, fonte, licença, edição e linhas de preços."))
}

impl AuthService {
    pub async fn estimating_catalog(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        catalog: &str,
        edition: u32,
    ) -> Result<CatalogEdition, AuthError> {
        self.limit("estimating-catalog", token, 30, 60).await?;
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        let value = load_catalog(&mut tx, a.tenant, scope, catalog, edition).await?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        tx.commit().await?;
        Ok(value)
    }
    /// Consulta comparativa autenticada em uma transação. A fonte anterior
    /// permanece legível e nenhuma edição histórica é regravada.
    pub async fn estimating_catalog_compare(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        catalog: &str,
        before: u32,
        after: u32,
    ) -> Result<CatalogComparison, AuthError> {
        self.limit("estimating-catalog-compare", token, 30, 60)
            .await?;
        if before == after {
            return Err(invalid(
                "Selecione duas edições diferentes da mesma tabela.",
            ));
        }
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        let old = load_catalog(&mut tx, a.tenant, scope, catalog, before).await?;
        let new = load_catalog(&mut tx, a.tenant, scope, catalog, after).await?;
        let result = old
            .compare_to(&new)
            .map_err(|_| invalid("Não foi possível comparar essas edições."))?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn estimating_catalog_publish(
        &self,
        token: &str,
        input: CatalogPublishCommand,
    ) -> Result<CatalogHeader, AuthError> {
        self.limit("estimating-publish-catalog", token, 20, 60)
            .await?;
        reason(&input.reason)?;
        let scope = id(&input.scope_id)?;
        let edition = &input.edition;
        let version =
            i32::try_from(edition.edition).map_err(|_| invalid("Edição de preços inválida."))?;
        let validated = CatalogEdition::assemble(
            edition.catalog_id.clone(),
            edition.edition,
            edition.published_on,
            edition.source.clone(),
            edition.items.values().cloned().collect(),
        )
        .map_err(|_| invalid("Confira a fonte, edição, unidades, preços e códigos da tabela."))?;
        if validated.items != edition.items {
            return Err(invalid(
                "Os códigos das linhas não correspondem ao catálogo enviado.",
            ));
        }
        let competence =
            NaiveDate::parse_from_str(&format!("{}-01", edition.source.competence), "%Y-%m-%d")
                .map_err(|_| invalid("Informe a competência da fonte no formato AAAA-MM."))?;
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        if tx
            .catalog_edition_exists(a.tenant, &edition.catalog_id, version)
            .await?
        {
            return Err(record_conflict());
        }
        let rows = edition
            .items
            .values()
            .map(|item| {
                (
                    item.code.clone(),
                    item.description.clone(),
                    category_name(item.category).to_owned(),
                    item.unit.to_string(),
                    item.price.to_string(),
                )
            })
            .collect::<Vec<_>>();
        let borrowed = rows
            .iter()
            .map(
                |(code, description, category, unit, price)| NewCatalogLine {
                    code,
                    description,
                    category,
                    unit,
                    price,
                },
            )
            .collect::<Vec<_>>();
        let header = NewCatalogEdition {
            tenant: a.tenant,
            scope,
            catalog_id: &edition.catalog_id,
            edition: version,
            source_title: &edition.source.title,
            source_organization: &edition.source.organization,
            license: &edition.source.license,
            locality: &edition.source.locality,
            tax_regime: &edition.source.tax_regime,
            competence,
            published_on: edition.published_on.as_naive_date(),
            actor: a.actor.id,
        };
        let created = tx.catalog_publish(&header, &borrowed).await?;
        if !created {
            return Err(record_conflict());
        }
        tx.org_audit(
            a.tenant,
            a.actor.id,
            "catalog_edition_published",
            Some(scope),
            &format!("catalog:{};edition:{}", edition.catalog_id, edition.edition),
        )
        .await?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(CatalogHeader {
            scope_id: scope.to_string(),
            catalog_id: edition.catalog_id.clone(),
            edition: edition.edition,
            source: edition.source.clone(),
            published_on: edition.published_on.to_string(),
        })
    }
    /// Uma consulta autenticada, sem gravação, de todos os preços e da variação histórica.
    pub async fn estimating_catalog_csv_preview(
        &self,
        token: &str,
        input: CatalogCsvImportCommand,
    ) -> Result<CatalogCsvPreviewReply, AuthError> {
        self.limit("estimating-catalog-import-preview", token, 10, 60)
            .await?;
        reason(&input.reason)?;
        let edition = edition_from_csv(&input)?;
        let scope = id(&input.scope_id)?;
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        let edition_number =
            i32::try_from(edition.edition).map_err(|_| invalid("Edição de preços inválida."))?;
        if tx
            .catalog_edition_exists(a.tenant, &edition.catalog_id, edition_number)
            .await?
        {
            return Err(record_conflict());
        }
        let previous = tx
            .catalog_headers(a.tenant, &[scope])
            .await?
            .into_iter()
            .filter(|header| {
                header.catalog_id == edition.catalog_id && header.edition < edition_number
            })
            .max_by_key(|header| header.edition);
        let (previous_edition, included, changed, removed) = if let Some(previous) = previous {
            let old = load_catalog(
                &mut tx,
                a.tenant,
                scope,
                &edition.catalog_id,
                previous.edition as u32,
            )
            .await?;
            let differences = old
                .compare_to(&edition)
                .map_err(|_| AuthError::unavailable())?;
            (
                Some(previous.edition as u32),
                differences.added.len(),
                differences.changed.len(),
                differences.removed.len(),
            )
        } else {
            (None, edition.items.len(), 0, 0)
        };
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(CatalogCsvPreviewReply {
            header: CatalogHeader {
                scope_id: scope.to_string(),
                catalog_id: edition.catalog_id,
                edition: edition.edition,
                source: edition.source,
                published_on: edition.published_on.to_string(),
            },
            items_total: edition.items.len(),
            sample: edition.items.values().take(20).cloned().collect(),
            previous_edition,
            included,
            changed,
            removed,
        })
    }

    /// Publica numa transação existente: nenhuma linha de uma edição rejeitada persiste.
    pub async fn estimating_catalog_csv_publish(
        &self,
        token: &str,
        input: CatalogCsvImportCommand,
    ) -> Result<CatalogHeader, AuthError> {
        self.limit("estimating-catalog-import-publish", token, 10, 60)
            .await?;
        let edition = edition_from_csv(&input)?;
        self.estimating_catalog_publish(
            token,
            CatalogPublishCommand {
                context: input.context,
                scope_id: input.scope_id,
                edition,
                reason: input.reason,
            },
        )
        .await
    }
}
