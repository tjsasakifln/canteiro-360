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
}
