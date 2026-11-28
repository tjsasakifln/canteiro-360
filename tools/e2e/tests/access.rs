#[allow(dead_code)]
mod support;

use canteiro_contracts::access::*;
use serde_json::{Value, json};
use std::time::Duration;
use support::{Browser, ENTER, ESCAPE, HOME, Harness, TestResult, access_fixture::*, write_json};

const WAIT: Duration = Duration::from_secs(30);

async fn ready(browser: &Browser) -> TestResult {
    browser.wait("return document.querySelector('[data-perm-ready]')?.dataset.permReady==='true'&&document.querySelector('[data-perm-busy]')?.dataset.permBusy==='false';",WAIT).await
}
async fn enter(h: &Harness, browser: &Browser, person: &AccountFixture) -> TestResult {
    browser.navigate(&h.app_url).await?;
    browser.post("cookie",json!({"cookie":{"name":"__Host-canteiro-session","value":person.session,"path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"}})).await?;
    browser
        .navigate(&format!("{}/permissoes", h.app_url))
        .await?;
    ready(browser).await
}
async fn select(browser: &Browser, id: &str) -> TestResult {
    browser
        .click(&format!("#perm-scope-selection option[value='{id}']"))
        .await?;
    browser.click("#perm-scope-open").await?;
    browser
        .wait(
            "return !!document.querySelector('#perm-workspace-title');",
            WAIT,
        )
        .await
}
async fn expand(browser: &Browser, id: &str) -> TestResult {
    if browser.property(id, "open").await? != true {
        browser.click(&format!("{id} > summary")).await?;
    }
    let selector = serde_json::to_string(id)?;
    browser
        .wait(
            &format!("return document.querySelector({selector})?.open === true;"),
            WAIT,
        )
        .await
}
async fn confirm(browser: &Browser) -> TestResult {
    browser
        .wait(
            "return document.querySelector('#perm-confirm-dialog')?.open;",
            WAIT,
        )
        .await?;
    browser.tab_to("#perm-confirm-submit").await?;
    browser.key(ENTER).await?;
    ready(browser).await
}
async fn cancel_confirmation(browser: &Browser) -> TestResult {
    browser
        .wait(
            "return document.querySelector('#perm-confirm-dialog')?.open;",
            WAIT,
        )
        .await?;
    browser.key(ESCAPE).await?;
    browser
        .wait(
            "return !document.querySelector('#perm-confirm-dialog')?.open;",
            WAIT,
        )
        .await
}
async fn company_route(browser: &Browser) -> TestResult {
    browser
        .tab_to(".site-account-nav a[href='/organizacoes']")
        .await?;
    browser.key(ENTER).await?;
    browser
        .wait("return !!document.querySelector('#org-title');", WAIT)
        .await?;
    assert_eq!(
        browser
            .execute("return !document.querySelector('.permissions-page');")
            .await?,
        true
    );
    Ok(())
}
async fn permission_route(browser: &Browser) -> TestResult {
    browser
        .tab_to(".site-account-nav a[href='/permissoes']")
        .await?;
    browser.key(ENTER).await?;
    ready(browser).await
}
async fn layout(browser: &Browser, h: &Harness, label: &str, width: u32) -> TestResult<Value> {
    browser.viewport(width, 1000, width == 360).await?;
    ready(browser).await?;
    if label == "administrator" {
        for panel in [
            "#perm-create-panel",
            "#perm-grant-panel",
            "#perm-policy-panel",
        ] {
            expand(browser, panel).await?;
        }
    }
    // Capture a real top-of-page viewport. A fixed skip link transformed above
    // a scrolled viewport can otherwise appear inside a full-page CDP image.
    browser.click("#perm-title").await?;
    browser.chord(&["\u{e009}"], HOME).await?;
    browser.wait("return scrollY===0;", WAIT).await?;
    let report=browser.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,fields:[...document.querySelectorAll('.permissions-page input,.permissions-page select')].filter(e=>e.getClientRects().length).map(e=>({id:e.id,label:!!document.querySelector('label[for=\"'+e.id+'\"]')}))};").await?;
    assert!(
        report["scroll"].as_u64().unwrap() <= report["width"].as_u64().unwrap(),
        "Permissions overflow"
    );
    assert!(
        report["fields"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["label"] == true),
        "Unlabelled permissions field"
    );
    browser
        .full_screenshot(&h.evidence.join(format!("permissions-{label}-{width}.png")))
        .await?;
    Ok(report)
}

