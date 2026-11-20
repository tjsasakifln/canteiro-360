use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use canteiro_api::{config::Config, router};
use serde_json::{Value, json};
use std::sync::OnceLock;
use tower::ServiceExt;

fn app() -> axum::Router {
    static APP: OnceLock<axum::Router> = OnceLock::new();
    APP.get_or_init(|| {
        // Leptos suspende recursos por um indicador global durante a geração
        // das rotas. Gerar novamente enquanto outra requisição SSR carrega
        // um recurso pode estacioná-lo indefinidamente. Como no servidor,
        // construímos o roteador uma vez antes de atender, sem serializar
        // as requisições nem os testes concorrentes.
        let config = Config::from_lookup(|_| None).unwrap();
        router(&config)
    })
    .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn worksheet_html_contains_only_its_page_before_hydration() {
    for (organization, prefix) in [("aurora", "AUR"), ("horizonte", "HOR")] {
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            app().oneshot(
                Request::get(format!(
                    "/prototipo/{organization}/planilha?page=2&page_size=25&sort=code&direction=asc"
                ))
                .body(Body::empty())
                .unwrap(),
            ),
        )
        .await
        .expect("o SSR paginado deve resolver os recursos antes da resposta")
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(response.into_body(), 1_048_576)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        // Out-of-order SSR can contain all row markup inside an inert template
        // while the live document only has a loading fallback. Counting the raw
        // response alone would incorrectly pass with JavaScript disabled.
        assert!(
            !html.contains("<template"),
            "The server must render the worksheet into the document, not an inert template"
        );
        assert!(
            !html.contains("id=\"table-loading\""),
            "The initial worksheet must not require a script to replace its loading fallback"
        );
        let table_container = html
            .split("id=\"table-content\"")
            .nth(1)
            .expect("The rendered worksheet has its table container")
            .split('>')
            .next()
            .unwrap();
        assert!(
            table_container.contains("aria-busy=\"false\""),
            "The resolved server-rendered worksheet must not remain announced as loading"
        );
        assert_eq!(html.matches("data-row-id=").count(), 25);
        assert!(html.contains(&format!("{prefix}-00026")));
        assert!(html.contains(&format!("{prefix}-00050")));
        assert!(!html.contains(&format!("{prefix}-00051")));
        assert!(html.contains("id=\"table-next\""));
        assert!(html.contains("page=3"));
        assert!(
            html.len() < 150_000,
            "SSR must contain a page, never all 10000 rows"
        );
    }

    // O executor SSR é compartilhado pelo binário de testes. As requisições
    // concorrentes usam o mesmo runtime das verificações de paginação acima.
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        concurrent_worksheet_ssr_preserves_title(),
    )
    .await
    .expect("a renderização concorrente deve terminar com todos os títulos");
}

