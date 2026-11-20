use crate::{
    auth::{AuthHttp, application_failure, failure, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::{
    organizations::OrganizationScope,
    portfolio::{PortfolioCommand, WorkStatus},
};
use serde::Deserialize;
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/portfolio/context", get(context))
        .route("/api/portfolio/command", post(command))
        .route("/api/portfolio/{work}", get(detail))
        .route("/api/portfolio/{work}/history", get(history))
        .layer(DefaultBodyLimit::max(128 * 1024))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    #[serde(default)]
    query: String,
    status: Option<WorkStatus>,
}
fn invalid() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira os campos da obra e a empresa selecionada.",
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
        .portfolio_context(&token, &q.query, q.status)
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<PortfolioCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(command)) = input else {
        return invalid();
    };
    if let Err(e) = command.input.clone().validated() {
        return failure(StatusCode::BAD_REQUEST, "invalid_input", &e.to_string());
    }
    match auth.service().portfolio_execute(&token, command).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn detail(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(work): Path<String>,
    query: Result<Query<OrganizationScope>, QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(scope)) = query else {
        return invalid();
    };
    match auth.service().portfolio_detail(&token, &scope, &work).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(work): Path<String>,
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
        .portfolio_revisions(&token, &scope, &work)
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
