use crate::{
    auth::{AuthHttp, application_failure, private_headers},
    organizations::private_json,
    projects::{authenticated, bad_input, field, form_context, form_headers, html_page},
};
use axum::{
    Extension, Form, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::{estimating::*, reliability::ReliabilityScope};
use serde::Deserialize;
use std::collections::BTreeMap;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/estimating/context", get(context))
        .route("/api/estimating/catalog", get(catalog))
        .route("/api/estimating/catalog/compare", get(catalog_compare))
        .route("/orcamentos/catalogos/comparar", get(catalog_compare_page))
        .route(
            "/orcamentos/catalogos/conferir",
            post(catalog_csv_preview_form),
        )
        .route(
            "/orcamentos/catalogos/publicar-csv",
            post(catalog_csv_publish_form),
        )
        .route("/api/estimating/catalog/publish", post(catalog_publish))
        .route(
            "/api/estimating/catalog/import/preview",
            post(catalog_csv_preview),
        )
        .route(
            "/api/estimating/catalog/import/publish",
            post(catalog_csv_publish),
        )
        .route(
            "/api/estimating/composition/calculate",
            post(composition_calculate),
        )
        .route(
            "/orcamentos/composicoes/memoria",
            get(composition_memory_page),
        )
        .route(
            "/api/estimating/composition/command",
            post(composition_write),
        )
        .route(
            "/orcamentos/composicoes/editar-descricao",
            post(composition_edit_description_form),
        )
        .route(
            "/orcamentos/cronograma/salvar",
            post(budget_save_schedule_form),
        )
        .route("/orcamentos/servicos/editar", get(budget_edit_line_page))
        .route("/orcamentos/servicos/salvar", post(budget_save_line_form))
        .route(
            "/orcamentos/servicos/copiar",
            post(budget_transfer_line_form),
        )
        .route(
            "/orcamentos/servicos/mover",
            post(budget_transfer_line_form),
        )
        .route(
            "/orcamentos/servicos/excluir",
            post(budget_transfer_line_form),
        )
        .route("/api/estimating/budget/lease", post(budget_lease))
        .route("/api/estimating/budget/command", post(budget_write))
        .route(
            "/api/estimating/budget/import/preview",
            post(import_preview),
        )
        .route(
            "/api/estimating/budget/{scope}/history",
            get(budget_history),
        )
        .route(
            "/api/estimating/budget/{scope}/export.csv",
            get(budget_export),
        )
        .route("/orcamentos", get(page))
        .route("/orcamentos/conferir-csv", post(preview_form))
        .route("/orcamentos/publicar-csv", post(publish_budget_csv_form))
        .layer(DefaultBodyLimit::max(9_000_000))
}
async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    match auth.service().estimating_context(&token).await {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogQuery {
    organization_id: String,
    context_version: i64,
    scope_id: String,
    catalog_id: String,
    edition: u32,
}
async fn catalog(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<CatalogQuery>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(query)) = query else {
        return bad_input();
    };
    let context = ReliabilityScope {
        organization_id: query.organization_id,
        context_version: query.context_version,
    };
    match auth
        .service()
        .estimating_catalog(
            &token,
            &context,
            &query.scope_id,
            &query.catalog_id,
            query.edition,
        )
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogCompareQuery {
    organization_id: String,
    context_version: i64,
    scope_id: String,
    catalog_id: String,
    before: u32,
    after: u32,
    page: Option<usize>,
}
async fn compare_catalog(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<CatalogCompareQuery>, QueryRejection>,
    as_page: bool,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(query)) = query else {
        return bad_input();
    };
    let context = ReliabilityScope {
        organization_id: query.organization_id,
        context_version: query.context_version,
    };
    match auth
        .service()
        .estimating_catalog_compare(
            &token,
            &context,
            &query.scope_id,
            &query.catalog_id,
            query.before,
            query.after,
        )
        .await
    {
        Ok(result) if as_page => match auth.service().estimating_context(&token).await {
            Ok(ctx) if ctx.context.as_ref() == Some(&context) => html_page(
                canteiro_web::estimating::catalog_comparison(
                    ctx,
                    result,
                    query.scope_id,
                    query.page.unwrap_or(1),
                ),
                StatusCode::OK,
            ),
            Ok(_) => bad_input(),
            Err(error) => application_failure(error),
        },
        Ok(result) => private_json(result),
        Err(error) => application_failure(error),
    }
}
async fn catalog_compare(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<CatalogCompareQuery>, QueryRejection>,
) -> Response {
    compare_catalog(auth, headers, query, false).await
}
async fn catalog_compare_page(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<CatalogCompareQuery>, QueryRejection>,
) -> Response {
    compare_catalog(auth, headers, query, true).await
}

