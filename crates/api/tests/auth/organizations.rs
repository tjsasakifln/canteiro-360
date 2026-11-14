//! Negativos de organizações na fronteira HTTP, com a mesma identidade real.
use super::*;
use canteiro_contracts::organizations::OrganizationResponse;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

const CONTEXT: &str = "/api/organizations/context";
const COMMAND: &str = "/api/organizations/command";

fn request(method: &str, path: &str, session: Option<&Reply>) -> axum::http::request::Builder {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(session) = session {
        request = request.header(
            header::COOKIE,
            session.cookie.as_ref().unwrap().split(';').next().unwrap(),
        );
    }
    request
}

fn command(session: &Reply) -> axum::http::request::Builder {
    request("POST", COMMAND, Some(session))
        .header(header::ORIGIN, ORIGIN)
        .header("x-csrf-token", session.success().csrf)
        .header(header::CONTENT_TYPE, "application/json")
}

async fn exchange(
    fixture: &Fixture,
    request: axum::http::request::Builder,
    body: impl Into<Body>,
) -> Reply {
    let response = fixture
        .app
        .clone()
        .oneshot(request.body(body.into()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 262_144).await.unwrap();
    Reply {
        status,
        cookie: headers
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_owned()),
        headers,
        body: serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8(bytes.to_vec()).unwrap())),
    }
}

