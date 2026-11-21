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
            Ok(ctx) => html_page(
                canteiro_web::estimating::catalog_comparison(ctx, result),
                StatusCode::OK,
            ),
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
        .estimating_budget_import_preview(&token, command)
        .await
    {
        Ok(value) => match auth.service().estimating_context(&token).await {
            Ok(ctx) => html_page(
                canteiro_web::estimating::preview(ctx, value),
                StatusCode::OK,
            ),
            Err(error) => application_failure(error),
        },
        Err(error) => application_failure(error),
    }
}
