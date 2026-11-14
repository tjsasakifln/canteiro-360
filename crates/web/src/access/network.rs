use crate::{http::RequestOptions, organizations::network::query_component};
use canteiro_contracts::{
    access::{AccessCommand, AccessResponse},
    auth::AuthFailure,
    organizations::OrganizationScope,
};
use serde::de::DeserializeOwned;

pub async fn context() -> Result<AccessResponse, AuthFailure> {
    request("/api/access/context", None, None, None).await
}

pub async fn command(
    command: &AccessCommand,
    csrf: &str,
    key: Option<&str>,
) -> Result<AccessResponse, AuthFailure> {
    let body = serde_json::to_string(command).map_err(|_| unavailable())?;
    request("/api/access/command", Some(body), Some(csrf), key).await
}

pub async fn authority_result(
    scope: &OrganizationScope,
    act: &str,
    key: &str,
) -> Result<Option<AccessResponse>, AuthFailure> {
    let path = format!(
        "/api/access/operations/{}/{}?organization_id={}&context_version={}",
        query_component(act),
        query_component(key),
        query_component(&scope.organization_id),
        scope.context_version
    );
    request(&path, None, None, None).await
}

fn unavailable() -> AuthFailure {
    AuthFailure {
        code: "network_unavailable".into(),
        message: "Não foi possível confirmar a resposta. Seus campos continuam nesta aba. Confira a conexão e atualize o acesso antes de repetir uma operação.".into(),
    }
}

async fn request<T: DeserializeOwned>(
    path: &str,
    body: Option<String>,
    csrf: Option<&str>,
    key: Option<&str>,
) -> Result<T, AuthFailure> {
    crate::http::request(
        path,
        body,
        RequestOptions {
            csrf,
            operation_key: key,
            ..RequestOptions::default()
        },
        unavailable,
    )
    .await
    .map_err(|failure| failure.details)
}
