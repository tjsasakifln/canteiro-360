//! Formulários nativos atravessam os mesmos controles de origem, sessão e CSRF.
use super::*;
use canteiro_contracts::organizations::OrganizationCommand;
use std::collections::BTreeMap;

async fn exchange(
    f: &Fixture,
    session: &Reply,
    path: &str,
    form: Option<&BTreeMap<String, String>>,
    origin: Option<&str>,
) -> (StatusCode, axum::http::HeaderMap, String) {
    let mut request = Request::builder()
        .method(if form.is_some() { "POST" } else { "GET" })
        .uri(path)
        .header(
            header::COOKIE,
            session.cookie.as_ref().unwrap().split(';').next().unwrap(),
        )
        .header("sec-fetch-site", "same-origin")
        .header("sec-fetch-mode", "navigate")
        .header("sec-fetch-dest", "document");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    let body = if let Some(form) = form {
        request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        let mut encoded = reqwest::Url::parse(ORIGIN).unwrap();
        encoded.query_pairs_mut().extend_pairs(form);
        Body::from(encoded.query().unwrap().to_owned())
    } else {
        Body::empty()
    };
    let reply = f
        .app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let (parts, body) = reply.into_parts();
    let bytes = to_bytes(body, 524_288).await.unwrap();
    (
        parts.status,
        parts.headers,
        String::from_utf8(bytes.to_vec()).unwrap(),
    )
}

fn input_value(html: &str, name: &str) -> String {
    let needle = format!("name=\"{name}\"");
    html.split("<input")
        .skip(1)
        .map(|input| input.split('>').next().unwrap())
        .find(|input| input.contains(&needle))
        .unwrap_or_else(|| panic!("campo {name} ausente do formulário"))
        .split("value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .to_owned()
}

fn saved_form(html: &str) -> BTreeMap<String, String> {
    [
        "organization_id",
        "context_version",
        "csrf",
        "operation_key",
        "action",
        "project_id",
        "expected_version",
        "reason",
    ]
    .into_iter()
    .map(|name| (name.to_owned(), input_value(html, name)))
    .collect()
}

fn native_private_headers(headers: &axum::http::HeaderMap) {
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(headers[header::PRAGMA], "no-cache");
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    // Fetch 3.2 serializa Origin como null em POST de navegação quando a página
    // usa no-referrer. same-origin preserva o POST interno e oculta a referência
    // entre sites, sem dispensar a checagem de Origin no servidor.
    assert_eq!(headers[header::REFERRER_POLICY], "same-origin");
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn native_project_form_creates_independently_and_preserves_invalid_revision() {
    let f = Fixture::new().await;
    let (_, password, logged) = f.account().await;
    let setup = f.run(&logged, AuthCommand::StartMfa { password }).await;
    let owner = f
        .run(
            &setup,
            AuthCommand::ConfirmMfa {
                code: super::organizations::authenticator_code(&setup.success().mfa_setup.unwrap()),
            },
        )
        .await;
    let token = owner.bearer();
    let created = f
        .service
        .organizations_execute(
            &token,
            OrganizationCommand::Create {
                name: "Empresa de formulário nativo".into(),
            },
        )
        .await
        .unwrap();
    f.service
        .organizations_execute(
            &token,
            OrganizationCommand::Select {
                organization_id: created.state.organizations[0].id.clone(),
                expected_context_version: created.state.context_version,
            },
        )
        .await
        .unwrap();

    let (status, headers, html) = exchange(&f, &owner, "/projetos", None, None).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    native_private_headers(&headers);
    let mut form = saved_form(&html);
    assert_eq!(form["csrf"], owner.success().csrf);
    assert_eq!(form["action"], "save");
    for (name, value) in [
        ("title", "Projeto antes da obra"),
        ("discipline", "Estruturas"),
        (
            "objective",
            "Definir a estrutura de um empreendimento fictício.",
        ),
        (
            "scope",
            "Estudo preliminar e critérios de aceite, sem obra ou contrato vinculado.",
        ),
        ("work_id", ""),
        ("external_reference", ""),
    ] {
        form.insert(name.into(), value.into());
    }
    for origin in [None, Some("null"), Some("https://other.example.test")] {
        let (status, _, body) = exchange(&f, &owner, "/projetos/acao", Some(&form), origin).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap()["code"],
            "origin_invalid"
        );
    }
    let mut wrong_csrf = form.clone();
    wrong_csrf.insert("csrf".into(), "invalid".into());
    let (status, _, body) = exchange(
        &f,
        &owner,
        "/projetos/acao",
        Some(&wrong_csrf),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["code"],
        "csrf_invalid"
    );
    assert!(
        f.service
            .projects_context(&token)
            .await
            .unwrap()
            .projects
            .is_empty()
    );

    let (status, headers, body) =
        exchange(&f, &owner, "/projetos/acao", Some(&form), Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    let location = headers[header::LOCATION].to_str().unwrap();
    let (status, headers, html) = exchange(&f, &owner, location, None, None).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    native_private_headers(&headers);
    assert!(html.contains("Projeto antes da obra"));
    assert!(html.contains("Em elaboração"));
    let context = f.service.projects_context(&token).await.unwrap();
    assert_eq!(context.projects.len(), 1);
    let project = &context.projects[0];
    assert_eq!(project.snapshot.spec.title, "Projeto antes da obra");
    assert!(project.snapshot.spec.work_id.is_none());
    assert_eq!(project.version, 1);

    let mut revision = saved_form(&html);
    for (name, value) in [
        ("title", "Título da revisão preservada"),
        ("discipline", "X"),
        ("objective", "Objetivo preservado após a recusa."),
        ("scope", "Escopo revisado sem perder o preenchimento."),
        ("work_id", ""),
        ("external_reference", "Referência preservada"),
        ("reason", "Conferência do formulário nativo"),
    ] {
        revision.insert(name.into(), value.into());
    }
    let (status, headers, html) =
        exchange(&f, &owner, "/projetos/acao", Some(&revision), Some(ORIGIN)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{html}");
    native_private_headers(&headers);
    for name in [
        "title",
        "discipline",
        "external_reference",
        "reason",
        "operation_key",
        "expected_version",
    ] {
        assert_eq!(input_value(&html, name), revision[name]);
    }
    assert!(html.contains(&revision["objective"]));
    assert!(html.contains(&revision["scope"]));
    assert!(html.contains("Informe título, disciplina, objetivo e escopo"));
    let saved = f.service.projects_context(&token).await.unwrap();
    assert_eq!(saved.projects[0].snapshot, project.snapshot);
    assert_eq!(saved.projects[0].version, 1);

    let (status, headers, html) = exchange(&f, &owner, "/obrigacoes", None, None).await;
    assert_eq!(status, StatusCode::OK, "{html}");
    native_private_headers(&headers);
    let (status, headers, _) = exchange(&f, &owner, "/api/projects/context", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::REFERRER_POLICY], "no-referrer");
    f.database.close().await;
}
