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
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;
        let window = web_sys::window().ok_or_else(unavailable)?;
        let options = web_sys::RequestInit::new();
        options.set_method(if body.is_some() { "POST" } else { "GET" });
        options.set_credentials(web_sys::RequestCredentials::SameOrigin);
        options.set_signal(Some(&web_sys::AbortSignal::timeout_with_u32(20_000)));
        let headers = web_sys::Headers::new().map_err(|_| unavailable())?;
        headers
            .set("Accept", "application/json")
            .map_err(|_| unavailable())?;
        if let Some(body) = &body {
            options.set_body(&JsValue::from_str(body));
            headers
                .set("Content-Type", "application/json")
                .map_err(|_| unavailable())?;
        }
        if let Some(csrf) = csrf {
            headers
                .set("X-CSRF-Token", csrf)
                .map_err(|_| unavailable())?;
        }
        options.set_headers(&headers);
        let response = JsFuture::from(window.fetch_with_str_and_init(path, &options))
            .await
            .map_err(|_| unavailable())?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| unavailable())?;
        let text = JsFuture::from(response.text().map_err(|_| unavailable())?)
            .await
            .map_err(|_| unavailable())?
            .as_string()
            .ok_or_else(unavailable)?;
        if response.ok() {
            serde_json::from_str(&text).map_err(|_| unavailable())
        } else {
            Err(serde_json::from_str(&text).unwrap_or_else(|_| unavailable()))
        }
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (path, body, csrf);
        Err(unavailable())
    }
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
