//! Servidor HTTP e fronteira de configuração da fundação.
mod access;
pub mod auth;
mod calculations;
pub mod config;
mod documents;
pub mod error;
mod estimating;
mod forms;
mod notifications;
mod obligations;
mod organizations;
mod parties;
mod portfolio;
mod projects;
mod reliability;

use axum::{
    Json, Router,
    body::Body,
    http::{HeaderValue, Method, Request, StatusCode, header},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use canteiro_web::{App, is_supported_prototype_route, shell};
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
        .merge(auth::routes())
        .merge(organizations::routes())
        .merge(access::routes())
        .merge(reliability::routes())
        .merge(documents::routes())
        .merge(parties::routes())
        .merge(portfolio::routes())
        .merge(projects::routes())
        .merge(obligations::routes())
        .merge(notifications::routes())
        .merge(estimating::routes())
        .route(
            "/api/prototipo/calculos/rateio",
            axum::routing::post(calculations::allocation)
                .layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .route(
            "/api/prototipo/calculos/calendario",
            axum::routing::post(calculations::calendar)
                .layer(axum::extract::DefaultBodyLimit::max(4096)),
        )
        .route("/api/prototipo/{organization}/medicoes", get(forms::page))
        .route(
            "/api/prototipo/{organization}/medicoes/export",
            get(forms::export),
        )
        .route(
            "/api/prototipo/{organization}/medicoes/preview",
            axum::routing::post(forms::preview).layer(axum::extract::DefaultBodyLimit::max(16_384)),
        )
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
        .layer(middleware::from_fn(prototype_route_status))
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
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(CompressionLayer::new())
        .with_state(options)
}

/// Dynamic Leptos routes also match unknown parameter values. Preserve the
/// rendered not-found page while giving GET/HEAD the corresponding HTTP status.
/// This validates demonstration URLs only, never a user's business permissions.
async fn prototype_route_status(request: Request<Body>, next: Next) -> Response {
    let invalid_prototype = matches!(*request.method(), Method::GET | Method::HEAD)
        && request.uri().path().starts_with("/prototipo/")
        && !is_supported_prototype_route(request.uri().path());
    let mut response = next.run(request).await;
    if invalid_prototype && response.status() == StatusCode::OK {
        *response.status_mut() = StatusCode::NOT_FOUND;
    }
    response
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
