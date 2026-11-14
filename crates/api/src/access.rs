//! A identidade deriva do cookie. O corpo informa intenção, nunca um papel confiável.
use crate::{
    auth::{AuthHttp, application_failure, failure, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::access::AccessCommand;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/access/context", get(context))
        .route("/api/access/command", post(command))
        .layer(DefaultBodyLimit::max(16_384))
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
    match auth.service().access_execute(&token, input).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}
