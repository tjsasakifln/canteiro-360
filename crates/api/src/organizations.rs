//! Fronteira de organizações. Identidade vem exclusivamente do cookie validado.
use crate::auth::{
    AuthHttp, application_failure, failure, private_headers, session_cookie, unavailable,
};
use axum::{
    Extension, Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::organizations::OrganizationCommand;
use serde::Deserialize;

pub(crate) struct HttpRejection(StatusCode, &'static str, &'static str);

impl IntoResponse for HttpRejection {
    fn into_response(self) -> Response {
        failure(self.0, self.1, self.2)
    }
}

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/organizations/context", get(context))
        .route("/api/organizations/command", post(command))
        .route("/api/organizations/{id}/members", get(members))
        .route("/api/organizations/{id}/members.tsv", get(export))
        .layer(DefaultBodyLimit::max(8_192))
}

pub(crate) fn authorize_http(
    auth: &AuthHttp,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<String, HttpRejection> {
    let origins = headers.get_all(header::ORIGIN).iter().count();
    if origins > 1
        || mutation && origins != 1
        || headers
            .get(header::ORIGIN)
            .is_some_and(|v| v.as_bytes() != auth.origin.as_bytes())
        || headers.get("sec-fetch-site").is_some_and(|v| {
            if mutation {
                v != "same-origin"
            } else {
                v != "same-origin" && v != "none"
            }
        })
    {
        return Err(HttpRejection(
            StatusCode::FORBIDDEN,
            "origin_invalid",
            "Abra esta página neste site e tente novamente.",
        ));
    }
    let token = match session_cookie(headers) {
        Ok(Some(token)) => token,
        _ => {
            return Err(HttpRejection(
                StatusCode::UNAUTHORIZED,
                "session_expired",
                "Entre na sua conta para continuar. Seus campos preenchidos podem ser mantidos abertos.",
            ));
        }
    };
    if mutation
        && (headers.get_all("x-csrf-token").iter().count() != 1
            || !headers
                .get("x-csrf-token")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| auth.service().verify_csrf(&token, v)))
    {
        return Err(HttpRejection(
            StatusCode::FORBIDDEN,
            "csrf_invalid",
            "Atualize o acesso antes de tentar novamente. A operação não foi gravada.",
        ));
    }
    Ok(token)
}

async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return r.into_response(),
    };
    match auth.service().organizations_context(&token).await {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}

async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<OrganizationCommand>, JsonRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return r.into_response(),
    };
    let Json(command) = match input {
        Ok(v) => v,
        Err(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_input",
                "Confira os campos informados. A operação não foi gravada.",
            );
        }
    };
    match auth.service().organizations_execute(&token, command).await {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchQuery {
    context_version: i64,
    #[serde(default)]
    query: String,
    #[serde(default)]
    offset: i64,
    #[serde(default = "page_size")]
    limit: i64,
}
fn page_size() -> i64 {
    50
}
fn invalid_query() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira a empresa, a busca e a página solicitadas.",
    )
}

async fn members(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return r.into_response(),
    };
    let (Ok(Path(id)), Ok(Query(query))) = (path, query) else {
        return invalid_query();
    };
    match auth
        .service()
        .organizations_members(
            &token,
            &id,
            query.context_version,
            &query.query,
            query.offset,
            query.limit,
        )
        .await
    {
        Ok(result) => private_json(result),
        Err(e) => application_failure(e),
    }
}
async fn export(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    path: Result<Path<String>, PathRejection>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return r.into_response(),
    };
    let (Ok(Path(id)), Ok(Query(query))) = (path, query) else {
        return invalid_query();
    };
    match auth
        .service()
        .organizations_export(&token, &id, query.context_version, &query.query)
        .await
    {
        Ok(output) => {
            let mut response = output.into_response();
            private_headers(&mut response);
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/tab-separated-values; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"membros.tsv\""),
            );
            response
        }
        Err(e) => application_failure(e),
    }
}
pub(crate) fn private_json<T: serde::Serialize>(value: T) -> Response {
    let mut response = Json(value).into_response();
    private_headers(&mut response);
    response
}
