//! A identidade deriva do cookie. O corpo informa intenção, nunca um papel confiável.
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
use canteiro_contracts::{access::AccessCommand, reliability::ReliabilityScope};

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/access/context", get(context))
        .route("/api/access/command", post(command))
        .route("/api/access/operations/{act}/{key}", get(operation))
        .layer(DefaultBodyLimit::max(16_384))
}
async fn operation(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path((act, key)): Path<(String, String)>,
    scope: Result<Query<ReliabilityScope>, QueryRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let Ok(Query(scope)) = scope else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "Confira a empresa e o identificador da operação.",
        );
    };
    match auth
        .service()
        .access_operation(&token, &scope, &act, &key)
        .await
    {
        Ok(result) => private_json(result),
        Err(error) => application_failure(error),
    }
}
async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match auth.service().access_context(&token).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<AccessCommand>, JsonRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    let token = match authorize_http(&auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    let Ok(Json(input)) = input else {
        return failure(
            StatusCode::BAD_REQUEST,
            "invalid_input",
            "Confira os campos, valores e datas. A operação não foi gravada.",
        );
    };
    let mut values = headers.get_all("idempotency-key").iter();
    let key = match (values.next(), values.next()) {
        (None, None) => None,
        (Some(value), None) if value.to_str().is_ok() => value.to_str().ok(),
        _ => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_input",
                "Informe uma única chave de operação válida.",
            );
        }
    };
    match auth
        .service()
        .access_execute_with_key(&token, input, key)
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
