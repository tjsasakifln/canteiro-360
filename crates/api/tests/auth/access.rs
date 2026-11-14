//! A mesma API do navegador, sem aceitar identidade, papel ou empresa implícitos.
use super::*;
use canteiro_contracts::{access::AccessResponse, organizations::OrganizationCommand};

async fn access_request(
    f: &Fixture,
    session: Option<&Reply>,
    body: Option<Value>,
    origin: Option<&str>,
    csrf: Option<&str>,
) -> Reply {
    let mut request = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(if body.is_some() {
            "/api/access/command"
        } else {
            "/api/access/context"
        });
    if let Some(s) = session {
        request = request.header(
            header::COOKIE,
            s.cookie.as_ref().unwrap().split(';').next().unwrap(),
        );
    }
    if let Some(o) = origin {
        request = request.header(header::ORIGIN, o);
    }
    if let Some(c) = csrf {
        request = request.header("x-csrf-token", c);
    }
    let payload = if let Some(body) = body {
        request = request.header(header::CONTENT_TYPE, "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = f
        .app
        .clone()
        .oneshot(request.body(payload).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = to_bytes(response.into_body(), 524_288).await.unwrap();
    Reply {
        status,
        cookie: None,
        headers,
        body: serde_json::from_slice(&body).unwrap(),
    }
}
fn private(reply: &Reply) {
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(reply.headers[header::PRAGMA], "no-cache");
    assert_eq!(reply.headers[header::REFERRER_POLICY], "no-referrer");
    assert!(
        reply.headers[header::VARY]
            .to_str()
            .unwrap()
            .to_lowercase()
            .contains("cookie")
    );
}
fn rejected(reply: &Reply, status: StatusCode, code: &str) {
    assert_eq!(reply.status, status, "{}", reply.body);
    assert_eq!(reply.body["code"], code);
    assert_eq!(reply.body.as_object().unwrap().len(), 2);
    private(reply);
    for secret in [
        "fixture-sensitive-marker",
        "canteiro_auth",
        "SELECT ",
        "INSERT ",
    ] {
        assert!(!reply.body.to_string().contains(secret));
    }
}
fn state(reply: &Reply) -> AccessResponse {
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.body);
    private(reply);
    serde_json::from_value(reply.body.clone()).unwrap()
}
fn context(response: &AccessResponse) -> Value {
    let org = response.state.organization.as_ref().unwrap();
    json!({"organization_id":org.id,"context_version":response.state.context_version,"organization_version":org.version})
}
async fn post(f: &Fixture, s: &Reply, body: Value) -> Reply {
    access_request(
        f,
        Some(s),
        Some(body),
        Some(ORIGIN),
        Some(&s.success().csrf),
    )
    .await
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn scoped_access_http_rejects_forged_identity_csrf_and_old_sessions() {
    let f = Fixture::new().await;
    rejected(
        &access_request(&f, None, None, None, None).await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    let anonymous = f.request(None, None, None, None).await;
    rejected(
        &access_request(&f, Some(&anonymous), None, None, None).await,
        StatusCode::FORBIDDEN,
        "forbidden",
    );
    let (_, _, logged) = f.account().await;
    let csrf = logged.success().csrf;
    let body = json!({"command":"create_scope","context":{"organization_id":Uuid::new_v4(),"context_version":1,"organization_version":1},"kind":"work","name":"Obra fictícia","parent_id":null});
    for origin in [None, Some("null"), Some("https://other.example.test")] {
        rejected(
            &access_request(&f, Some(&logged), Some(body.clone()), origin, Some(&csrf)).await,
            StatusCode::FORBIDDEN,
            "origin_invalid",
        );
    }
    for token in [None, Some("incorrect")] {
        rejected(
            &access_request(&f, Some(&logged), Some(body.clone()), Some(ORIGIN), token).await,
            StatusCode::FORBIDDEN,
            "csrf_invalid",
        );
    }
    let mut injected = body.clone();
    injected["actor_id"] = json!("fixture-sensitive-marker");
    rejected(
        &post(&f, &logged, injected).await,
        StatusCode::BAD_REQUEST,
        "invalid_input",
    );
    let mut injected = body.clone();
    injected["context"]["administrator"] = json!(true);
    rejected(
        &post(&f, &logged, injected).await,
        StatusCode::BAD_REQUEST,
        "invalid_input",
    );
    rejected(
        &post(&f, &logged, json!({"command":"fixture-sensitive-marker"})).await,
        StatusCode::BAD_REQUEST,
        "invalid_input",
    );
    let current = state(&access_request(&f, Some(&logged), None, None, None).await);
    assert!(current.state.organization.is_none() && current.state.scopes.is_empty());
    f.run(&logged, AuthCommand::Logout).await.success();
    rejected(
        &access_request(&f, Some(&logged), None, None, None).await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    rejected(
        &post(&f, &logged, body).await,
        StatusCode::UNAUTHORIZED,
        "session_expired",
    );
    f.database.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn scoped_access_http_mutations_require_explicit_current_rights_and_versions() {
    let f = Fixture::new().await;
    let (email, password, logged) = f.account().await;
    let setup = f.run(&logged, AuthCommand::StartMfa { password }).await;
    let session = f
        .run(
            &setup,
            AuthCommand::ConfirmMfa {
                code: super::organizations::authenticator_code(&setup.success().mfa_setup.unwrap()),
            },
        )
        .await;
    let token = session.bearer();
    let created = f
        .service
        .organizations_execute(
            &token,
            OrganizationCommand::Create {
                name: "Empresa de permissões HTTP".into(),
            },
        )
        .await
        .unwrap();
    let org = &created.state.organizations[0];
    f.service
        .organizations_execute(
            &token,
            OrganizationCommand::Select {
                organization_id: org.id.clone(),
                expected_context_version: created.state.context_version,
            },
        )
        .await
        .unwrap();
    let current = state(&access_request(&f, Some(&session), None, None, None).await);
    let create = json!({"command":"create_scope","context":context(&current),"kind":"work","name":"Obra HTTP","parent_id":null});
    let scope = state(&post(&f, &session, create.clone()).await);
    let scope_id = &scope.state.scopes[0].id;
    rejected(
        &post(&f, &session, create).await,
        StatusCode::CONFLICT,
        "version_conflict",
    );
    let propose = json!({"command":"propose","context":context(&scope),"scope_id":scope_id,"action":"approve_change","title":"Mudança controlada","increase":"100.00","decrease":"100.00","conflicted_user_ids":[]});
    rejected(
        &post(&f, &session, propose.clone()).await,
        StatusCode::FORBIDDEN,
        "access_denied",
    );
    let mut numeric = propose.clone();
    numeric["increase"] = json!(100);
    rejected(
        &post(&f, &session, numeric).await,
        StatusCode::BAD_REQUEST,
        "invalid_input",
    );
    let mut too_precise = propose.clone();
    too_precise["increase"] = json!("100.001");
    rejected(
        &post(&f, &session, too_precise).await,
        StatusCode::BAD_REQUEST,
        "invalid_input",
    );
    let granted=state(&post(&f,&session,json!({"command":"grant","context":context(&scope),"scope_id":scope_id,"user_email":email,"role":"inspector","actions":["view","propose_change"],"max_amount":"1000.00","valid_from":"2020-01-01T00:00:00Z","valid_until":null,"required":false})).await);
    let policy=state(&post(&f,&session,json!({"command":"set_policy","context":context(&granted),"scope_id":scope_id,"action":"approve_change","threshold":"100.00","approvals_above_threshold":2,"segregate_proposer":true,"expected_version":null})).await);
    let mut propose = propose;
    propose["context"] = context(&policy);
    let requested = state(&post(&f, &session, propose).await);
    let request = &requested.state.requests[0];
    assert_eq!(request.authority_amount.to_string(), "100.00");
    assert!(request.contains_increase);
    assert_eq!(request.required_approvals, 2);
    assert_eq!(request.valid_approvals, 0);
    assert!(request.allowed_grant_ids.is_empty());
    let decision = json!({"command":"decide","context":context(&requested),"request_id":request.id,"request_version":request.version,"grant_id":requested.state.grants[0].id,"grant_version":1,"approve":true});
    rejected(
        &post(&f, &session, decision).await,
        StatusCode::FORBIDDEN,
        "access_denied",
    );
    let mut foreign = json!({"command":"create_scope","context":context(&requested),"kind":"work","name":"fixture-sensitive-marker","parent_id":null});
    foreign["context"]["organization_id"] = json!(Uuid::new_v4());
    rejected(
        &post(&f, &session, foreign).await,
        StatusCode::CONFLICT,
        "context_changed",
    );
    let final_state = state(&access_request(&f, Some(&session), None, None, None).await);
    assert_eq!(final_state.state.scopes.len(), 1);
    assert_eq!(final_state.state.requests[0].status, "pending");
    f.database.close().await;
}
