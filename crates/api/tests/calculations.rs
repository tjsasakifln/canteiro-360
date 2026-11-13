use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use canteiro_api::{config::Config, router};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn post(path: &str, input: Value) -> (StatusCode, Value) {
    let config = Config::from_lookup(|_| None).unwrap();
    let response = router(&config)
        .oneshot(
            Request::post(path)
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&input).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let data = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, serde_json::from_slice(&data).unwrap())
}

#[tokio::test]
async fn server_preserves_money_and_calendar_and_returns_the_policy() {
    let allocation = json!({"policy":"C360-DECIMAL-1", "total":"100.00", "shares":3});
    let (status, result) = post("/api/prototipo/calculos/rateio", allocation.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["policy"], "C360-DECIMAL-1");
    assert_eq!(result["shares"][0]["amount"], "33.34");
    assert_eq!(result["shares"][1]["amount"], "33.33");
    assert_eq!(result["shares"][2]["amount"], "33.33");
    assert_eq!(result["shares"][0]["received_cent"], true);
    assert_eq!(
        result,
        post("/api/prototipo/calculos/rateio", allocation).await.1
    );
    let (status, result) = post("/api/prototipo/calculos/calendario", json!({"policy":"C360-DECIMAL-1", "calendar_policy":"CALENDARIO_CIVIL", "cadence":"MENSAL", "anchor":"2027-01-31", "formal_end":"2027-03-31"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["periods"][0]["end"], "2027-02-28");
    assert_eq!(result["periods"][1]["end"], "2027-03-31");
    assert_eq!(result["duration_days"], 59);
}

#[tokio::test]
async fn server_rejects_float_unknown_policy_scale_overflow_and_invalid_dates() {
    for input in [
        json!({"policy":"future", "total":"100.00", "shares":3}),
        json!({"policy":"C360-DECIMAL-1", "total":100.01, "shares":3}),
        json!({"policy":"C360-DECIMAL-1", "total":"100.001", "shares":3}),
        json!({"policy":"C360-DECIMAL-1", "total":"1000000000000000000.00", "shares":3}),
        json!({"policy":"C360-DECIMAL-1", "total":"-1.00", "shares":3}),
        json!({"policy":"C360-DECIMAL-1", "total":"1.00", "shares":0}),
        json!({"policy":"C360-DECIMAL-1", "total":"1.00", "shares":101}),
        json!({"policy":"C360-DECIMAL-1", "total":"1.00", "shares":3,"rounding":"up"}),
    ] {
        let (status, result) = post("/api/prototipo/calculos/rateio", input).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(result["message"].is_string());
        assert!(result.get("shares").is_none());
    }
    for (anchor, end) in [
        ("2027-02-31", "2027-03-31"),
        ("2027-03-31", "2027-01-31"),
        ("2027-01-01", "9999-12-31"),
    ] {
        let (status, _) = post("/api/prototipo/calculos/calendario", json!({"policy":"C360-DECIMAL-1", "calendar_policy":"CALENDARIO_CIVIL", "cadence":"MENSAL", "anchor":anchor, "formal_end":end})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
}

#[tokio::test]
async fn calculation_page_renders_examples_and_labels_without_scripts() {
    let config = Config::from_lookup(|_| None).unwrap();
    let response = router(&config)
        .oneshot(Request::get("/conferencia").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let html = String::from_utf8(
        to_bytes(response.into_body(), 1_048_576)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    for text in [
        "33,34",
        "28/02/2027",
        "Regra de cálculo versão 1",
        "allocation-total",
        "calendar-start",
        "maiores restos",
        "C360-DECIMAL-1",
    ] {
        assert!(html.contains(text), "{text}");
    }
    assert!(!html.contains("<template"));
}
