use canteiro_contracts::auth::{AuthCommand, AuthFailure, AuthResponse};

pub async fn context() -> Result<AuthResponse, AuthFailure> {
    request(None, None).await
}

pub async fn command(input: &AuthCommand, csrf: &str) -> Result<AuthResponse, AuthFailure> {
    let body = serde_json::to_string(input).map_err(|_| unavailable())?;
    request(Some(body), Some(csrf)).await
}

fn unavailable() -> AuthFailure {
    AuthFailure {
        code: "network_unavailable".into(),
        message: "Não foi possível confirmar a resposta. Confira a conexão e tente novamente. Seus campos continuam nesta aba.".into(),
    }
}

async fn request(body: Option<String>, csrf: Option<&str>) -> Result<AuthResponse, AuthFailure> {
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
        if let Some(body) = body.as_ref() {
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
        let path = if body.is_some() {
            "/api/auth/command"
        } else {
            "/api/auth/context"
        };
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
        let _ = (body, csrf);
        Err(unavailable())
    }
}
