use crate::{
    auth::{AuthHttp, application_failure, failure, private_headers, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::{organizations::OrganizationScope, parties::PartyCommand};
use serde::Deserialize;
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/parties/context", get(context))
        .route("/api/parties/command", post(command))
        .route("/api/parties/export.json", get(export))
        .route("/api/parties/{party}", get(detail))
        .route("/api/parties/{party}/history", get(history))
        .route("/api/parties/{party}/bank", get(bank))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    #[serde(default)]
    query: String,
    #[serde(default)]
    page: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportQuery {
    organization_id: String,
    context_version: i64,
    organization_version: i64,
    #[serde(default)]
    query: String,
}
fn invalid() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira os dados, a busca e a empresa selecionada.",
    )
}
fn authenticated(
    auth: Option<Extension<AuthHttp>>,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<(AuthHttp, String), Box<Response>> {
    let Some(Extension(auth)) = auth else {
        return Err(Box::new(unavailable()));
    };
    let token =
        authorize_http(&auth, headers, mutation).map_err(|r| Box::new(r.into_response()))?;
    Ok((auth, token))
}
async fn context(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<Search>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(q)) = query else {
        return invalid();
    };
    match auth
        .service()
        .parties_context(&token, &q.query, q.page)
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<PartyCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(command)) = input else {
        return invalid();
    };
    match auth.service().parties_execute(&token, command).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn detail(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(party): Path<String>,
    query: Result<Query<OrganizationScope>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(scope)) = query else {
        return invalid();
    };
    match auth.service().party_detail(&token, &scope, &party).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(party): Path<String>,
    query: Result<Query<OrganizationScope>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(scope)) = query else {
        return invalid();
    };
    match auth.service().party_revisions(&token, &scope, &party).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn bank(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(party): Path<String>,
    query: Result<Query<OrganizationScope>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(scope)) = query else {
        return invalid();
    };
    match auth
        .service()
        .party_bank_history(&token, &scope, &party)
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn export(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<ExportQuery>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(q)) = query else {
        return invalid();
    };
    let scope = OrganizationScope {
        organization_id: q.organization_id,
        context_version: q.context_version,
        organization_version: q.organization_version,
    };
    match auth
        .service()
        .parties_export(&token, &scope, &q.query)
        .await
    {
        Ok(rows) => {
            let mut response = private_json(rows);
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=cadastros.json"),
            );
            private_headers(&mut response);
            response
        }
        Err(e) => application_failure(e),
    }
}
