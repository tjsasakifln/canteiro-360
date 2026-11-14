use canteiro_contracts::{
    auth::AuthFailure,
    organizations::{MemberSearch, OrganizationCommand, OrganizationResponse},
};
use serde::de::DeserializeOwned;

pub async fn context() -> Result<OrganizationResponse, AuthFailure> {
    request("/api/organizations/context", None, None).await
}

pub async fn command(
    input: &OrganizationCommand,
    csrf: &str,
) -> Result<OrganizationResponse, AuthFailure> {
    let body = serde_json::to_string(input).map_err(|_| unavailable())?;
    request("/api/organizations/command", Some(body), Some(csrf)).await
}

pub fn query_component(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(char::from(*byte));
        } else {
            use std::fmt::Write;
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

pub fn members_url(
    organization: &str,
    generation: i64,
    query: &str,
    offset: i64,
    export: bool,
) -> String {
    format!(
        "/api/organizations/{}/members{}?context_version={generation}&query={}&offset={offset}&limit=50",
        query_component(organization),
        if export { ".tsv" } else { "" },
        query_component(query)
    )
}

pub async fn members(
    organization: &str,
    generation: i64,
    query: &str,
    offset: i64,
) -> Result<MemberSearch, AuthFailure> {
    request(
        &members_url(organization, generation, query, offset, false),
        None,
        None,
    )
    .await
}

fn unavailable() -> AuthFailure {
    AuthFailure {
        code: "network_unavailable".into(),
        message: "Não foi possível confirmar a resposta. Seus campos continuam nesta aba. Confira a conexão e atualize a empresa antes de repetir uma operação.".into(),
    }
}

async fn request<T: DeserializeOwned>(
    path: &str,
    body: Option<String>,
    csrf: Option<&str>,
) -> Result<T, AuthFailure> {
    crate::http::request(
        path,
        body,
        crate::http::RequestOptions {
            csrf,
            ..Default::default()
        },
        unavailable,
    )
    .await
    .map_err(|failure| failure.details)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn member_search_parameters_cannot_change_scope_or_path() {
        assert_eq!(
            query_component("á&context_version=8/#"),
            "%C3%A1%26context_version%3D8%2F%23"
        );
        let url = members_url("empresa/2", 3, "x&limit=900", 50, true);
        assert!(url.starts_with("/api/organizations/empresa%2F2/members.tsv?context_version=3&"));
        assert!(url.contains("query=x%26limit%3D900&offset=50&limit=50"));
    }
}
