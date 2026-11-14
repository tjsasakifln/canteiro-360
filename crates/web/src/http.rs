use canteiro_contracts::auth::AuthFailure;
use serde::de::DeserializeOwned;

/// Identidade aleatória de uma tentativa. Tokens de sessão continuam em cookies.
pub(crate) fn operation_id() -> Result<String, AuthFailure> {
    #[cfg(feature = "hydrate")]
    if let Some(crypto) = web_sys::window().and_then(|window| window.crypto().ok()) {
        return Ok(crypto.random_uuid());
    }
    Err(AuthFailure {
        code: "identity_unavailable".into(),
        message: "Reabra a página em uma conexão segura para identificar a operação.".into(),
    })
}

#[derive(Debug)]
pub(crate) struct RequestFailure {
    pub status: Option<u16>,
    pub details: AuthFailure,
}

#[derive(Default)]
pub(crate) struct RequestOptions<'a> {
    pub csrf: Option<&'a str>,
    pub operation_key: Option<&'a str>,
    pub accept_typed_conflict: bool,
}

pub(crate) async fn request<T: DeserializeOwned>(
    path: &str,
    body: Option<String>,
    settings: RequestOptions<'_>,
    unavailable: fn() -> AuthFailure,
) -> Result<T, RequestFailure> {
    let RequestOptions {
        csrf,
        operation_key,
        accept_typed_conflict,
    } = settings;
    let network_error = || RequestFailure {
        status: None,
        details: unavailable(),
    };
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::{JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;
        let window = web_sys::window().ok_or_else(network_error)?;
        let options = web_sys::RequestInit::new();
        options.set_method(if body.is_some() { "POST" } else { "GET" });
        options.set_credentials(web_sys::RequestCredentials::SameOrigin);
        options.set_signal(Some(&web_sys::AbortSignal::timeout_with_u32(20_000)));
        let headers = web_sys::Headers::new().map_err(|_| network_error())?;
        headers
            .set("Accept", "application/json")
            .map_err(|_| network_error())?;
        if let Some(body) = &body {
            options.set_body(&JsValue::from_str(body));
            headers
                .set("Content-Type", "application/json")
                .map_err(|_| network_error())?;
        }
        if let Some(csrf) = csrf {
            headers
                .set("X-CSRF-Token", csrf)
                .map_err(|_| network_error())?;
        }
        if let Some(key) = operation_key {
            headers
                .set("Idempotency-Key", key)
                .map_err(|_| network_error())?;
        }
        options.set_headers(&headers);
        let response = JsFuture::from(window.fetch_with_str_and_init(path, &options))
            .await
            .map_err(|_| network_error())?
            .dyn_into::<web_sys::Response>()
            .map_err(|_| network_error())?;
        let status = response.status();
        let text = JsFuture::from(response.text().map_err(|_| network_error())?)
            .await
            .map_err(|_| network_error())?
            .as_string()
            .ok_or_else(network_error)?;
        if response.ok() || accept_typed_conflict && status == 409 {
            match serde_json::from_str(&text) {
                Ok(value) => return Ok(value),
                Err(_) if response.ok() => return Err(network_error()),
                Err(_) => {}
            }
        }
        let Ok(details) = serde_json::from_str(&text) else {
            return Err(network_error());
        };
        Err(RequestFailure {
            status: Some(status),
            details,
        })
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (path, body, csrf, operation_key, accept_typed_conflict);
        Err(network_error())
    }
}
