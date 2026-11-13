use canteiro_contracts::forms::{FormPreview, MeasurementForm, MeasurementPage, MeasurementQuery};

pub fn query_string(query: &MeasurementQuery) -> String {
    use super::super::model::encode_query;
    format!(
        "page={}&page_size={}&search={}&sort={}&direction={}",
        query.page,
        query.page_size,
        encode_query(&query.search),
        encode_query(&query.sort),
        encode_query(&query.direction)
    )
}

pub async fn load_page(
    organization: String,
    query: MeasurementQuery,
) -> Result<MeasurementPage, String> {
    #[cfg(feature = "ssr")]
    {
        canteiro_contracts::forms::demo_page(&organization, &query)
            .map_err(|error| error.to_string())
    }
    #[cfg(all(feature = "hydrate", not(feature = "ssr")))]
    {
        // Browser futures stay on the browser thread; Resource requires Send.
        send_wrapper::SendWrapper::new(async move {
            let path = format!(
                "/api/prototipo/{organization}/medicoes?{}",
                query_string(&query)
            );
            let (status, text) = request(&path, None).await?;
            if status == 422 {
                return Err(
                    serde_json::from_str::<canteiro_contracts::forms::FieldError>(&text)
                        .map(|error| error.message)
                        .unwrap_or_else(|_| {
                            "Revise a busca, a página e a ordenação antes de consultar novamente."
                                .into()
                        }),
                );
            }
            serde_json::from_str(&text).map_err(|_| {
                "A consulta retornou dados que não puderam ser lidos. Tente novamente.".into()
            })
        })
        .await
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        let _ = (organization, query);
        Err("Consulta disponível no servidor e no navegador.".into())
    }
}

pub async fn validate_form(
    organization: &str,
    input: &MeasurementForm,
) -> Result<FormPreview, String> {
    #[cfg(feature = "hydrate")]
    {
        let path = format!("/api/prototipo/{organization}/medicoes/preview");
        let payload = serde_json::to_string(input).map_err(|_| {
            "Não foi possível preparar a conferência. Os campos foram preservados.".to_string()
        })?;
        let (_, text) = request(&path, Some(payload)).await?;
        serde_json::from_str(&text)
            .map_err(|_| "Não foi possível ler a conferência. Os campos foram preservados.".into())
    }
    #[cfg(not(feature = "hydrate"))]
    {
        let _ = (organization, input);
        Err("Ative a interação para conferir o rascunho.".into())
    }
}

#[cfg(feature = "hydrate")]
async fn request(path: &str, payload: Option<String>) -> Result<(u16, String), String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    const FAILURE: &str = "Não foi possível consultar o servidor. Confira a conexão e tente novamente. Os dados digitados foram preservados; nenhuma gravação foi feita.";
    let window = web_sys::window().ok_or_else(|| FAILURE.to_string())?;
    let options = web_sys::RequestInit::new();
    options.set_method(if payload.is_some() { "POST" } else { "GET" });
    let signal = web_sys::AbortSignal::timeout_with_u32(15_000);
    options.set_signal(Some(&signal));
    if let Some(body) = payload {
        options.set_body(&wasm_bindgen::JsValue::from_str(&body));
        let headers = web_sys::Headers::new().map_err(|_| FAILURE.to_string())?;
        headers
            .set("Content-Type", "application/json")
            .map_err(|_| FAILURE.to_string())?;
        options.set_headers(&headers);
    }
    let response = JsFuture::from(window.fetch_with_str_and_init(path, &options))
        .await
        .map_err(|_| FAILURE.to_string())?
        .dyn_into::<web_sys::Response>()
        .map_err(|_| FAILURE.to_string())?;
    let status = response.status();
    if !response.ok() && status != 422 {
        return Err(FAILURE.into());
    }
    let text = JsFuture::from(response.text().map_err(|_| FAILURE.to_string())?)
        .await
        .map_err(|_| FAILURE.to_string())?
        .as_string()
        .ok_or_else(|| FAILURE.to_string())?;
    Ok((status, text))
}

pub fn saved_time() -> String {
    #[cfg(feature = "hydrate")]
    {
        js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    String::new()
}