#[tokio::test]
#[ignore = "requires built SSR/WASM server, PostgreSQL and WebDriver"]
async fn scoped_roles_policies_and_independent_decisions_work_in_the_browser() -> TestResult {
    let h = Harness::start("permissions-interface").await?;
    let owner = account(&h, "owner", true).await?;
    let first = account(&h, "first", true).await?;
    let second = account(&h, "second", true).await?;
    let reader = account(&h, "reader", false).await?;
    let id = company(&h, &owner, "Aurora Permissões").await?;
    for person in [&first, &second, &reader] {
        join(&h, &owner, person, &id).await?;
    }
    let browser = Browser::start(&h).await?;
    let reviewer = Browser::start(&h).await?;
    let html = h
        .http
        .get(format!("{}/permissoes", h.app_url))
        .send()
        .await?
        .text()
        .await?;
    assert!(html.contains("Permissões e aprovações") && !html.contains(&owner.email));
    enter(&h, &browser, &owner).await?;
    expand(&browser, "#perm-create-panel").await?;
    browser.fill("#perm-scope-name", "Obra Aurora").await?;
    browser.tab_to("#perm-scope-create").await?;
    browser.key(ENTER).await?;
    ready(&browser).await?;
    let state = h.auth.access_context(&owner.session).await?.state;
    let work = state
        .scopes
        .iter()
        .find(|s| s.name == "Obra Aurora")
        .ok_or("work creation failed")?
        .id
        .clone();
    browser
        .click("#perm-scope-kind option[value='contract']")
        .await?;
    browser
        .fill("#perm-scope-name", "Contrato Estrutural")
        .await?;
    browser
        .click(&format!("#perm-scope-parent option[value='{work}']"))
        .await?;
    browser.click("#perm-scope-create").await?;
    ready(&browser).await?;
    assert!(
        h.auth
            .access_context(&owner.session)
            .await?
            .state
            .scopes
            .iter()
            .any(|s| s.kind == ScopeKind::Contract && s.parent_id.as_ref() == Some(&work))
    );
    let contract = h
        .auth
        .access_context(&owner.session)
        .await?
        .state
        .scopes
        .into_iter()
        .find(|s| s.name == "Contrato Estrutural")
        .ok_or("contract fixture missing")?
        .id;
    browser
        .click("#perm-scope-kind option[value='budget']")
        .await?;
    browser
        .fill("#perm-scope-name", "Orçamento Estrutural")
        .await?;
    browser
        .click(&format!("#perm-scope-parent option[value='{contract}']"))
        .await?;
    browser.click("#perm-scope-create").await?;
    ready(&browser).await?;
    assert!(
        h.auth
            .access_context(&owner.session)
            .await?
            .state
            .scopes
            .iter()
            .any(|s| s.kind == ScopeKind::Budget && s.parent_id.as_ref() == Some(&contract))
    );
    select(&browser, &work).await?;
    expand(&browser, "#perm-request-panel").await?;
    assert_eq!(
        browser.property("#perm-request-submit", "disabled").await?,
        true,
        "Administrator acquired implicit proposing authority"
    );
    expand(&browser, "#perm-grant-panel").await?;
    browser.fill("#perm-grant-email", &owner.email).await?;
    browser
        .click("#perm-grant-role option[value='inspector']")
        .await?;
    browser.fill("#perm-grant-limit", "1.000,00").await?;
    browser.click("#perm-grant-submit").await?;
    browser
        .wait(
            "return document.querySelector('#perm-confirm-dialog')?.open;",
            WAIT,
        )
        .await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "perm-confirm-cancel"
    );
    browser.key(ESCAPE).await?;
    assert_eq!(
        browser.property("#perm-grant-email", "value").await?,
        owner.email
    );
    assert!(
        h.auth
            .access_context(&owner.session)
            .await?
            .state
            .grants
            .is_empty()
    );
    browser.click("#perm-grant-submit").await?;
    confirm(&browser).await?;
    assert_eq!(
        browser.property("#perm-request-submit", "disabled").await?,
        false
    );
    expand(&browser, "#perm-policy-panel").await?;
    browser.fill("#perm-policy-threshold", "100,00").await?;
    browser.click("#perm-policy-submit").await?;
    confirm(&browser).await?;
    browser.fill("#perm-policy-threshold", "500,00").await?;
    browser
        .click("#perm-policy-action option[value='authorize_payment']")
        .await?;
    cancel_confirmation(&browser).await?;
    assert_eq!(
        browser.property("#perm-policy-action", "value").await?,
        "approve_change"
    );
    assert_eq!(
        browser.property("#perm-policy-threshold", "value").await?,
        "500,00"
    );
    browser
        .click("#perm-policy-action option[value='authorize_payment']")
        .await?;
    confirm(&browser).await?;
    assert_eq!(
        browser.property("#perm-policy-action", "value").await?,
        "authorize_payment"
    );
    assert_eq!(
        browser.property("#perm-policy-threshold", "value").await?,
        "0,00"
    );
    browser
        .click("#perm-policy-action option[value='approve_change']")
        .await?;
    assert_eq!(
        browser.property("#perm-policy-threshold", "value").await?,
        "100,00"
    );
    for person in [&owner, &first, &second] {
        fixture_grant(
            &h,
            &owner,
            person,
            &work,
            AccessRole::Approver,
            vec![AccessAction::View, AccessAction::ApproveChange],
        )
        .await?;
    }
    fixture_grant(
        &h,
        &owner,
        &reader,
        &work,
        AccessRole::Viewer,
        vec![AccessAction::View],
    )
    .await?;
    let c = h.auth.access_context(&owner.session).await?;
    h.auth
        .access_execute(
            &owner.session,
            AccessCommand::CreateScope {
                context: scope(&c.state),
                kind: ScopeKind::Work,
                name: "Obra reservada".into(),
                parent_id: None,
            },
        )
        .await?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    expand(&browser, "#perm-request-panel").await?;
    browser
        .fill("#perm-request-title", "Alteração mista do canteiro")
        .await?;
    browser.fill("#perm-request-increase", "100,00").await?;
    browser.fill("#perm-request-decrease", "100,00").await?;
    // Execute the request against the real server and drop only its response.
    browser.execute(r#"
        window.fixtureFetch = window.fetch.bind(window);
        window.fixtureAuthorityWrites = 0;
        window.fixtureAuthorityQueries = 0;
        window.fetch = async (...args) => {
          const response = await window.fixtureFetch(...args);
          const path = new URL(args[0] instanceof Request ? args[0].url : args[0], location.href).pathname;
          if (path.startsWith('/api/access/operations/')) window.fixtureAuthorityQueries += 1;
          if (path === '/api/access/command' && ++window.fixtureAuthorityWrites === 1 && response.ok) {
            throw new TypeError('Fixture lost the response after the HTTP commit');
          }
          return response;
        };
        return true;
    "#).await?;
    browser.click("#perm-request-submit").await?;
    browser
        .wait(
            "return document.querySelector('#perm-confirm-dialog')?.open;",
            WAIT,
        )
        .await?;
    browser.tab_to("#perm-confirm-submit").await?;
    browser.key(ENTER).await?;
    browser.wait(
        "return document.querySelector('#perm-feedback')?.textContent.includes('Não foi possível confirmar a resposta');",
        WAIT,
    ).await?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    expand(&browser, "#perm-request-panel").await?;
    assert_eq!(
        browser.property("#perm-request-title", "value").await?,
        "Alteração mista do canteiro"
    );
    browser.tab_to("#perm-authority-retry").await?;
    browser.key(ENTER).await?;
    browser.wait(
        "return document.querySelector('#perm-feedback')?.textContent.includes('Pedido registrado para decisão');",
        WAIT,
    ).await?;
    assert_eq!(
        browser.execute("const result = [window.fixtureAuthorityWrites, window.fixtureAuthorityQueries]; window.fetch = window.fixtureFetch; return result;").await?,
        json!([1, 1]),
        "A resposta perdida foi recuperada por consulta, sem repetir o POST"
    );
    browser
        .wait(
            "return document.activeElement?.id === 'perm-feedback';",
            WAIT,
        )
        .await?;
    assert_eq!(
        browser.property("#perm-scope-selection", "value").await?,
        work
    );
    assert_eq!(browser.property("#perm-request-title", "value").await?, "");
    let request = h
        .auth
        .access_context(&owner.session)
        .await?
        .state
        .requests
        .into_iter()
        .find(|r| r.title == "Alteração mista do canteiro")
        .ok_or("request missing")?;
    assert_eq!(request.authority_amount.to_string(), "100.00");
    assert!(request.contains_increase);
    assert_eq!(request.required_approvals, 2);
    assert!(
        request.allowed_grant_ids.is_empty(),
        "Proponent can approve own request"
    );
    assert_eq!(
        browser
            .execute("return document.querySelectorAll('button[id^=perm-approve-]').length;")
            .await?,
        0
    );
    let current = h.auth.access_context(&owner.session).await?;
    let second_request = h
        .auth
        .access_execute(
            &owner.session,
            AccessCommand::Propose {
                context: scope(&current.state),
                scope_id: work.clone(),
                action: AccessAction::ApproveChange,
                title: "Pedido para revisão".into(),
                increase: "10.00".parse()?,
                decrease: "0.00".parse()?,
                conflicted_user_ids: vec![],
            },
        )
        .await?
        .state
        .requests
        .into_iter()
        .find(|r| r.title == "Pedido para revisão")
        .ok_or("second request fixture missing")?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    browser
        .click(&format!("#perm-revise-{}", request.id))
        .await?;
    // Revision opens the details and moves focus on the next animation frame.
    // Require both user-visible effects before typing, without opening it here.
    browser
        .wait(
            "return document.querySelector('#perm-request-panel')?.open && document.activeElement?.id === 'perm-request-title';",
            WAIT,
        )
        .await?;
    browser
        .fill("#perm-request-title", "Revisão ainda não enviada")
        .await?;
    browser.fill("#perm-request-increase", "150,00").await?;
    browser
        .click(&format!("#perm-revise-{}", second_request.id))
        .await?;
    cancel_confirmation(&browser).await?;
    assert_eq!(
        browser.property("#perm-request-title", "value").await?,
        "Revisão ainda não enviada"
    );
    assert_eq!(
        browser.property("#perm-request-increase", "value").await?,
        "150,00"
    );
    browser
        .click(&format!("#perm-revise-{}", second_request.id))
        .await?;
    confirm(&browser).await?;
    assert_eq!(
        browser.property("#perm-request-title", "value").await?,
        "Pedido para revisão"
    );
    assert_eq!(
        browser.property("#perm-request-increase", "value").await?,
        "10,00"
    );
    browser.click("#perm-request-cancel").await?;
    let mut layouts = vec![];
    for width in [360, 1280] {
        layouts.push(layout(&browser, &h, "administrator", width).await?);
    }
    for (i, person) in [&first, &second].into_iter().enumerate() {
        enter(&h, &reviewer, person).await?;
        assert!(
            !reviewer
                .text("#perm-scope-selection")
                .await?
                .contains("Obra reservada")
        );
        assert!(
            !reviewer
                .text("#perm-scope-selection")
                .await?
                .contains("Contrato Estrutural")
        );
        assert!(
            !reviewer
                .text("#perm-scope-selection")
                .await?
                .contains("Orçamento Estrutural")
        );
        select(&reviewer, &work).await?;
        reviewer
            .tab_to(&format!("#perm-approve-{}", request.id))
            .await?;
        reviewer.key(ENTER).await?;
        confirm(&reviewer).await?;
        let current = h
            .auth
            .access_context(&person.session)
            .await?
            .state
            .requests
            .into_iter()
            .find(|r| r.id == request.id)
            .ok_or("decision missing")?;
        assert_eq!(current.valid_approvals, (i + 1) as u8);
        assert_eq!(current.status, if i == 0 { "pending" } else { "approved" });
    }
    enter(&h, &reviewer, &reader).await?;
    select(&reviewer, &work).await?;
    assert_eq!(reviewer.execute("return !document.querySelector('#perm-grant-panel')&&!document.querySelector('#perm-policy-submit')&&document.querySelectorAll('button[id^=perm-approve-]').length===0;").await?,true);
    expand(&reviewer, "#perm-request-panel").await?;
    assert_eq!(
        reviewer
            .property("#perm-request-submit", "disabled")
            .await?,
        true
    );
    for width in [360, 1280] {
        layouts.push(layout(&reviewer, &h, "reader", width).await?);
    }
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    let source = h
        .auth
        .access_context(&owner.session)
        .await?
        .state
        .grants
        .into_iter()
        .find(|g| {
            g.user_email == owner.email
                && g.role == AccessRole::Approver
                && g.source_grant_id.is_none()
        })
        .ok_or("source grant missing")?;
    let other_source = h
        .auth
        .access_context(&owner.session)
        .await?
        .state
        .grants
        .into_iter()
        .find(|g| g.user_email == owner.email && g.role == AccessRole::Inspector && g.can_delegate)
        .ok_or("second source grant missing")?;
    browser
        .click(&format!("#perm-grant-delegate-{}", source.id))
        .await?;
    browser.fill("#perm-delegate-email", &first.email).await?;
    browser.fill("#perm-delegate-limit", "50,00").await?;
    browser
        .fill(
            "#perm-delegate-reason",
            "Cobertura durante férias do titular",
        )
        .await?;
    browser
        .click(&format!("#perm-grant-delegate-{}", other_source.id))
        .await?;
    cancel_confirmation(&browser).await?;
    assert_eq!(
        browser.property("#perm-delegate-email", "value").await?,
        first.email
    );
    assert_eq!(
        browser.property("#perm-delegate-reason", "value").await?,
        "Cobertura durante férias do titular"
    );
    assert_eq!(
        browser.property("#perm-delegate-limit", "value").await?,
        "50,00"
    );
    browser
        .click(&format!("#perm-grant-delegate-{}", other_source.id))
        .await?;
    confirm(&browser).await?;
    assert_eq!(browser.property("#perm-delegate-email", "value").await?, "");
    assert_eq!(
        browser.property("#perm-delegate-reason", "value").await?,
        ""
    );
    browser
        .click(&format!("#perm-grant-delegate-{}", source.id))
        .await?;
    confirm(&browser).await?;
    browser.fill("#perm-delegate-email", &first.email).await?;
    browser.fill("#perm-delegate-limit", "50,00").await?;
    browser
        .fill(
            "#perm-delegate-reason",
            "Cobertura durante férias do titular",
        )
        .await?;
    browser.click("#perm-delegate-substitution").await?;
    browser.click("#perm-delegate-submit").await?;
    confirm(&browser).await?;
    assert!(
        h.auth
            .access_context(&owner.session)
            .await?
            .state
            .grants
            .iter()
            .any(|g| g.source_grant_id.as_ref() == Some(&source.id)
                && g.substitution
                && g.reason == "Cobertura durante férias do titular"
                && g.max_amount.to_string() == "50.00")
    );
    expand(&browser, "#perm-grant-panel").await?;
    browser
        .fill("#perm-grant-email", "rascunho@example.invalid")
        .await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":true,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#perm-refresh").await?;
    browser
        .wait(
            "return document.querySelector('[data-perm-busy]')?.dataset.permBusy==='false';",
            WAIT,
        )
        .await?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    expand(&browser, "#perm-grant-panel").await?;
    assert_eq!(
        browser.property("#perm-grant-email", "value").await?,
        "rascunho@example.invalid"
    );
    // Router navigation and Back/Forward retain a bound draft without keeping
    // the private AccessState or form in the other route's DOM.
    company_route(&browser).await?;
    permission_route(&browser).await?;
    expand(&browser, "#perm-grant-panel").await?;
    assert_eq!(
        browser.property("#perm-grant-email", "value").await?,
        "rascunho@example.invalid"
    );
    browser.history_back().await?;
    browser
        .wait("return !!document.querySelector('#org-title');", WAIT)
        .await?;
    browser.post("forward", json!({})).await?;
    ready(&browser).await?;
    expand(&browser, "#perm-grant-panel").await?;
    assert_eq!(
        browser.property("#perm-grant-email", "value").await?,
        "rascunho@example.invalid"
    );
    company_route(&browser).await?;
    // Another identity cannot restore the previous person's suspended draft.
    browser.post("cookie",json!({"cookie":{"name":"__Host-canteiro-session","value":reader.session,"path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"}})).await?;
    permission_route(&browser).await?;
    assert!(
        !browser
            .text(".permissions-page")
            .await?
            .contains("rascunho@example.invalid")
    );
    assert_eq!(browser.execute("return !document.querySelector('#perm-grant-panel')&&!document.querySelector('#perm-policy-submit');").await?,true);
    // The explicit refresh also invalidates a draft without navigating away.
    browser.post("cookie",json!({"cookie":{"name":"__Host-canteiro-session","value":owner.session,"path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"}})).await?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    select(&browser, &work).await?;
    expand(&browser, "#perm-grant-panel").await?;
    browser
        .fill("#perm-grant-email", "rascunho@example.invalid")
        .await?;
    browser.post("cookie",json!({"cookie":{"name":"__Host-canteiro-session","value":reader.session,"path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"}})).await?;
    browser.click("#perm-refresh").await?;
    ready(&browser).await?;
    assert!(
        !browser
            .text(".permissions-page")
            .await?
            .contains("rascunho@example.invalid")
    );
    assert_eq!(browser.execute("return !document.querySelector('#perm-grant-panel')&&!document.querySelector('#perm-policy-submit');").await?,true);
    let storage=browser.execute("return {local:Object.keys(localStorage),session:Object.keys(sessionStorage),url:location.pathname+location.search+location.hash};").await?;
    assert!(
        storage["local"].as_array().unwrap().is_empty()
            && storage["session"].as_array().unwrap().is_empty()
    );
    assert_eq!(storage["url"], "/permissoes");
    let logs = browser.logs().await?;
    assert!(!logs.to_string().contains("panicked") && !logs.to_string().contains("Hydration"));
    write_json(
        &h.evidence.join("permissions-interface.json"),
        &json!({"status":"passed","environment":h.environment,"layouts":layouts,"scenarios":["public SSR without private data","explicit work scope","administrator has no implicit proposing power","role grant and cancelled confirmation","threshold policy with segregation","policy switch cancelled with visual selection restored","revision switch requires explicit discard","delegation source switch requires explicit discard","mixed increase and decrease","authority recovery by keyboard after losing the confirmed HTTP response","proponent cannot approve","two independent approvers","reader cannot configure or decide","temporary substitution with lower authority","network failure preserves scoped draft","Router and history preserve identity-bound draft","another identity cannot restore suspended draft","identity change removes draft and admin controls","no private browser storage"]}),
    )?;
    browser.close().await?;
    reviewer.close().await?;
    Ok(())
}
