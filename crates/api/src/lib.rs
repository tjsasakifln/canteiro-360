//! Servidor HTTP e fronteira de configuração da fundação.
pub mod config;
pub mod error;

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderValue, Request, StatusCode, header},
    routing::get,
};
use canteiro_web::{App, shell};
use config::Config;
use leptos_axum::{LeptosRoutes, generate_route_list, render_app_to_stream};
use tower::ServiceExt;
use tower_http::{
    compression::CompressionLayer, services::ServeDir, set_header::SetResponseHeaderLayer,
};

pub fn router(config: &Config) -> Router {
    let options = config.leptos_options();
    let routes = generate_route_list(App);
    let shell_options = options.clone();
    let fallback_options = options.clone();
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(canteiro_application::liveness()) }),
        )
        .nest_service(
            "/pkg",
            ServeDir::new(config.site_root.join("pkg")).precompressed_br(),
        )
        .leptos_routes(&options, routes, move || shell(shell_options.clone()))
        .fallback(move |request: Request<Body>| {
            let options = fallback_options.clone();
            let handler = render_app_to_stream(move || shell(options.clone()));
            async move {
                let mut response = handler(request).await;
                *response.status_mut() = StatusCode::NOT_FOUND;
                response
            }
        })
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(CompressionLayer::new())
        .with_state(options)
}

/// Usa o mesmo serviço da aplicação para gerar a evidência e medir o bootstrap.
pub async fn render_shell(
    config: &Config,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let response = router(config)
        .oneshot(Request::builder().uri("/").body(Body::empty())?)
        .await?;
    if response.status() != StatusCode::OK {
        return Err("A rota inicial não retornou HTTP 200".into());
    }
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576).await?;
    Ok(String::from_utf8(bytes.to_vec())?)
}
