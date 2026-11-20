//! Uploads privados e links curtos que revalidam sessão, vínculo e versão.
use crate::{
    auth::{AuthHttp, application_failure, failure, private_headers, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::documents::*;
use serde::Deserialize;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/documents/context", get(context))
        .route("/api/documents", get(list))
        .route("/api/documents/upload", post(upload))
        .route("/api/documents/retry", post(retry))
        .route("/api/documents/link", post(link))
        .route("/api/documents/signature", post(signature))
        .route("/api/documents/download/{ticket}", get(download))
        .layer(DefaultBodyLimit::max(
            canteiro_domain::documents::MAX_DOCUMENT_BYTES.div_ceil(3) * 4 + 8192,
        ))
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
fn invalid() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira os campos, o arquivo e a referência selecionada. Seu formulário permanece disponível.",
    )
}

async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    match auth.service().documents_context(&token).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListQuery {
    organization_id: String,
    context_version: i64,
    scope_id: String,
    before: Option<String>,
}

async fn list(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<ListQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(q)) = query else {
        return invalid();
    };
    let scope = DocumentScope {
        organization_id: q.organization_id,
        context_version: q.context_version,
        scope_id: q.scope_id,
    };
    match auth
        .service()
        .documents_list(&token, &scope, q.before.as_deref())
        .await
    {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}

async fn upload(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<UploadDocument>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(input)) = input else {
        return invalid();
    };
    match auth.service().document_upload_new(&token, input).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}

async fn retry(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<RetryDocumentUpload>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(input)) = input else {
        return invalid();
    };
    match auth.service().document_upload_retry(&token, input).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}

async fn link(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<DocumentLinkRequest>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(input)) = input else {
        return invalid();
    };
    match auth.service().document_download_link(&token, input).await {
        Ok(v) => private_json(v),
        Err(e) => application_failure(e),
    }
}

async fn signature(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<RecordDocumentSignature>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Json(input)) = input else {
        return invalid();
    };
    match auth
        .service()
        .document_record_signature(&token, input)
        .await
    {
        Ok(()) => private_json(
            serde_json::json!({"message":"Registro de assinatura associado à versão e ao resumo do arquivo. A autenticidade não foi verificada e não há aprovação interna automática."}),
        ),
        Err(e) => application_failure(e),
    }
}

async fn download(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(ticket): Path<String>,
    query: Result<Query<DocumentScope>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let Ok(Query(scope)) = query else {
        return invalid();
    };
    let download = match auth
        .service()
        .document_download(&token, &scope, &ticket)
        .await
    {
        Ok(v) => v,
        Err(e) => return application_failure(e),
    };
    let mut response = download.content.into_response();
    let content_type = HeaderValue::from_str(&download.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    let disposition = if download.preview {
        "inline"
    } else {
        "attachment"
    };
    let encoded = download
        .file_name
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect::<String>();
    if let Ok(value) = HeaderValue::from_str(&format!(
        "{disposition}; filename=\"documento\"; filename*=UTF-8''{encoded}"
    )) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static("default-src 'none'; sandbox"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    private_headers(&mut response);
    response
}
