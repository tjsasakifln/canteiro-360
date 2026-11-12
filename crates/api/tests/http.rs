use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use canteiro_api::{config::Config, router};
use tower::ServiceExt;

#[tokio::test]
async fn initial_document_has_useful_content_before_wasm() {
    let config = Config::from_lookup(|_| None).unwrap();
    let response = router(&config)
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/html")
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let html = String::from_utf8(
        to_bytes(response.into_body(), 1_048_576)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(html.contains("Cada etapa da obra, com clareza."));
    assert!(html.contains("lang=\"pt-BR\""));
    assert!(html.contains("id=\"foundation-details-toggle\""));
    assert!(html.contains("disabled"));
    assert!(html.contains("canteiro-web_bg.wasm"));
    assert!(html.contains("Recarregar a página"));
    assert!(!html.contains("postgres://"));
    assert!(!html.contains("CANTEIRO_DATABASE_URL"));
}

#[tokio::test]
async fn unknown_routes_and_private_files_never_receive_a_success_page() {
    let config = Config::from_lookup(|_| None).unwrap();
    for path in [
        "/nao-existe",
        "/.env",
        "/Cargo.toml",
        "/config/homologation.toml",
        "/pkg/../../../.env",
        "/pkg/missing.wasm",
    ] {
        let response = router(&config)
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let response = router(&config)
        .oneshot(Request::post("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn prototype_routes_render_only_the_requested_company_and_reject_unknown_contexts() {
    let config = Config::from_lookup(|_| None).unwrap();
    for (company, own_contract, other_contract) in [
        ("aurora", "CT-014/2026", "CT-027/2026"),
        ("horizonte", "CT-027/2026", "CT-014/2026"),
    ] {
        for task in ["painel", "orcamento", "medicao", "aprovacoes", "documentos"] {
            let path = format!("/prototipo/{company}/{task}?busca=alvenaria");
            let response = router(&config)
                .oneshot(Request::get(&path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let html = String::from_utf8(
                to_bytes(response.into_body(), 1_048_576)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();
            assert!(html.contains(own_contract), "{path}");
            assert!(!html.contains(other_contract), "{path}");
            assert!(html.contains("Dados fictícios"), "{path}");
            assert!(html.contains("id=\"prototype-shell\""), "{path}");
        }
    }
    for path in [
        "/prototipo/desconhecida/painel",
        "/prototipo/aurora/tarefa-inexistente",
        "/prototipo/horizonte/tarefa-inexistente?busca=concreto",
    ] {
        let response = router(&config)
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let html = String::from_utf8(
            to_bytes(response.into_body(), 1_048_576)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(
            html.contains("Este endereço não está disponível."),
            "{path}"
        );
        assert!(
            !html.contains("CT-014/2026") && !html.contains("CT-027/2026"),
            "{path}"
        );
        assert!(!html.contains("id=\"prototype-shell\""), "{path}");
    }
    // A URL check must not turn method errors into success or mask their status.
    for path in ["/prototipo/aurora/painel", "/prototipo/invalida/painel"] {
        let response = router(&config)
            .oneshot(Request::post(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED, "{path}");
    }
    let response = router(&config)
        .oneshot(
            Request::head("/prototipo/invalida/painel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(
        to_bytes(response.into_body(), 1024)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn liveness_does_not_claim_business_or_database_readiness() {
    let config = Config::from_lookup(|_| None).unwrap();
    let response = router(&config)
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json, serde_json::json!({"status":"running"}));
}

#[tokio::test]
async fn html_is_actually_transferred_with_brotli_when_requested() {
    use std::io::Read;
    let config = Config::from_lookup(|_| None).unwrap();
    let request = Request::get("/")
        .header("accept-encoding", "br")
        .body(Body::empty())
        .unwrap();
    let response = router(&config).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "br");
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    let mut html = String::new();
    brotli::Decompressor::new(bytes.as_ref(), 4096)
        .read_to_string(&mut html)
        .unwrap();
    assert!(html.contains("Cada etapa da obra, com clareza."));
    assert!(html.contains("canteiro-web_bg.wasm"));
}

#[tokio::test]
async fn static_asset_serves_the_measured_brotli_without_recompression() {
    use std::io::Write;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let site = std::env::temp_dir().join(format!("canteiro-assets-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(site.join("pkg")).unwrap();
    let css = b"body { color: #172033; background: #f7f9fc; }";
    let mut compressed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
        encoder.write_all(css).unwrap();
    }
    std::fs::write(site.join("pkg/test.css"), css).unwrap();
    std::fs::write(site.join("pkg/test.css.br"), &compressed).unwrap();
    let mut config = Config::from_lookup(|_| None).unwrap();
    config.site_root = site.clone();
    let request = Request::get("/pkg/test.css")
        .header("accept-encoding", "br")
        .body(Body::empty())
        .unwrap();
    let response = router(&config).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "br");
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/css")
    );
    let actual = to_bytes(response.into_body(), 4096).await.unwrap();
    assert_eq!(actual.as_ref(), compressed);
    std::fs::remove_dir_all(site).unwrap();
}
