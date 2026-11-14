//! Os comandos atravessam a fronteira HTTP com sessão, CSRF e PostgreSQL real.
use super::*;
use canteiro_contracts::organizations::{OrganizationCommand, OrganizationRole};

async fn request(
    f: &Fixture,
    session: &Reply,
    path: &str,
    body: Option<Value>,
    csrf: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut req = Request::builder()
        .method(if body.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header(
            header::COOKIE,
            session.cookie.as_ref().unwrap().split(';').next().unwrap(),
        );
    let payload = if let Some(body) = body {
        req = req
            .header(header::ORIGIN, ORIGIN)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(csrf) = csrf {
            req = req.header("x-csrf-token", csrf);
        }
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let reply = f
        .app
        .clone()
        .oneshot(req.body(payload).unwrap())
        .await
        .unwrap();
    let (parts, body) = reply.into_parts();
    (
        parts.status,
        parts.headers,
        to_bytes(body, 524_288).await.unwrap().to_vec(),
    )
}

async fn post(f: &Fixture, session: &Reply, body: Value) -> (StatusCode, Value) {
    let (status, headers, bytes) = request(
        f,
        session,
        "/api/reliability/command",
        Some(body),
        Some(&session.success().csrf),
    )
    .await;
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn get(f: &Fixture, session: &Reply, path: &str) -> (StatusCode, Value) {
    let (status, headers, bytes) = request(f, session, path, None, None).await;
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn select(f: &Fixture, session: &Reply, org: &str) {
    let token = session.bearer();
    let state = f.service.organizations_context(&token).await.unwrap();
    f.service
        .organizations_execute(
            &token,
            OrganizationCommand::Select {
                organization_id: org.into(),
                expected_context_version: state.state.context_version,
            },
        )
        .await
        .unwrap();
}

async fn access_state(f: &Fixture, session: &Reply) -> canteiro_contracts::access::AccessResponse {
    super::access::state(&super::access::access_request(f, Some(session), None, None, None).await)
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn contract_http_replays_only_for_current_editor_and_preserves_versions_and_report() {
    let f = Fixture::new().await;
    let (owner_email, password, original) = f.account().await;
    let setup = f.run(&original, AuthCommand::StartMfa { password }).await;
    let owner = f
        .run(
            &setup,
            AuthCommand::ConfirmMfa {
                code: super::organizations::authenticator_code(&setup.success().mfa_setup.unwrap()),
            },
        )
        .await;
    let token = owner.bearer();
    let (_, _, reader) = f.account().await;
    let created = f
        .service
        .organizations_execute(
            &token,
            OrganizationCommand::Create {
                name: "Empresa de histórico HTTP".into(),
            },
        )
        .await
        .unwrap();
    let org = &created.state.organizations[0].id;
    select(&f, &owner, org).await;
    let current = f.service.organizations_context(&token).await.unwrap();
    let active = current.state.active.as_ref().unwrap();
    let invitation = f
        .service
        .organizations_execute(
            &token,
            OrganizationCommand::Invite {
                scope: canteiro_contracts::organizations::OrganizationScope {
                    organization_id: org.clone(),
                    context_version: current.state.context_version,
                    organization_version: active.organization.version,
                },
                email: reader.success().context.email.unwrap(),
                role: OrganizationRole::Member,
                valid_for_hours: 1,
                membership_valid_until: None,
            },
        )
        .await
        .unwrap();
    let reader_email = reader.success().context.email.unwrap();
    f.service
        .organizations_execute(
            &reader.bearer(),
            OrganizationCommand::AcceptInvitation {
                token: invitation.invitation_code.unwrap(),
            },
        )
        .await
        .unwrap();
    select(&f, &reader, org).await;

    let mut a = access_state(&f, &owner).await;
    let work = super::access::state(&super::access::post(&f,&owner,
        json!({"command":"create_scope","context":super::access::context(&a),"kind":"work","name":"Obra de teste","parent_id":null})
    ).await).state.scopes[0].id.clone();
    a = access_state(&f, &owner).await;
    let response = super::access::state(&super::access::post(&f,&owner,
        json!({"command":"create_scope","context":super::access::context(&a),"kind":"contract","name":"Contrato inicial","parent_id":work})
    ).await);
    let contract = response
        .state
        .scopes
        .iter()
        .find(|v| v.name == "Contrato inicial")
        .unwrap()
        .id
        .clone();
    for (email, role, actions) in [
        (&owner_email, "viewer", json!(["view"])),
        (
            &owner_email,
            "manager",
            json!(["view", "edit_contract_record"]),
        ),
        (&reader_email, "viewer", json!(["view"])),
    ] {
        a = access_state(&f, &owner).await;
        super::access::state(&super::access::post(&f,&owner,
            json!({"command":"grant","context":super::access::context(&a),"scope_id":contract,"user_email":email,"role":role,"actions":actions,"max_amount":"0","valid_from":"2020-01-01T00:00:00Z","valid_until":null,"required":false})
        ).await);
    }

    let (ok, ctx) = get(&f, &owner, "/api/reliability/context").await;
    assert_eq!(ok, StatusCode::OK);
    let scope = &ctx["organization"]["scope"];
    let base = format!("/api/reliability/contracts/{contract}");
    let query = |v: &Value| {
        format!(
            "?organization_id={org}&context_version={}",
            v["context_version"]
        )
    };
    let path = format!("{base}{}", query(scope));
    let (ok, initial) = get(&f, &owner, &path).await;
    assert_eq!(ok, StatusCode::OK);
    assert_eq!(initial["contract"]["version"], 1);
    assert_eq!(initial["history"][0]["after"], "Contrato inicial");

    let lease = Uuid::new_v4();
    let key = Uuid::new_v4();
    let acquire =
        json!({"command":"acquire","scope":scope,"contract_id":contract,"lease_id":lease});
    let (status, _, _) = request(
        &f,
        &owner,
        "/api/reliability/command",
        Some(acquire.clone()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(post(&f, &owner, acquire).await.0, StatusCode::OK);
    let input = json!({"command":"save","scope":scope,"contract_id":contract,"lease_id":lease,"key":key,"expected_version":1,"title":"=Ano fiscal","reason":"Ajuste retroativo","occurred_at":"2026-01-01T10:00:00-03:00"});
    let (first, second) = tokio::join!(
        post(&f, &owner, input.clone()),
        post(&f, &owner, input.clone())
    );
    assert_eq!(first.0, StatusCode::OK, "{:?}", first.1);
    assert_eq!(first, second);
    assert_eq!(first.1["receipt"]["version"], 2);
    let status_url = format!("{base}/operations/{key}{}", query(scope));
    assert_eq!(
        get(&f, &owner, &status_url).await,
        (StatusCode::OK, first.1["receipt"].clone())
    );

    let mut changed = input.clone();
    changed["title"] = json!("Outro título");
    let (status, error) = post(&f, &owner, changed).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "idempotency_conflict");
    let mut stale = input.clone();
    stale["key"] = json!(Uuid::new_v4());
    let (status, conflict) = post(&f, &owner, stale).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(conflict["outcome"], "version_conflict");
    assert_eq!(conflict["current"]["title"], "=Ano fiscal");

    let (_, reader_ctx) = get(&f, &reader, "/api/reliability/context").await;
    let reader_scope = &reader_ctx["organization"]["scope"];
    let report_url = format!("{base}/history.csv{}", query(reader_scope));
    let (status, headers, bytes) = request(&f, &reader, &report_url, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "attachment; filename=historico-do-contrato.csv"
    );
    let report = String::from_utf8(bytes).unwrap();
    assert_eq!(report.lines().count(), 3);
    assert!(report.contains("\"Contrato inicial\";\"'=Ano fiscal\""));
    assert!(report.contains("2026-01-01T13:00:00+00:00"));
    assert!(report.lines().last().unwrap().ends_with("\"Sim\""));
    let reader_status = format!("{base}/operations/{key}{}", query(reader_scope));
    assert_eq!(
        get(&f, &reader, &reader_status).await.0,
        StatusCode::FORBIDDEN
    );

    a = access_state(&f, &owner).await;
    let grant = a
        .state
        .grants
        .iter()
        .find(|g| g.role.as_str() == "manager")
        .unwrap();
    super::access::state(&super::access::post(&f,&owner,
        json!({"command":"revoke_grant","context":super::access::context(&a),"grant_id":grant.id,"grant_version":grant.version})
    ).await);
    assert_eq!(get(&f, &owner, &status_url).await.0, StatusCode::FORBIDDEN);
    assert_eq!(post(&f, &owner, input).await.0, StatusCode::FORBIDDEN);
    let (status, detail) = get(&f, &owner, &path).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["contract"]["can_edit"], false);
    assert_eq!(detail["history"].as_array().unwrap().len(), 2);
    f.database.close().await;
}
