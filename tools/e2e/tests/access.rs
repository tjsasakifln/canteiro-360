#[allow(dead_code)]
mod support;

use base64::Engine;
use canteiro_contracts::auth::AuthCommand;
use canteiro_contracts::{access::*, organizations::*};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha1::Sha1;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use support::{Browser, ENTER, ESCAPE, Harness, TestResult, write_json};

const WAIT: Duration = Duration::from_secs(30);

struct AccountFixture {
    email: String,
    session: String,
}

fn totp(secret: &str) -> TestResult<String> {
    let bytes = data_encoding::BASE32_NOPAD.decode(secret.as_bytes())?;
    let counter = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&bytes)?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 15);
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into()?) & 0x7fff_ffff;
    Ok(format!("{:06}", value % 1_000_000))
}

async fn account(h: &Harness, label: &str, mfa: bool) -> TestResult<AccountFixture> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| "Unable to generate fixture credentials")?;
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random);
    let email = format!(
        "permissions-{label}-{}@example.invalid",
        nonce.to_lowercase()
    );
    let password = format!("Acesso isolado {nonce}");
    let mut result = h.auth.context(None).await?;
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::Register {
                email: email.clone(),
                password: password.clone(),
            },
            &email,
        )
        .await?;
    let mut code = None;
    for message in h.auth.pending_mail(25).await? {
        if message.email == email && message.kind == "verify_email" {
            code = Some(message.token.clone());
        }
        h.auth.mark_mail_delivered(&message.id).await?;
    }
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::VerifyEmail {
                token: code.ok_or("Missing verification fixture")?,
                password: password.clone(),
            },
            &email,
        )
        .await?;
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::Login {
                email: email.clone(),
                password: password.clone(),
            },
            &email,
        )
        .await?;
    if mfa {
        result = h
            .auth
            .execute(
                &result.session_token,
                AuthCommand::StartMfa { password },
                &email,
            )
            .await?;
        let setup = result
            .mfa_setup
            .as_ref()
            .ok_or("Missing MFA enrollment fixture")?;
        let code = totp(setup)?;
        result = h
            .auth
            .execute(
                &result.session_token,
                AuthCommand::ConfirmMfa { code },
                &email,
            )
            .await?;
    }
    Ok(AccountFixture {
        email,
        session: result.session_token,
    })
}

fn scope(state: &AccessState) -> OrganizationScope {
    let org = state
        .organization
        .as_ref()
        .expect("selected fixture company");
    OrganizationScope {
        organization_id: org.id.clone(),
        organization_version: org.version,
        context_version: state.context_version,
    }
}
async fn company(h: &Harness, owner: &AccountFixture, name: &str) -> TestResult<String> {
    let r = h
        .auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Create { name: name.into() },
        )
        .await?;
    let id = r
        .state
        .organizations
        .into_iter()
        .find(|o| o.name == name)
        .ok_or("fixture company missing")?
        .id;
    h.auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Select {
                organization_id: id.clone(),
                expected_context_version: r.state.context_version,
            },
        )
        .await?;
    Ok(id)
}
async fn join(
    h: &Harness,
    owner: &AccountFixture,
    person: &AccountFixture,
    id: &str,
) -> TestResult {
    let state = h.auth.organizations_context(&owner.session).await?.state;
    let org = state
        .active
        .ok_or("fixture company not selected")?
        .organization;
    let invitation = h
        .auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Invite {
                scope: OrganizationScope {
                    organization_id: org.id,
                    organization_version: org.version,
                    context_version: state.context_version,
                },
                email: person.email.clone(),
                role: OrganizationRole::Member,
                valid_for_hours: 24,
                membership_valid_until: None,
            },
        )
        .await?;
    let response = h
        .auth
        .organizations_execute(
            &person.session,
            OrganizationCommand::AcceptInvitation {
                token: invitation
                    .invitation_code
                    .ok_or("fixture invitation missing")?,
            },
        )
        .await?;
    h.auth
        .organizations_execute(
            &person.session,
            OrganizationCommand::Select {
                organization_id: id.into(),
                expected_context_version: response.state.context_version,
            },
        )
        .await?;
    Ok(())
}
async fn fixture_grant(
    h: &Harness,
    owner: &AccountFixture,
    person: &AccountFixture,
    scope_id: &str,
    role: AccessRole,
    actions: Vec<AccessAction>,
) -> TestResult {
    let r = h.auth.access_context(&owner.session).await?;
    h.auth
        .access_execute(
            &owner.session,
            AccessCommand::Grant {
                context: scope(&r.state),
                scope_id: scope_id.into(),
                user_email: person.email.clone(),
                role,
                actions,
                max_amount: "1000.00".parse()?,
                valid_from: "2020-01-01T00:00:00Z".into(),
                valid_until: Some("2090-01-01T00:00:00Z".into()),
                required: false,
            },
        )
        .await?;
    Ok(())
}
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
    Ok(())
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
async fn layout(browser: &Browser, h: &Harness, label: &str, width: u32) -> TestResult<Value> {
    browser.viewport(width, 1000, width == 360).await?;
    ready(browser).await?;
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
    browser.click("#perm-request-submit").await?;
    confirm(&browser).await?;
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
    // Changing the authenticated identity invalidates even a saved draft in the same company.
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
        &json!({"status":"passed","environment":h.environment,"layouts":layouts,"scenarios":["public SSR without private data","explicit work scope","administrator has no implicit proposing power","role grant and cancelled confirmation","threshold policy with segregation","mixed increase and decrease","proponent cannot approve","two independent approvers","reader cannot configure or decide","temporary substitution with lower authority","network failure preserves scoped draft","identity change removes draft and admin controls","no private browser storage"]}),
    )?;
    browser.close().await?;
    reviewer.close().await?;
    Ok(())
}