async fn catalog_publish(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<CatalogPublishCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_catalog_publish(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionCalculateCommand {
    context: ReliabilityScope,
    scope_id: String,
    composition: Composition,
}

async fn catalog_csv_preview(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<CatalogCsvImportCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_catalog_csv_preview(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(error) => application_failure(error),
    }
}

async fn catalog_csv_publish(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<CatalogCsvImportCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_catalog_csv_publish(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(error) => application_failure(error),
    }
}

async fn composition_calculate(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<CompositionCalculateCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_composition_calculate(
            &token,
            &input.context,
            &input.scope_id,
            input.composition,
        )
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompositionMemoryQuery {
    organization_id: String,
    context_version: i64,
    scope_id: String,
    code: String,
}

async fn composition_memory_page(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<CompositionMemoryQuery>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(query)) = query else {
        return bad_input();
    };
    let context = ReliabilityScope {
        organization_id: query.organization_id,
        context_version: query.context_version,
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(page) => page,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context) {
        return bad_input();
    }
    let Some(record) = page
        .compositions
        .iter()
        .find(|entry| entry.scope_id == query.scope_id && entry.code == query.code)
        .cloned()
    else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_composition_calculate(
            &token,
            &context,
            &query.scope_id,
            record.composition.clone(),
        )
        .await
    {
        Ok(memory) => html_page(
            canteiro_web::estimating::composition_memory(page, record, memory),
            StatusCode::OK,
        ),
        Err(error) => application_failure(error),
    }
}

async fn composition_write(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<CompositionWriteCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_composition_write(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
fn changed_composition_from_form(
    mut composition: Composition,
    data: &BTreeMap<String, String>,
) -> Option<Composition> {
    match field(data, "change_kind") {
        "" => composition.description = field(data, "description").to_owned(),
        "costs" => {
            composition.bdi = field(data, "bdi").parse().ok()?;
            composition.discount = field(data, "discount").parse().ok()?;
        }
        "coefficient" => {
            let line_id = field(data, "line_id");
            let line = composition
                .lines
                .iter_mut()
                .find(|line| line.id == line_id)?;
            line.coefficient = field(data, "coefficient").parse().ok()?;
        }
        "duplicate_line" => {
            let original = composition
                .lines
                .iter()
                .find(|line| line.id == field(data, "line_id"))?
                .clone();
            let mut copied = original;
            copied.id = field(data, "new_line_id").to_owned();
            // Evita identificadores interpretáveis como fórmulas ao exportar uma planilha.
            if !canteiro_domain::budget_csv::safe(&copied.id) {
                return None;
            }
            copied.coefficient = field(data, "coefficient").parse().ok()?;
            composition.lines.push(copied);
        }
        "add_catalog_line" => {
            let id = field(data, "new_line_id");
            if !canteiro_domain::budget_csv::safe(id) {
                return None;
            }
            let edition = field(data, "catalog_edition").parse::<u32>().ok()?;
            if edition == 0 {
                return None;
            }
            composition
                .lines
                .push(canteiro_domain::compositions::ComponentLine {
                    id: id.to_owned(),
                    source: canteiro_domain::compositions::ComponentSource::Catalog {
                        catalog_id: field(data, "catalog_id").to_owned(),
                        edition,
                        code: field(data, "catalog_code").to_owned(),
                    },
                    coefficient: field(data, "coefficient").parse().ok()?,
                    unit: field(data, "unit").parse().ok()?,
                    conversion: None,
                });
        }
        "add_composition_line" => {
            let id = field(data, "new_line_id");
            if !canteiro_domain::budget_csv::safe(id) {
                return None;
            }
            let source_code = field(data, "source_code");
            if source_code == composition.code {
                return None;
            }
            composition
                .lines
                .push(canteiro_domain::compositions::ComponentLine {
                    id: id.to_owned(),
                    source: canteiro_domain::compositions::ComponentSource::Composition {
                        code: source_code.to_owned(),
                    },
                    coefficient: field(data, "coefficient").parse().ok()?,
                    unit: field(data, "unit").parse().ok()?,
                    conversion: None,
                });
        }
        "remove_line" => {
            let line_id = field(data, "line_id");
            if composition
                .incidences
                .iter()
                .any(|incidence| incidence.base_lines.iter().any(|base| base == line_id))
            {
                return None;
            }
            let index = composition
                .lines
                .iter()
                .position(|line| line.id == line_id)?;
            composition.lines.remove(index);
        }
        "incidence" => {
            let incidence_id = field(data, "incidence_id");
            let incidence = composition
                .incidences
                .iter_mut()
                .find(|incidence| incidence.id == incidence_id)?;
            incidence.percent = field(data, "incidence_percent").parse().ok()?;
        }
        "add_incidence" => {
            let id = field(data, "new_incidence_id");
            if !canteiro_domain::budget_csv::safe(id) {
                return None;
            }
            let base_lines = field(data, "base_lines")
                .split(',')
                .map(str::trim)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if base_lines.iter().any(String::is_empty) {
                return None;
            }
            composition
                .incidences
                .push(canteiro_domain::compositions::CostIncidence {
                    id: id.to_owned(),
                    percent: field(data, "incidence_percent").parse().ok()?,
                    base_lines,
                });
        }
        "remove_incidence" => {
            let index = composition
                .incidences
                .iter()
                .position(|incidence| incidence.id == field(data, "incidence_id"))?;
            composition.incidences.remove(index);
        }
        "incidence_base" => {
            let incidence_id = field(data, "incidence_id");
            let bases = field(data, "base_lines")
                .split(',')
                .map(str::trim)
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if bases.iter().any(String::is_empty) {
                return None;
            }
            let incidence = composition
                .incidences
                .iter_mut()
                .find(|incidence| incidence.id == incidence_id)?;
            incidence.base_lines = bases;
        }
        "cost_mode" => {
            use canteiro_domain::compositions::CostMode;
            composition.cost_mode = match (field(data, "target_mode"), &composition.cost_mode) {
                ("formula", CostMode::Informed { .. }) => CostMode::Formula,
                ("informed", CostMode::Formula) => {
                    let reason = field(data, "cost_reason");
                    if !canteiro_domain::budget_csv::safe(reason) {
                        return None;
                    }
                    CostMode::Informed {
                        cost: field(data, "informed_cost").parse().ok()?,
                        reason: reason.to_owned(),
                    }
                }
                _ => return None,
            };
        }
        "informed_cost" => match &mut composition.cost_mode {
            canteiro_domain::compositions::CostMode::Informed { cost, reason } => {
                let technical_basis = field(data, "cost_reason");
                if !canteiro_domain::budget_csv::safe(technical_basis) {
                    return None;
                }
                *cost = field(data, "informed_cost").parse().ok()?;
                *reason = technical_basis.to_owned();
            }
            canteiro_domain::compositions::CostMode::Formula => return None,
        },
        _ => return None,
    }
    composition.validate().ok()?;
    Some(composition)
}

/// Alteração pontual preserva a estrutura, as referências e o cálculo da
/// composição salva. A versão e a autorização são revalidadas no comando.
async fn composition_edit_description_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let scope_id = field(&data, "scope_id");
    let code = field(&data, "code");
    let version = match field(&data, "expected_version").parse::<i64>() {
        Ok(value) if value > 0 => value,
        _ => return bad_input(),
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context) {
        return bad_input();
    }
    let Some(record) = page
        .compositions
        .iter()
        .find(|entry| entry.scope_id == scope_id && entry.code == code)
    else {
        return bad_input();
    };
    if record.version != version {
        let mut response = (
            StatusCode::CONFLICT,
            "A composição mudou desde a abertura do formulário. Recarregue a página e confira a revisão.",
        )
            .into_response();
        private_headers(&mut response);
        return response;
    }
    let Some(composition) = changed_composition_from_form(record.composition.clone(), &data) else {
        return bad_input();
    };
    let command = CompositionWriteCommand {
        context,
        scope_id: scope_id.to_owned(),
        operation_key: uuid::Uuid::new_v4().to_string(),
        expected_version: Some(version),
        reason: field(&data, "reason").to_owned(),
        composition,
    };
    match auth
        .service()
        .estimating_composition_write(&token, command)
        .await
    {
        Ok(_) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(canteiro_web::estimating::page(ctx), StatusCode::OK),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}
#[cfg(test)]
mod composition_form_tests {
    use super::*;
    use canteiro_domain::{
        compositions::{ComponentLine, ComponentSource, CostMode},
        units::MeasureUnit,
    };

    fn saved() -> Composition {
        Composition {
            code: "COMP-01".into(),
            description: "Composição de ensaio".into(),
            unit: MeasureUnit::Unit,
            cost_mode: CostMode::Informed {
                cost: "100".parse().unwrap(),
                reason: "Custo informado e documentado".into(),
            },
            lines: vec![ComponentLine {
                id: "insumo-01".into(),
                source: ComponentSource::Composition {
                    code: "BASE-01".into(),
                },
                coefficient: "1".parse().unwrap(),
                unit: MeasureUnit::Unit,
                conversion: None,
            }],
            incidences: vec![],
            bdi: "20".parse().unwrap(),
            discount: "10".parse().unwrap(),
        }
    }

    #[test]
    fn changing_cost_mode_requires_explicit_direction_and_informed_justification() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "cost_mode".into());
        fields.insert("target_mode".into(), "formula".into());
        let formula = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert!(matches!(formula.cost_mode, CostMode::Formula));
        assert_eq!(formula.lines, original.lines);
        assert_eq!(formula.incidences, original.incidences);
        assert!(matches!(original.cost_mode, CostMode::Informed { .. }));
        assert!(changed_composition_from_form(formula.clone(), &fields).is_none());

        fields.insert("target_mode".into(), "informed".into());
        fields.insert("informed_cost".into(), "145.50".into());
        fields.insert("cost_reason".into(), "Preço aferido em vistoria".into());
        let restored = changed_composition_from_form(formula.clone(), &fields).unwrap();
        assert_eq!(restored.lines, formula.lines);
        assert_eq!(restored.incidences, formula.incidences);
        match restored.cost_mode {
            CostMode::Informed { cost, reason } => {
                assert_eq!(cost.to_string(), "145.500000");
                assert_eq!(reason, "Preço aferido em vistoria");
            }
            CostMode::Formula => panic!("A revisão deveria ter custo informado."),
        }
        assert!(matches!(formula.cost_mode, CostMode::Formula));
        for (key, invalid) in [
            ("target_mode", "desconhecido"),
            ("target_mode", "formula"),
            ("informed_cost", "-1"),
            ("informed_cost", "=20"),
            ("cost_reason", ""),
            ("cost_reason", "=soma(1;2)"),
        ] {
            let previous = fields.get(key).cloned();
            fields.insert(key.into(), invalid.into());
            assert!(changed_composition_from_form(formula.clone(), &fields).is_none());
            if let Some(value) = previous {
                fields.insert(key.into(), value);
            }
        }
    }

    #[test]
    fn coefficient_change_preserves_provenance_and_rejects_unknown_or_invalid_lines() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "coefficient".into());
        fields.insert("line_id".into(), "insumo-01".into());
        fields.insert("coefficient".into(), "2.5".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.lines[0].coefficient.to_string(), "2.5");
        assert_eq!(original.lines[0].coefficient.to_string(), "1");
        assert_eq!(changed.lines[0].source, original.lines[0].source);

        fields.insert("line_id".into(), "absent".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("line_id".into(), "insumo-01".into());
        fields.insert("coefficient".into(), "-1".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn duplicate_line_keeps_historical_source_and_rejects_invalid_copies() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "duplicate_line".into());
        fields.insert("line_id".into(), "insumo-01".into());
        fields.insert("new_line_id".into(), "insumo-02".into());
        fields.insert("coefficient".into(), "2.5".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.lines.len(), 2);
        assert_eq!(changed.lines[1].id, "insumo-02");
        assert_eq!(changed.lines[1].source, original.lines[0].source);
        assert_eq!(changed.lines[1].unit, original.lines[0].unit);
        assert_eq!(changed.lines[1].coefficient.to_string(), "2.5");
        assert_eq!(original.lines.len(), 1);

        fields.insert("new_line_id".into(), "insumo-01".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("new_line_id".into(), "=1+1".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("new_line_id".into(), "insumo\u{202e}ferido".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("new_line_id".into(), "insumo-02".into());
        fields.insert("coefficient".into(), "-1".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("coefficient".into(), "2".into());
        fields.insert("line_id".into(), "absent".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn catalog_input_is_added_without_editing_old_revision_or_price_source() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "add_catalog_line".into());
        fields.insert("new_line_id".into(), "novo-equipamento".into());
        fields.insert("catalog_id".into(), "SINAPI".into());
        fields.insert("catalog_edition".into(), "3".into());
        fields.insert("catalog_code".into(), "EQ-401".into());
        fields.insert("unit".into(), "h".into());
        fields.insert("coefficient".into(), "1.25".into());
        let added = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(original.lines.len(), 1);
        assert_eq!(added.lines.len(), 2);
        assert_eq!(added.lines[0], original.lines[0]);
        assert_eq!(added.lines[1].id, "novo-equipamento");
        assert_eq!(added.lines[1].coefficient.to_string(), "1.25");
        assert_eq!(added.lines[1].unit, MeasureUnit::Hour);
        assert_eq!(added.lines[1].conversion, None);
        assert_eq!(
            added.lines[1].source,
            ComponentSource::Catalog {
                catalog_id: "SINAPI".into(),
                edition: 3,
                code: "EQ-401".into(),
            }
        );
        assert_eq!(added.incidences, original.incidences);
        assert_eq!(added.cost_mode, original.cost_mode);

        for invalid_id in ["novo-equipamento", "insumo-01", "=SOMA(1)"] {
            if invalid_id == "novo-equipamento" {
                continue;
            }
            fields.insert("new_line_id".into(), invalid_id.into());
            assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        }
        fields.insert("new_line_id".into(), "novo-equipamento".into());
        for (key, value) in [
            ("catalog_edition", "0"),
            ("catalog_edition", "invalid"),
            ("unit", "unidade-desconhecida"),
            ("coefficient", "-10"),
            ("coefficient", "=1+1"),
        ] {
            let original_value = fields.get(key).unwrap().clone();
            fields.insert(key.into(), value.into());
            assert!(changed_composition_from_form(original.clone(), &fields).is_none());
            fields.insert(key.into(), original_value);
        }
    }

    #[test]
    fn nested_composition_input_preserves_history_and_rejects_self_reference() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "add_composition_line".into());
        fields.insert("new_line_id".into(), "subcomposicao-02".into());
        fields.insert("source_code".into(), "BASE-02".into());
        fields.insert("coefficient".into(), "0.75".into());
        fields.insert("unit".into(), "un".into());
        let result = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(original.lines.len(), 1);
        assert_eq!(result.lines.len(), 2);
        assert_eq!(result.lines[0], original.lines[0]);
        assert_eq!(result.lines[1].id, "subcomposicao-02");
        assert_eq!(result.lines[1].coefficient.to_string(), "0.75");
        assert_eq!(result.lines[1].unit, MeasureUnit::Unit);
        assert_eq!(result.lines[1].conversion, None);
        assert_eq!(
            result.lines[1].source,
            ComponentSource::Composition {
                code: "BASE-02".into(),
            }
        );
        for (key, invalid) in [
            ("new_line_id", "insumo-01"),
            ("new_line_id", "@soma"),
            ("source_code", "COMP-01"),
            ("source_code", ""),
            ("coefficient", "-1"),
            ("coefficient", "=1+1"),
            ("unit", "desconhecida"),
        ] {
            let old = fields.get(key).unwrap().clone();
            fields.insert(key.into(), invalid.into());
            assert!(changed_composition_from_form(original.clone(), &fields).is_none());
            fields.insert(key.into(), old);
        }
    }

    #[test]
    fn remove_line_rejects_incidence_references_and_preserves_source_revision() {
        use canteiro_domain::compositions::CostIncidence;
        let mut original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "remove_line".into());
        fields.insert("line_id".into(), "insumo-01".into());
        original.incidences = vec![CostIncidence {
            id: "encargo".into(),
            percent: "10".parse().unwrap(),
            base_lines: vec!["insumo-01".into()],
        }];
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        original.incidences.clear();
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert!(changed.lines.is_empty());
        assert_eq!(original.lines.len(), 1);
        assert_eq!(changed.code, original.code);
        fields.insert("line_id".into(), "absent".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn indices_change_is_validated_without_overwriting_the_saved_composition() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "costs".into());
        fields.insert("bdi".into(), "25".into());
        fields.insert("discount".into(), "5".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.bdi.to_string(), "25");
        assert_eq!(changed.discount.to_string(), "5");
        assert_eq!(original.bdi.to_string(), "20");
        assert_eq!(changed.lines, original.lines);

        fields.insert("bdi".into(), "25%".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("change_kind".into(), "invalid".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn incidence_percent_change_keeps_base_and_does_not_mutate_saved_revision() {
        use canteiro_domain::compositions::CostIncidence;
        let mut original = saved();
        original.incidences = vec![CostIncidence {
            id: "adicional-01".into(),
            percent: "12".parse().unwrap(),
            base_lines: vec!["insumo-01".into()],
        }];
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "incidence".into());
        fields.insert("incidence_id".into(), "adicional-01".into());
        fields.insert("incidence_percent".into(), "15.5".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.incidences[0].percent.to_string(), "15.5");
        assert_eq!(
            changed.incidences[0].base_lines,
            original.incidences[0].base_lines
        );
        assert_eq!(original.incidences[0].percent.to_string(), "12");
        assert_eq!(changed.lines, original.lines);

        fields.insert("incidence_percent".into(), "-1".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("incidence_percent".into(), "15%".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("incidence_percent".into(), "15".into());
        fields.insert("incidence_id".into(), "inexistente".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn add_incidence_creates_versionable_basis_without_touching_original() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "add_incidence".into());
        fields.insert("new_incidence_id".into(), "encargos-equipe".into());
        fields.insert("incidence_percent".into(), "12.5".into());
        fields.insert("base_lines".into(), "insumo-01".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert!(original.incidences.is_empty());
        assert_eq!(changed.incidences.len(), 1);
        assert_eq!(changed.incidences[0].id, "encargos-equipe");
        assert_eq!(changed.incidences[0].percent.to_string(), "12.5");
        assert_eq!(changed.incidences[0].base_lines, vec!["insumo-01"]);
        assert_eq!(changed.lines, original.lines);
        assert_eq!(changed.cost_mode, original.cost_mode);

        for (key, invalid) in [
            ("new_incidence_id", "=1+1"),
            ("new_incidence_id", ""),
            ("incidence_percent", "-2"),
            ("incidence_percent", "=12"),
            ("base_lines", "nao-existe"),
            ("base_lines", "insumo-01, insumo-01"),
            ("base_lines", "insumo-01,"),
        ] {
            let valid = fields.get(key).unwrap().clone();
            fields.insert(key.into(), invalid.into());
            assert!(changed_composition_from_form(original.clone(), &fields).is_none());
            fields.insert(key.into(), valid);
        }
        let mut existing = original;
        existing.incidences = changed.incidences;
        assert!(changed_composition_from_form(existing, &fields).is_none());
    }

    #[test]
    fn remove_incidence_keeps_inputs_and_other_bases_without_mutating_history() {
        use canteiro_domain::compositions::CostIncidence;
        let mut original = saved();
        original.incidences = vec![
            CostIncidence {
                id: "encargos-equipe".into(),
                percent: "12".parse().unwrap(),
                base_lines: vec!["insumo-01".into()],
            },
            CostIncidence {
                id: "transporte".into(),
                percent: "5".parse().unwrap(),
                base_lines: vec!["insumo-01".into()],
            },
        ];
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "remove_incidence".into());
        fields.insert("incidence_id".into(), "encargos-equipe".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.incidences, original.incidences[1..]);
        assert_eq!(changed.lines, original.lines);
        assert_eq!(original.incidences.len(), 2);
        fields.insert("incidence_id".into(), "inexistente".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn incidence_base_can_change_only_to_existing_distinct_lines() {
        use canteiro_domain::compositions::CostIncidence;
        let mut original = saved();
        let mut second = original.lines[0].clone();
        second.id = "insumo-02".into();
        original.lines.push(second);
        original.incidences = vec![CostIncidence {
            id: "encargo".into(),
            percent: "15".parse().unwrap(),
            base_lines: vec!["insumo-01".into()],
        }];
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "incidence_base".into());
        fields.insert("incidence_id".into(), "encargo".into());
        fields.insert("base_lines".into(), "insumo-02".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        assert_eq!(changed.incidences[0].base_lines, vec!["insumo-02"]);
        assert_eq!(
            changed.incidences[0].percent,
            original.incidences[0].percent
        );
        assert_eq!(changed.lines, original.lines);
        assert_eq!(original.incidences[0].base_lines, vec!["insumo-01"]);

        for invalid in ["", "insumo-03", "insumo-01, insumo-01", "insumo-01,"] {
            fields.insert("base_lines".into(), invalid.into());
            assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        }
        fields.insert("base_lines".into(), "insumo-02".into());
        fields.insert("incidence_id".into(), "inexistente".into());
        assert!(changed_composition_from_form(original, &fields).is_none());
    }

    #[test]
    fn informed_cost_can_change_without_switching_formula_mode() {
        let original = saved();
        let mut fields = BTreeMap::new();
        fields.insert("change_kind".into(), "informed_cost".into());
        fields.insert("informed_cost".into(), "250.75".into());
        // Uma revisão de preço precisa atualizar também o fundamento técnico.
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("cost_reason".into(), "Nova cotação verificada".into());
        let changed = changed_composition_from_form(original.clone(), &fields).unwrap();
        match changed.cost_mode {
            CostMode::Informed { cost, reason } => {
                assert_eq!(cost.to_string(), "250.750000");
                assert_eq!(reason, "Nova cotação verificada");
            }
            CostMode::Formula => panic!("O custo informado não deve virar fórmula"),
        }
        assert_eq!(changed.lines, original.lines);
        assert_eq!(changed.incidences, original.incidences);
        assert_eq!(original.cost_mode, saved().cost_mode);

        fields.insert("cost_reason".into(), "=HYPERLINK(\"http://host\")".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("cost_reason".into(), "Preço com\u{202e} marca".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("cost_reason".into(), "Nova cotação verificada".into());
        fields.insert("informed_cost".into(), "=100+20".into());
        assert!(changed_composition_from_form(original.clone(), &fields).is_none());
        fields.insert("informed_cost".into(), "200".into());
        let mut formula = original;
        formula.cost_mode = CostMode::Formula;
        assert!(changed_composition_from_form(formula, &fields).is_none());
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BudgetLineQuery {
    organization_id: String,
    context_version: i64,
    scope_id: String,
    item_id: String,
}

async fn budget_edit_line_page(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<BudgetLineQuery>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Ok(Query(query)) = query else {
        return bad_input();
    };
    let context = ReliabilityScope {
        organization_id: query.organization_id,
        context_version: query.context_version,
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context)
        || !page
            .scopes
            .iter()
            .any(|scope| scope.id == query.scope_id && scope.can_edit)
    {
        return bad_input();
    }
    let Some(record) = page
        .budgets
        .iter()
        .find(|entry| {
            entry.scope_id == query.scope_id
                && entry.snapshot.status != BudgetState::Approved
                && entry
                    .snapshot
                    .lines
                    .iter()
                    .any(|line| line.id == query.item_id)
        })
        .cloned()
    else {
        return bad_input();
    };
    html_page(
        canteiro_web::estimating::budget_line_editor(page, record, &query.item_id),
        StatusCode::OK,
    )
}

async fn budget_save_line_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let scope_id = field(&data, "scope_id").to_owned();
    let item_id = field(&data, "item_id").to_owned();
    let version = match field(&data, "expected_version").parse::<i64>() {
        Ok(value) if value > 0 => value,
        _ => return bad_input(),
    };
    let revision = match version.checked_add(1).and_then(|v| u32::try_from(v).ok()) {
        Some(value) => value,
        None => return bad_input(),
    };
    let quantity = match field(&data, "quantity").parse::<canteiro_domain::decimal::Quantity>() {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let price = match field(&data, "price").parse::<canteiro_domain::decimal::UnitPrice>() {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context)
        || !page
            .scopes
            .iter()
            .any(|scope| scope.id == scope_id && scope.can_edit)
    {
        return bad_input();
    }
    let Some(record) = page
        .budgets
        .iter()
        .find(|entry| {
            entry.scope_id == scope_id
                && entry.version == version
                && entry.snapshot.status != BudgetState::Approved
        })
        .cloned()
    else {
        let mut response = (
            StatusCode::CONFLICT,
            "O orçamento mudou desde a abertura do formulário. Confira a revisão antes de editar.",
        )
            .into_response();
        private_headers(&mut response);
        return response;
    };
    let mut snapshot = record.snapshot;
    if snapshot
        .edit_line_values(&item_id, quantity, price)
        .is_err()
    {
        return bad_input();
    }
    let lease_key = uuid::Uuid::new_v4().to_string();
    let lease = auth
        .service()
        .estimating_budget_lease(
            &token,
            BudgetLeaseCommand {
                context: context.clone(),
                scope_id: scope_id.clone(),
                lease_key: lease_key.clone(),
            },
        )
        .await;
    match lease {
        Ok(reply) if reply.revision == revision => {}
        Ok(_) => {
            let mut response = (
                StatusCode::CONFLICT,
                "Uma revisão mais recente foi registrada. Confira os dados antes de salvar.",
            )
                .into_response();
            private_headers(&mut response);
            return response;
        }
        Err(error) => return application_failure(error),
    }
    let command = BudgetCommand {
        context,
        scope_id,
        operation_key: uuid::Uuid::new_v4().to_string(),
        expected_version: Some(version),
        lease_key: Some(lease_key),
        reason: field(&data, "reason").to_owned(),
        change: BudgetChange::EditLineValues {
            line_id: item_id,
            quantity,
            price,
        },
    };
    match auth
        .service()
        .estimating_budget_execute(&token, command)
        .await
    {
        Ok(_) => match auth.service().estimating_context(&token).await {
            Ok(updated) => html_page(canteiro_web::estimating::page(updated), StatusCode::OK),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

/// Copiar ou mover um serviço exige reserva, concorrência e auditoria da edição.
/// A identidade e a origem do preço original são preservadas.
async fn budget_transfer_line_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let scope_id = field(&data, "scope_id").to_owned();
    let version = match field(&data, "expected_version").parse::<i64>() {
        Ok(value) if value > 0 => value,
        _ => return bad_input(),
    };
    let revision = match version.checked_add(1).and_then(|v| u32::try_from(v).ok()) {
        Some(value) => value,
        None => return bad_input(),
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context)
        || !page
            .scopes
            .iter()
            .any(|scope| scope.id == scope_id && scope.can_edit)
    {
        return bad_input();
    }
    let Some(record) = page.budgets.iter().find(|entry| {
        entry.scope_id == scope_id
            && entry.version == version
            && entry.snapshot.status != BudgetState::Approved
    }) else {
        let mut response = (
            StatusCode::CONFLICT,
            "A revisão mudou ou não permite cópia. Atualize o orçamento antes de continuar.",
        )
            .into_response();
        private_headers(&mut response);
        return response;
    };
    if field(&data, "operation") == "delete" && field(&data, "confirm_delete") != "yes" {
        return bad_input();
    }
    let mut snapshot = record.snapshot.clone();
    let result = match field(&data, "operation") {
        "copy" => snapshot.copy_line(
            field(&data, "item_id"),
            field(&data, "new_item_id"),
            field(&data, "variation_id"),
        ),
        "move" => snapshot.move_line(field(&data, "item_id"), field(&data, "variation_id")),
        "delete" => snapshot.remove_unlinked_line(field(&data, "item_id")),
        _ => return bad_input(),
    };
    if result.is_err() {
        return bad_input();
    }
    let lease_key = uuid::Uuid::new_v4().to_string();
    match auth
        .service()
        .estimating_budget_lease(
            &token,
            BudgetLeaseCommand {
                context: context.clone(),
                scope_id: scope_id.clone(),
                lease_key: lease_key.clone(),
            },
        )
        .await
    {
        Ok(reply) if reply.revision == revision => {}
        Ok(_) => {
            let mut response = (
                StatusCode::CONFLICT,
                "A versão mudou enquanto você preparava a cópia. Confira novamente.",
            )
                .into_response();
            private_headers(&mut response);
            return response;
        }
        Err(error) => return application_failure(error),
    }
    let command = BudgetCommand {
        context,
        scope_id,
        operation_key: uuid::Uuid::new_v4().to_string(),
        expected_version: Some(version),
        lease_key: Some(lease_key),
        reason: field(&data, "reason").to_owned(),
        change: match field(&data, "operation") {
            "copy" => BudgetChange::CopyLine {
                source_id: field(&data, "item_id").to_owned(),
                new_id: field(&data, "new_item_id").to_owned(),
                target_variation_id: field(&data, "variation_id").to_owned(),
            },
            "move" => BudgetChange::MoveLine {
                line_id: field(&data, "item_id").to_owned(),
                target_variation_id: field(&data, "variation_id").to_owned(),
            },
            "delete" => BudgetChange::RemoveLine {
                line_id: field(&data, "item_id").to_owned(),
            },
            _ => return bad_input(),
        },
    };
    match auth
        .service()
        .estimating_budget_execute(&token, command)
        .await
    {
        Ok(_) => match auth.service().estimating_context(&token).await {
            Ok(updated) => html_page(canteiro_web::estimating::page(updated), StatusCode::OK),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

/// Revisão explícita do cronograma sem reescrever preços ou serviços.
/// A versão é verificada antes da reserva e novamente na transação.
async fn budget_save_schedule_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let scope_id = field(&data, "scope_id").to_owned();
    let version = match field(&data, "expected_version").parse::<i64>() {
        Ok(value) if value > 0 => value,
        _ => return bad_input(),
    };
    let revision = match version.checked_add(1).and_then(|v| u32::try_from(v).ok()) {
        Some(value) => value,
        None => return bad_input(),
    };
    if field(&data, "confirm_schedule") != "yes" {
        return bad_input();
    }
    let starts_on = match field(&data, "starts_on").parse::<canteiro_domain::CivilDate>() {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let deadline_days = match field(&data, "deadline_days").parse::<u32>() {
        Ok(value) if value > 0 => value,
        _ => return bad_input(),
    };
    let periods = match field(&data, "periods").parse::<u32>() {
        Ok(value) if (1..=10000).contains(&value) => value,
        _ => return bad_input(),
    };
    let basis = match field(&data, "basis") {
        "fixed_30_days" => canteiro_domain::budgets::PeriodBasis::Fixed30Days,
        "civil_months" => canteiro_domain::budgets::PeriodBasis::CivilMonths,
        _ => return bad_input(),
    };
    let page = match auth.service().estimating_context(&token).await {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if page.context.as_ref() != Some(&context)
        || !page
            .scopes
            .iter()
            .any(|scope| scope.id == scope_id && scope.can_edit)
    {
        return bad_input();
    }
    let Some(_record) = page.budgets.iter().find(|entry| {
        entry.scope_id == scope_id
            && entry.version == version
            && entry.snapshot.status != BudgetState::Approved
    }) else {
        let mut response = (
            StatusCode::CONFLICT,
            "O orçamento mudou desde a abertura do cronograma. Confira a revisão atual.",
        )
            .into_response();
        private_headers(&mut response);
        return response;
    };
    if canteiro_domain::budgets::BudgetSchedule::linear(starts_on, deadline_days, basis, periods)
        .is_err()
    {
        return bad_input();
    }
    let lease_key = uuid::Uuid::new_v4().to_string();
    match auth
        .service()
        .estimating_budget_lease(
            &token,
            BudgetLeaseCommand {
                context: context.clone(),
                scope_id: scope_id.clone(),
                lease_key: lease_key.clone(),
            },
        )
        .await
    {
        Ok(reply) if reply.revision == revision => {}
        Ok(_) => {
            let mut response = (
                StatusCode::CONFLICT,
                "A revisão foi alterada enquanto você preparava o cronograma.",
            )
                .into_response();
            private_headers(&mut response);
            return response;
        }
        Err(error) => return application_failure(error),
    }
    let command = BudgetCommand {
        context,
        scope_id,
        operation_key: uuid::Uuid::new_v4().to_string(),
        expected_version: Some(version),
        lease_key: Some(lease_key),
        reason: field(&data, "reason").to_owned(),
        change: BudgetChange::RescheduleLinear {
            starts_on,
            deadline_days,
            basis,
            periods,
        },
    };
    match auth
        .service()
        .estimating_budget_execute(&token, command)
        .await
    {
        Ok(_) => match auth.service().estimating_context(&token).await {
            Ok(updated) => html_page(canteiro_web::estimating::page(updated), StatusCode::OK),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

async fn budget_lease(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<BudgetLeaseCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth.service().estimating_budget_lease(&token, input).await {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
async fn budget_write(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<BudgetCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_budget_execute(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
async fn import_preview(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<BudgetImportPreviewCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_budget_import_preview(&token, input)
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
async fn budget_history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(scope): Path<String>,
    query: Result<Query<ReliabilityScope>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(context)) = query else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_budget_history(&token, &context, &scope)
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportQuery {
    organization_id: String,
    context_version: i64,
    locale: NumberLocale,
}
async fn budget_export(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(scope): Path<String>,
    query: Result<Query<ExportQuery>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(query)) = query else {
        return bad_input();
    };
    let context = ReliabilityScope {
        organization_id: query.organization_id,
        context_version: query.context_version,
    };
    match auth
        .service()
        .estimating_budget_export_csv(&token, &context, &scope, query.locale)
        .await
    {
        Ok(csv) => {
            let mut response = csv.into_response();
            private_headers(&mut response);
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"canteiro-orcamento.csv\""),
            );
            response
        }
        Err(e) => application_failure(e),
    }
}
async fn page(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    match auth.service().estimating_context(&token).await {
        Ok(ctx) => html_page(canteiro_web::estimating::page(ctx), StatusCode::OK),
        Err(e) => application_failure(e),
    }
}

fn catalog_csv_from_form(data: &BTreeMap<String, String>) -> Result<CatalogCsvImportCommand, ()> {
    let context = form_context(data).map_err(|_| ())?;
    let edition = field(data, "edition").parse::<u32>().map_err(|_| ())?;
    Ok(CatalogCsvImportCommand {
        context,
        scope_id: field(data, "scope_id").to_owned(),
        catalog_id: field(data, "catalog_id").to_owned(),
        edition,
        published_on: field(data, "published_on").to_owned(),
        source: CostSource {
            title: field(data, "source_title").to_owned(),
            organization: field(data, "source_organization").to_owned(),
            license: field(data, "source_license").to_owned(),
            locality: field(data, "source_locality").to_owned(),
            tax_regime: field(data, "source_tax_regime").to_owned(),
            competence: field(data, "source_competence").to_owned(),
        },
        reason: field(data, "reason").to_owned(),
        csv: field(data, "csv").to_owned(),
        locale: NumberLocale::PtBr,
    })
}

async fn catalog_csv_preview_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Ok(input) = catalog_csv_from_form(&data) else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_catalog_csv_preview(&token, input.clone())
        .await
    {
        Ok(result) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(
                canteiro_web::estimating::catalog_csv_preview(ctx, result, input),
                StatusCode::OK,
            ),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

async fn catalog_csv_publish_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let Ok(input) = catalog_csv_from_form(&data) else {
        return bad_input();
    };
    match auth
        .service()
        .estimating_catalog_csv_publish(&token, input)
        .await
    {
        Ok(header) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(
                canteiro_web::estimating::catalog_csv_published(ctx, header),
                StatusCode::OK,
            ),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

async fn preview_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let command = BudgetImportPreviewCommand {
        context,
        scope_id: field(&data, "scope_id").to_owned(),
        csv: field(&data, "csv").to_owned(),
        locale: NumberLocale::PtBr,
    };
    match auth
        .service()
        .estimating_budget_import_preview(&token, command.clone())
        .await
    {
        Ok(value) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(
                canteiro_web::estimating::preview(ctx, value, command),
                StatusCode::OK,
            ),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

// A confirmação exige a identidade durável da prévia, sem chave substituta.
fn confirmed_import_batch_id<'a>(staged: Option<&'a str>, supplied: &str) -> Option<&'a str> {
    let batch = staged?;
    let parsed = uuid::Uuid::parse_str(batch).ok()?;
    if parsed.is_nil() || parsed.to_string() != batch || batch != supplied {
        return None;
    }
    Some(batch)
}

async fn publish_budget_csv_form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(error) => return *error,
    };
    let context = match form_context(&data) {
        Ok(value) => value,
        Err(_) => return bad_input(),
    };
    let expected_version = if field(&data, "expected_version").is_empty() {
        None
    } else {
        match field(&data, "expected_version").parse::<i64>() {
            Ok(value) if value > 0 => Some(value),
            _ => return bad_input(),
        }
    };
    let input = BudgetImportPreviewCommand {
        context: context.clone(),
        scope_id: field(&data, "scope_id").to_owned(),
        csv: field(&data, "csv").to_owned(),
        locale: NumberLocale::PtBr,
    };
    // Refazer toda a validação no servidor: o resultado mostrado na prévia
    // não é uma autorização para gravar um CSV alterado ou uma revisão antiga.
    let preview = match auth
        .service()
        .estimating_budget_import_preview(&token, input.clone())
        .await
    {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    let confirmed_batch_id = match confirmed_import_batch_id(
        preview.batch_id.as_deref(),
        field(&data, "batch_id"),
    ) {
        Some(batch) if preview.base_version == expected_version => batch.to_owned(),
        _ => {
            let mut response = (
                StatusCode::CONFLICT,
                "O orçamento ou o lote mudou desde a prévia. Confira novamente o arquivo antes de publicar.",
            )
                .into_response();
            private_headers(&mut response);
            return response;
        }
    };
    let lease_key = uuid::Uuid::new_v4().to_string();
    let lease = match auth
        .service()
        .estimating_budget_lease(
            &token,
            BudgetLeaseCommand {
                context: context.clone(),
                scope_id: input.scope_id.clone(),
                lease_key: lease_key.clone(),
            },
        )
        .await
    {
        Ok(value) => value,
        Err(error) => return application_failure(error),
    };
    if lease.revision != preview.snapshot.revision {
        let mut response = (
            StatusCode::CONFLICT,
            "A revisão foi alterada durante a importação. Refaça a conferência.",
        )
            .into_response();
        private_headers(&mut response);
        return response;
    }
    let command = BudgetCommand {
        context,
        scope_id: input.scope_id,
        // A operação da importação é a identidade durável da prévia conferida.
        operation_key: confirmed_batch_id,
        expected_version,
        lease_key: Some(lease_key),
        reason: field(&data, "reason").to_owned(),
        change: BudgetChange::Save {
            snapshot: preview.snapshot,
        },
    };
    match auth
        .service()
        .estimating_budget_execute(&token, command)
        .await
    {
        Ok(saved) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(
                canteiro_web::estimating::budget_csv_published(ctx, saved),
                StatusCode::OK,
            ),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}

#[cfg(test)]
mod budget_import_batch_confirmation_tests {
    use super::confirmed_import_batch_id;

    #[test]
    fn confirmation_requires_a_canonical_staged_batch_without_fallback() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(confirmed_import_batch_id(Some(id), id), Some(id));
        for staged in [
            None,
            Some(""),
            Some("not-a-uuid"),
            Some("00000000-0000-0000-0000-000000000000"),
        ] {
            assert_eq!(confirmed_import_batch_id(staged, id), None);
        }
        assert_eq!(confirmed_import_batch_id(Some(id), ""), None);
        assert_eq!(
            confirmed_import_batch_id(Some(id), "550e8400-e29b-41d4-a716-446655440001"),
            None
        );
        assert_eq!(
            confirmed_import_batch_id(Some("550E8400-E29B-41D4-A716-446655440000"), id),
            None
        );
    }
}