async fn concurrent_worksheet_ssr_preserves_title() {
    let app = app();
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..16 {
        for (company, expected) in [
            ("aurora", "Escola Vila Nova"),
            ("horizonte", "Galpão Horizonte"),
        ] {
            let app = app.clone();
            requests.spawn(async move {
                let response = app
                    .oneshot(
                        Request::get(format!("/prototipo/{company}/planilha"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                assert_eq!(response.status(), StatusCode::OK);
                let body = to_bytes(response.into_body(), 1_048_576).await.unwrap();
                let body = std::str::from_utf8(&body).unwrap();
                assert!(body.contains(&format!(
                    "<title>Planilha de medição | {expected} | Canteiro 360</title>"
                )));
                assert_eq!(body.matches("data-row-id=").count(), 25);
            });
        }
    }
    while let Some(result) = requests.join_next().await {
        result.unwrap();
    }
}

async fn get(path: &str) -> (StatusCode, Value) {
    let response = app()
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn post(value: Value) -> (StatusCode, Value) {
    let response = app()
        .oneshot(
            Request::post("/api/prototipo/aurora/medicoes/preview")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&value).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn server_pages_and_filters_a_bounded_fictitious_dataset() {
    let (status, page) = get("/api/prototipo/aurora/medicoes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 10000);
    assert_eq!(page["total_pages"], 400);
    assert_eq!(page["rows"].as_array().unwrap().len(), 25);
    assert_eq!(page["rows"][0]["previous"], "20");
    assert_eq!(page["rows"][0]["current"], "35");
    assert_eq!(page["rows"][0]["difference"], "15");
    assert_eq!(page["rows"][0]["period_value"], "150.00");
    assert_eq!(page["rows"][0]["unit"], "m²");
    let (status, second) =
        get("/api/prototipo/aurora/medicoes?page=2&page_size=50&sort=code&direction=desc").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(second["rows"][0]["id"], 9950);
    assert_eq!(second["rows"].as_array().unwrap().len(), 50);
    let (status, filtered) = get("/api/prototipo/aurora/medicoes?search=Alvenaria").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["total"], 1667);
    assert!(
        filtered["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["description"].as_str().unwrap().contains("Alvenaria"))
    );
    let (status, other) = get("/api/prototipo/horizonte/medicoes?search=AUR-00001").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other["total"], 0);
    let (status, other) = get("/api/prototipo/horizonte/medicoes?sort=unit_price").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other["rows"][0]["code"], "HOR-00002");
}

#[tokio::test]
async fn invalid_queries_and_contexts_are_rejected_without_clamping_or_details() {
    for query in [
        "page=0",
        "page=401",
        "page=99999999999999999999999",
        "page_size=10000",
        "sort=unknown",
        "direction=DESC",
        "page=1&page=2",
        "unexpected=1",
        "search=%0A",
    ] {
        let (status, body) = get(&format!("/api/prototipo/aurora/medicoes?{query}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{query}");
        assert!(body["message"].is_string());
        assert!(body.get("source").is_none());
        assert!(body.get("rows").is_none());
    }
    let (status, error) = get("/api/prototipo/unknown/medicoes").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error["code"], "not_found");
    let long = format!("/api/prototipo/aurora/medicoes?search={}", "x".repeat(81));
    assert_eq!(get(&long).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn preview_validates_every_field_in_the_server_and_never_writes_the_fixture() {
    let input = json!({"row_id":1,"current":"35","unit_price":"1.234,56","percent":"5,5","date":"12/11/2026","unit":"m²","notes":"Observação preservada"});
    let (status, preview) = post(input.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(preview["canonical"]["unit_price"], "1234.56");
    assert_eq!(preview["canonical"]["date"], "2026-11-12");
    assert_eq!(preview["preview"]["period_value"], "18518.40");
    let second = post(input.clone()).await;
    assert_eq!(second.0, StatusCode::OK);
    assert_eq!(second.1, preview);
    let (_, page) = get("/api/prototipo/aurora/medicoes").await;
    assert_eq!(page["rows"][0]["unit_price"], "10");
    let mut invalid = input.clone();
    invalid["current"] = json!("1.234");
    let (status, rejected) = post(invalid).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(rejected["errors"][0]["field"], "current");
    assert_eq!(rejected["errors"][0]["code"], "ambiguous");
    assert_eq!(rejected["canonical"]["notes"], "Observação preservada");
    assert_eq!(rejected["canonical"]["unit_price"], "1234.56");
    assert!(rejected["preview"].is_null());
    for (field, value) in [
        ("current", "101"),
        ("current", "19"),
        ("unit", "m³"),
        ("percent", "101"),
        ("date", "31/02/2026"),
    ] {
        let mut invalid = input.clone();
        invalid[field] = json!(value);
        let (status, body) = post(invalid).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            body["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| error["field"] == field)
        );
    }
    let mut malformed = input;
    malformed["unit_price"] = json!(1234.56);
    assert_eq!(post(malformed).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn export_is_a_download_of_all_filtered_results_not_only_the_page() {
    let response = app()
        .oneshot(
            Request::get("/api/prototipo/horizonte/medicoes/export?search=Alvenaria&page=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["content-type"],
        "text/tab-separated-values; charset=utf-8"
    );
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .starts_with("attachment;")
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let text = String::from_utf8(
        to_bytes(response.into_body(), 2_097_152)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(text.lines().count(), 1668);
    assert!(text.contains("HOR-00001"));
    assert!(text.contains("HOR-09997"));
    assert!(!text.contains("AUR-"));
}
