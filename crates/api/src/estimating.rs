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
    let mut composition = record.composition.clone();
    composition.description = field(&data, "description").to_owned();
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
    snapshot.revision = revision;
    snapshot.status = BudgetState::Draft;
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
        change: BudgetChange::Save { snapshot },
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
    let mut snapshot = record.snapshot.clone();
    let result = match field(&data, "operation") {
        "copy" => snapshot.copy_line(
            field(&data, "item_id"),
            field(&data, "new_item_id"),
            field(&data, "variation_id"),
        ),
        "move" => snapshot.move_line(field(&data, "item_id"), field(&data, "variation_id")),
        _ => return bad_input(),
    };
    if result.is_err() {
        return bad_input();
    }
    snapshot.revision = revision;
    snapshot.status = BudgetState::Draft;
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
        change: BudgetChange::Save { snapshot },
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