fn private_response(reply: &Reply) {
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(reply.headers[header::PRAGMA], "no-cache");
    assert_eq!(reply.headers[header::REFERRER_POLICY], "no-referrer");
    assert_eq!(reply.headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    let vary = reply
        .headers
        .get_all(header::VARY)
        .iter()
        .flat_map(|value| value.to_str().unwrap().split(','))
        .map(|value| value.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    assert!(vary.iter().any(|value| value == "cookie"));
    assert!(vary.iter().any(|value| value == "origin"));
    assert!(
        reply.cookie.is_none(),
        "organizações não rotacionam identidade"
    );
}

fn denied(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code);
    assert!(
        reply.body["message"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    );
    assert_eq!(reply.body.as_object().unwrap().len(), 2);
    private_response(reply);
    let output = reply.body.to_string().to_lowercase();
    for forbidden in [
        "select ",
        "insert ",
        "sqlx",
        "postgres",
        "canteiro_auth",
        "organization_members",
        "fixture-sensitive-marker",
    ] {
        assert!(!output.contains(forbidden));
    }
}

fn organization_success(reply: &Reply) -> OrganizationResponse {
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    private_response(reply);
    serde_json::from_value(reply.body.clone()).unwrap()
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn organization_http_enforces_origin_cookie_csrf_and_private_error_responses() {
    let fixture = Fixture::new().await;
    let (_, _, logged) = fixture.account().await;
    let cookie = logged.cookie.as_ref().unwrap().split(';').next().unwrap();
    let csrf = logged.success().csrf;
    let unrelated = Uuid::new_v4();
    let endpoints = [
        CONTEXT.to_owned(),
        format!("/api/organizations/{unrelated}/members?context_version=1"),
        format!("/api/organizations/{unrelated}/members.tsv?context_version=1"),
    ];
    for path in &endpoints {
        denied(
            &exchange(&fixture, request("GET", path, None), Body::empty()).await,
            StatusCode::UNAUTHORIZED,
            "session_expired",
        );
        for origin in [
            "null",
            "https://other.example.test",
            "http://canteiro.example.test",
            "https://canteiro.example.test:8443",
        ] {
            let reply = exchange(
                &fixture,
                request("GET", path, Some(&logged)).header(header::ORIGIN, origin),
                Body::empty(),
            )
            .await;
            denied(&reply, StatusCode::FORBIDDEN, "origin_invalid");
        }
        let duplicate = exchange(
            &fixture,
            request("GET", path, Some(&logged))
                .header(header::ORIGIN, ORIGIN)
                .header(header::ORIGIN, ORIGIN),
            Body::empty(),
        )
        .await;
        denied(&duplicate, StatusCode::FORBIDDEN, "origin_invalid");
        for site in ["cross-site", "same-site"] {
            let reply = exchange(
                &fixture,
                request("GET", path, Some(&logged)).header("sec-fetch-site", site),
                Body::empty(),
            )
            .await;
            denied(&reply, StatusCode::FORBIDDEN, "origin_invalid");
        }
    }
    let body = r#"{"action":"create","name":"Empresa fictícia"}"#;
    for origin in [None, Some("null"), Some("https://other.example.test")] {
        let mut builder = request("POST", COMMAND, Some(&logged))
            .header("x-csrf-token", &csrf)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        denied(
            &exchange(&fixture, builder, body).await,
            StatusCode::FORBIDDEN,
            "origin_invalid",
        );
    }
    for value in [None, Some("invalid-csrf")] {
        let mut builder = request("POST", COMMAND, Some(&logged))
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(value) = value {
            builder = builder.header("x-csrf-token", value);
        }
        denied(
            &exchange(&fixture, builder, body).await,
            StatusCode::FORBIDDEN,
            "csrf_invalid",
        );
    }
    for builder in [
        command(&logged).header(header::ORIGIN, ORIGIN),
        command(&logged).header("sec-fetch-site", "same-site"),
        command(&logged).header("sec-fetch-site", "cross-site"),
    ] {
        denied(
            &exchange(&fixture, builder, body).await,
            StatusCode::FORBIDDEN,
            "origin_invalid",
        );
    }
    denied(
        &exchange(
            &fixture,
            command(&logged).header("x-csrf-token", &csrf),
            body,
        )
        .await,
        StatusCode::FORBIDDEN,
        "csrf_invalid",
    );
    for invalid_cookie in [
        "__Host-canteiro-session=short".to_owned(),
        format!("{cookie}; {cookie}"),
        format!("__Host-canteiro-session={}", "a".repeat(43)),
    ] {
        denied(
            &exchange(
                &fixture,
                request("GET", CONTEXT, None).header(header::COOKIE, invalid_cookie),
                Body::empty(),
            )
            .await,
            StatusCode::UNAUTHORIZED,
            "session_expired",
        );
    }
    denied(
        &exchange(
            &fixture,
            request("GET", CONTEXT, Some(&logged)).header(header::COOKIE, cookie),
            Body::empty(),
        )
        .await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    denied(
        &exchange(
            &fixture,
            request("POST", COMMAND, None)
                .header(header::ORIGIN, ORIGIN)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json"),
            body,
        )
        .await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    // Rejeições de transporte não criam empresa nem comprometem a sessão.
    let unchanged = exchange(
        &fixture,
        request("GET", CONTEXT, Some(&logged)),
        Body::empty(),
    )
    .await;
    assert!(
        organization_success(&unchanged)
            .state
            .organizations
            .is_empty()
    );
    fixture.database.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn organization_http_rejects_anonymous_invalid_inputs_and_revoked_identity() {
    let fixture = Fixture::new().await;
    let anonymous = fixture.request(None, None, None, None).await;
    denied(
        &exchange(
            &fixture,
            request("GET", CONTEXT, Some(&anonymous)),
            Body::empty(),
        )
        .await,
        StatusCode::FORBIDDEN,
        "forbidden",
    );
    let (_, password, logged) = fixture.account().await;
    let context = exchange(
        &fixture,
        request("GET", CONTEXT, Some(&logged)),
        Body::empty(),
    )
    .await;
    let current = organization_success(&context);
    assert!(current.state.organizations.is_empty());
    assert!(current.state.active.is_none());
    assert!(!current.state.mfa_verified);
    assert!(!context.body.to_string().contains(&password));
    assert!(!context.body.to_string().contains(&logged.bearer()));
    denied(
        &exchange(
            &fixture,
            command(&logged),
            r#"{"action":"create","name":"Empresa fictícia"}"#,
        )
        .await,
        StatusCode::FORBIDDEN,
        "mfa_required",
    );
    for body in [
        "{\"action\":".to_owned(),
        r#"{"action":"create","name":17}"#.to_owned(),
        r#"{"action":"create","name":"Empresa fictícia","tenant_id":"fixture-sensitive-marker"}"#
            .to_owned(),
        r#"{"action":"fixture-sensitive-marker"}"#.to_owned(),
        format!(
            "{{\"action\":\"create\",\"name\":\"{}\"}}",
            "x".repeat(9_000)
        ),
    ] {
        denied(
            &exchange(&fixture, command(&logged), body).await,
            StatusCode::BAD_REQUEST,
            "invalid_input",
        );
    }
    let unrelated = Uuid::new_v4();
    for suffix in ["members", "members.tsv"] {
        for path in [
            format!("/api/organizations/fixture-sensitive-marker/{suffix}?context_version=1"),
            format!("/api/organizations/%FF/{suffix}?context_version=1"),
            format!("/api/organizations/{unrelated}/{suffix}"),
            format!("/api/organizations/{unrelated}/{suffix}?context_version=bad"),
            format!("/api/organizations/{unrelated}/{suffix}?context_version=1&context_version=2"),
            format!(
                "/api/organizations/{unrelated}/{suffix}?context_version=1&extra=fixture-sensitive-marker"
            ),
        ] {
            denied(
                &exchange(
                    &fixture,
                    request("GET", &path, Some(&logged)),
                    Body::empty(),
                )
                .await,
                StatusCode::BAD_REQUEST,
                "invalid_input",
            );
        }
        let unavailable = exchange(
            &fixture,
            request(
                "GET",
                &format!("/api/organizations/{unrelated}/{suffix}?context_version=1"),
                Some(&logged),
            ),
            Body::empty(),
        )
        .await;
        denied(&unavailable, StatusCode::CONFLICT, "context_changed");
    }
    for query in [
        "offset=-1",
        "limit=101",
        "query=%0Afixture-sensitive-marker",
    ] {
        let path = format!("/api/organizations/{unrelated}/members?context_version=1&{query}");
        denied(
            &exchange(
                &fixture,
                request("GET", &path, Some(&logged)),
                Body::empty(),
            )
            .await,
            StatusCode::BAD_REQUEST,
            "invalid_input",
        );
    }
    let cleared = exchange(
        &fixture,
        command(&logged),
        json!({"action":"clear_selection","expected_context_version":current.state.context_version}).to_string(),
    ).await;
    let cleared = organization_success(&cleared);
    assert!(cleared.state.active.is_none());
    assert_eq!(
        cleared.state.context_version,
        current.state.context_version + 1
    );
    let logout = fixture.run(&logged, AuthCommand::Logout).await;
    assert_eq!(logout.success().context.state, AuthState::Anonymous);
    for path in [
        CONTEXT.to_owned(),
        format!("/api/organizations/{unrelated}/members?context_version=1"),
        format!("/api/organizations/{unrelated}/members.tsv?context_version=1"),
    ] {
        denied(
            &exchange(
                &fixture,
                request("GET", &path, Some(&logged)),
                Body::empty(),
            )
            .await,
            StatusCode::UNAUTHORIZED,
            "session_expired",
        );
    }
    denied(
        &exchange(
            &fixture,
            command(&logged),
            r#"{"action":"clear_selection","expected_context_version":1}"#,
        )
        .await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    fixture.database.close().await;
}

fn authenticator_code(secret: &str) -> String {
    // Cálculo de referência RFC 6238 do cliente de teste, sem acesso ao segredo cifrado.
    let bytes = data_encoding::BASE32_NOPAD
        .decode(secret.as_bytes())
        .unwrap();
    let counter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&bytes).unwrap();
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 15);
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into().unwrap()) & 0x7fff_ffff;
    format!("{:06}", value % 1_000_000)
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn organization_http_requires_confirmed_mfa_and_serves_only_current_company_export() {
    let fixture = Fixture::new().await;
    let (email, password, logged) = fixture.account().await;
    let setup = fixture
        .run(
            &logged,
            AuthCommand::StartMfa {
                password: password.clone(),
            },
        )
        .await;
    let secret = setup.success().mfa_setup.unwrap();
    let verified = fixture
        .run(
            &setup,
            AuthCommand::ConfirmMfa {
                code: authenticator_code(&secret),
            },
        )
        .await;
    assert_eq!(verified.success().context.state, AuthState::Authenticated);
    let created = exchange(
        &fixture,
        command(&verified),
        r#"{"action":"create","name":"Empresa HTTP fictícia"}"#,
    )
    .await;
    let created = organization_success(&created);
    assert_eq!(created.state.organizations.len(), 1);
    assert!(
        created.state.active.is_none(),
        "a escolha da empresa é explícita"
    );
    let tenant = &created.state.organizations[0].id;
    let selected = exchange(&fixture, command(&verified), json!({"action":"select","organization_id":tenant,"expected_context_version":created.state.context_version}).to_string()).await;
    let selected = organization_success(&selected);
    assert_eq!(
        selected.state.active.as_ref().unwrap().organization.id,
        *tenant
    );
    // O contexto válido da empresa selecionada não autoriza outro ID no caminho.
    let unrelated = Uuid::new_v4();
    for suffix in ["members", "members.tsv"] {
        let crossed = exchange(
            &fixture,
            request(
                "GET",
                &format!(
                    "/api/organizations/{unrelated}/{suffix}?context_version={}",
                    selected.state.context_version
                ),
                Some(&verified),
            ),
            Body::empty(),
        )
        .await;
        denied(&crossed, StatusCode::CONFLICT, "context_changed");
        let output = crossed.body.to_string();
        assert!(!output.contains("Empresa HTTP fictícia"));
        assert!(!output.contains(&email));
        assert!(!output.contains(tenant));
    }
    let search = exchange(
        &fixture,
        request(
            "GET",
            &format!(
                "/api/organizations/{tenant}/members?context_version={}",
                selected.state.context_version
            ),
            Some(&verified),
        ),
        Body::empty(),
    )
    .await;
    assert_eq!(search.status, StatusCode::OK);
    assert_eq!(search.body["total"], 1);
    assert_eq!(search.body["members"][0]["email"], email);
    private_response(&search);
    let exported = exchange(
        &fixture,
        request(
            "GET",
            &format!(
                "/api/organizations/{tenant}/members.tsv?context_version={}",
                selected.state.context_version
            ),
            Some(&verified),
        ),
        Body::empty(),
    )
    .await;
    assert_eq!(exported.status, StatusCode::OK);
    private_response(&exported);
    assert_eq!(
        exported.headers[header::CONTENT_TYPE],
        "text/tab-separated-values; charset=utf-8"
    );
    assert_eq!(
        exported.headers[header::CONTENT_DISPOSITION],
        "attachment; filename=\"membros.tsv\""
    );
    let output = exported.body.as_str().unwrap();
    for expected in [
        "Empresa HTTP fictícia",
        tenant,
        "Revisão\t",
        "Filtro\t",
        "Total de membros\t1",
        "Emissão UTC\t",
        &email,
    ] {
        assert!(output.contains(expected));
    }
    assert!(!output.contains(&secret));
    assert!(!output.contains(&password));
    let old_context = selected.state.context_version;
    let cleared = exchange(
        &fixture,
        command(&verified),
        json!({"action":"clear_selection","expected_context_version":old_context}).to_string(),
    )
    .await;
    let cleared = organization_success(&cleared);
    assert!(cleared.state.active.is_none());
    let stale = exchange(
        &fixture,
        request(
            "GET",
            &format!("/api/organizations/{tenant}/members.tsv?context_version={old_context}"),
            Some(&verified),
        ),
        Body::empty(),
    )
    .await;
    denied(&stale, StatusCode::CONFLICT, "context_changed");
    let logout = fixture.run(&verified, AuthCommand::Logout).await;
    let pending = fixture
        .run(&logout, AuthCommand::Login { email, password })
        .await;
    assert_eq!(pending.success().context.state, AuthState::MfaPending);
    for path in [
        CONTEXT.to_owned(),
        format!("/api/organizations/{tenant}/members?context_version=1"),
        format!("/api/organizations/{tenant}/members.tsv?context_version=1"),
    ] {
        denied(
            &exchange(
                &fixture,
                request("GET", &path, Some(&pending)),
                Body::empty(),
            )
            .await,
            StatusCode::FORBIDDEN,
            "forbidden",
        );
    }
    denied(
        &exchange(
            &fixture,
            command(&pending),
            r#"{"action":"create","name":"Empresa sem confirmação"}"#,
        )
        .await,
        StatusCode::FORBIDDEN,
        "forbidden",
    );
    fixture.database.close().await;
}
