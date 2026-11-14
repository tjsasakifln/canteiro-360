#[allow(dead_code)]
mod support;

use base64::Engine;
use canteiro_contracts::auth::AuthCommand;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha1::Sha1;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use support::{Browser, ENTER, ESCAPE, Harness, SHIFT, TAB, TestResult, write_json};

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
    let email = format!("org-{label}-{}@example.invalid", nonce.to_lowercase());
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

async fn ready(browser: &Browser) -> TestResult {
    browser.wait("return document.querySelector('[data-org-ready]')?.dataset.orgReady === 'true' && document.querySelector('[data-org-busy]')?.dataset.orgBusy === 'false';", WAIT).await
}

async fn idle(browser: &Browser) -> TestResult {
    browser
        .wait(
            "return document.querySelector('[data-org-busy]')?.dataset.orgBusy === 'false';",
            WAIT,
        )
        .await
}

async fn enter(h: &Harness, browser: &Browser, account: &AccountFixture) -> TestResult {
    browser.navigate(&h.app_url).await?;
    browser
        .post(
            "cookie",
            json!({"cookie":{
                "name":"__Host-canteiro-session","value":account.session,
                "path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"
            }}),
        )
        .await?;
    browser
        .navigate(&format!("{}/organizacoes", h.app_url))
        .await?;
    ready(browser).await
}

async fn company_id(browser: &Browser, name: &str) -> TestResult<String> {
    browser.execute(&format!("return [...document.querySelectorAll('#org-selection option')].find(o=>o.textContent === {})?.value;", serde_json::to_string(name)?)).await?.as_str().map(str::to_owned).ok_or_else(|| "Company not available in authorized selector".into())
}

async fn select_company(browser: &Browser, id: &str) -> TestResult {
    browser
        .click(&format!("#org-selection option[value='{id}']"))
        .await?;
    browser.click("#org-select-submit").await?;
    Ok(())
}

async fn dialog(browser: &Browser) -> TestResult {
    browser
        .wait(
            "return document.querySelector('#org-confirm-dialog')?.open === true;",
            WAIT,
        )
        .await
}

async fn confirm(browser: &Browser) -> TestResult {
    dialog(browser).await?;
    browser.tab_to("#org-confirm-submit").await?;
    browser.key(ENTER).await?;
    idle(browser).await
}

async fn read_code(browser: &Browser, selector: &str) -> TestResult<String> {
    browser
        .wait(
            &format!(
                "return !!document.querySelector({})?.getClientRects().length;",
                serde_json::to_string(selector)?
            ),
            WAIT,
        )
        .await?;
    let value = browser.property(selector, "value").await?;
    let text = value.as_str().ok_or("Expected transient code")?;
    assert!(text.len() > 50 && text.len() <= 100, "Invalid code shape");
    Ok(text.to_owned())
}

async fn secret_input(browser: &Browser, selector: &str, value: &str) -> TestResult {
    browser.click(selector).await?;
    browser.chord(&["\u{e009}"], "a").await?;
    browser.key("\u{e003}").await?;
    let id = browser.element(selector).await?;
    browser
        .post(&format!("element/{id}/value"), json!({"text":value}))
        .await?;
    assert!(
        browser.property(selector, "value").await?.as_str() == Some(value),
        "Code did not reach its field"
    );
    Ok(())
}

async fn paste_code(browser: &Browser, selector: &str, value: &str) -> TestResult {
    // Prepare only the clipboard. Ctrl+V performs the actual product input.
    let copied = browser.post("execute/async", json!({
        "script":"const done=arguments[arguments.length-1];navigator.clipboard.writeText(arguments[0]).then(()=>done(true)).catch(()=>done(false));",
        "args":[value]
    })).await?;
    assert_eq!(copied, true, "Clipboard fixture unavailable");
    browser.click(selector).await?;
    browser.chord(&["\u{e009}"], "v").await?;
    // No failure assertion includes the transient value.
    for _ in 0..100 {
        if browser.property(selector, "value").await?.as_str() == Some(value) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    Err("Pasted code did not reach the field".into())
}

async fn layout(browser: &Browser, h: &Harness, name: &str, width: u32) -> TestResult<Value> {
    browser.viewport(width, 1000, width == 360).await?;
    ready(browser).await?;
    browser.tab_to("#org-selection").await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "org-selection"
    );
    let metrics=browser.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,labels:[...document.querySelectorAll('.organizations-page input,.organizations-page select,.organizations-page textarea')].filter(e=>e.getClientRects().length).map(e=>({id:e.id,label:!!document.querySelector('label[for=\"'+e.id+'\"]'),height:e.getBoundingClientRect().height})),privateCodes:!!document.querySelector('#org-invitation-code,#org-access-code')};").await?;
    assert!(
        metrics["scroll"].as_u64().unwrap() <= metrics["width"].as_u64().unwrap(),
        "Horizontal page overflow"
    );
    assert!(
        metrics["labels"]
            .as_array()
            .unwrap()
            .iter()
            .all(|field| field["label"] == true && field["height"].as_f64().unwrap() >= 44.0),
        "Missing label or small target"
    );
    assert_eq!(
        metrics["privateCodes"], false,
        "Do not capture transient codes"
    );
    browser
        .full_screenshot(&h.evidence.join(format!("organizations-{name}-{width}.png")))
        .await?;
    Ok(metrics)
}

#[tokio::test]
#[ignore = "requires built SSR/WASM server, PostgreSQL and WebDriver"]
async fn organization_membership_context_and_invitations_work_through_the_browser() -> TestResult {
    let h = Harness::start("organizations-interface").await?;
    let owner = account(&h, "owner", true).await?;
    let colleague = account(&h, "member", false).await?;
    let browser = Browser::start(&h).await?;
    let member = Browser::start(&h).await?;
    let suffix = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let company_a = format!("Aurora Equipe {suffix}");
    let company_b = format!("Horizonte Equipe {suffix}");
    let html = h
        .http
        .get(format!("{}/organizacoes", h.app_url))
        .send()
        .await?
        .text()
        .await?;
    assert!(html.contains("Empresas e acesso"));
    assert!(!html.contains(&owner.email) && !html.contains(&colleague.email));
    enter(&h, &browser, &owner).await?;
    browser.fill("#org-create-name", &company_a).await?;
    browser.tab_to("#org-create-submit").await?;
    browser.key(ENTER).await?;
    ready(&browser).await?;
    let _access_a = read_code(&browser, "#org-access-code").await?;
    browser.click("#org-hide-access-code").await?;
    assert_eq!(
        browser.text("#org-active-name").await?,
        "Escolha uma empresa"
    );
    let id_a = company_id(&browser, &company_a).await?;
    select_company(&browser, &id_a).await?;
    ready(&browser).await?;
    assert_eq!(browser.text("#org-active-name").await?, company_a);

    browser.fill("#org-create-name", &company_b).await?;
    browser.click("#org-create-submit").await?;
    ready(&browser).await?;
    let access_b = read_code(&browser, "#org-access-code").await?;
    browser.click("#org-hide-access-code").await?;
    let id_b = company_id(&browser, &company_b).await?;
    assert_eq!(browser.text("#org-active-name").await?, company_a);

    browser.fill("#org-invite-email", &colleague.email).await?;
    browser.click("#org-invite-submit").await?;
    ready(&browser).await?;
    let invitation = read_code(&browser, "#org-invitation-code").await?;
    browser.click("#org-hide-invitation-code").await?;
    enter(&h, &member, &colleague).await?;
    member.cdp("Browser.grantPermissions", json!({"origin":h.app_url,"permissions":["clipboardReadWrite","clipboardSanitizedWrite"]})).await?;
    assert!(
        member
            .execute("return document.querySelector('#org-create-submit').disabled;")
            .await?
            == true
    );
    paste_code(&member, "#org-invitation-token", &invitation).await?;
    member.click("#org-preview-invitation").await?;
    ready(&member).await?;
    assert_eq!(member.text("#org-preview-title").await?, company_a);
    member.tab_to("#org-accept-invitation").await?;
    member.key(ENTER).await?;
    ready(&member).await?;
    select_company(&member, &id_a).await?;
    ready(&member).await?;
    assert_eq!(member.text("#org-active-name").await?, company_a);
    assert_eq!(member.execute("return document.querySelector('#org-invite-heading') === null && document.querySelectorAll('#org-member-list > li').length === 1;").await?,true);
    assert!(!member.text("#org-selection").await?.contains(&company_b));

    browser.click("#org-refresh").await?;
    ready(&browser).await?;
    browser
        .wait(
            "return document.querySelectorAll('#org-member-list > li').length === 2;",
            WAIT,
        )
        .await?;
    let mut layouts = vec![];
    for width in [360, 1280] {
        layouts.push(layout(&browser, &h, "administrator", width).await?);
        layouts.push(layout(&member, &h, "member", width).await?);
    }

    let own_remove=browser.execute("return [...document.querySelectorAll('#org-member-list > li')].find(row=>row.textContent.includes('(você)'))?.querySelector('button[id^=org-member-remove]')?.id;").await?.as_str().ok_or("Own membership action missing")?.to_owned();
    browser.click(&format!("#{own_remove}")).await?;
    confirm(&browser).await?;
    assert!(browser.execute("return document.querySelector('#org-feedback').classList.contains('danger') && document.querySelectorAll('#org-member-list > li').length === 2;").await?==true);

    let draft = "rascunho-preservado@example.invalid";
    browser.fill("#org-invite-email", draft).await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":true,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#org-invite-submit").await?;
    idle(&browser).await?;
    assert_eq!(browser.property("#org-invite-email", "value").await?, draft);
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#org-refresh").await?;
    ready(&browser).await?;
    assert_eq!(browser.property("#org-invite-email", "value").await?, draft);

    select_company(&browser, &id_b).await?;
    dialog(&browser).await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "org-confirm-cancel"
    );
    browser.key(TAB).await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "org-confirm-submit"
    );
    browser.key(TAB).await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "org-confirm-cancel"
    );
    browser.chord(&[SHIFT], TAB).await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "org-confirm-submit"
    );
    browser.key(ESCAPE).await?;
    browser
        .wait(
            "return document.querySelector('#org-confirm-dialog')?.open === false;",
            WAIT,
        )
        .await?;
    assert_eq!(browser.text("#org-active-name").await?, company_a);
    assert_eq!(browser.property("#org-invite-email", "value").await?, draft);
    browser.click("#org-select-submit").await?;
    confirm(&browser).await?;
    ready(&browser).await?;
    assert_eq!(browser.text("#org-active-name").await?, company_b);
    assert_eq!(browser.property("#org-invite-email", "value").await?, "");
    assert!(
        !browser
            .text("#org-member-list")
            .await?
            .contains(&colleague.email)
    );

    // Delay only the old company's search response. The selection command stays
    // unthrottled. Observe actual completion order before asserting isolation.
    select_company(&browser, &id_a).await?;
    ready(&browser).await?;
    browser.fill("#org-member-search", &colleague.email).await?;
    browser
        .cdp(
            "Network.emulateNetworkConditionsByRule",
            json!({"offline":false,"matchedNetworkConditions":[{
                "urlPattern":format!("{}/api/organizations/{id_a}/members?*",h.app_url),
                "latency":4000,"downloadThroughput":-1,"uploadThroughput":-1
            }]}),
        )
        .await?;
    let started = browser
        .execute("return performance.now();")
        .await?
        .as_f64()
        .ok_or("Clock unavailable")?;
    browser.click("#org-member-search-submit").await?;
    select_company(&browser, &id_b).await?;
    ready(&browser).await?;
    let switched = browser
        .execute("return performance.now();")
        .await?
        .as_f64()
        .ok_or("Clock unavailable")?;
    browser.wait(&format!("return performance.getEntriesByType('resource').some(e=>e.name.includes('/organizations/{id_a}/members?') && e.startTime>{started} && e.responseEnd>{switched});"),WAIT).await?;
    assert_eq!(browser.text("#org-active-name").await?, company_b);
    assert!(
        !browser
            .text("#org-member-list")
            .await?
            .contains(&colleague.email)
    );
    assert_eq!(browser.property("#org-member-search", "value").await?, "");
    let late=browser.execute(&format!("return performance.getEntriesByType('resource').filter(e=>e.name.includes('/organizations/{id_a}/members?')&&e.startTime>{started}).map(e=>({{start:e.startTime,end:e.responseEnd}}));")).await?;
    browser
        .cdp(
            "Network.emulateNetworkConditionsByRule",
            json!({"offline":false,"matchedNetworkConditions":[]}),
        )
        .await?;

    secret_input(&member, "#org-request-code", &access_b).await?;
    member.click("#org-request-submit").await?;
    ready(&member).await?;
    browser.click("#org-refresh").await?;
    ready(&browser).await?;
    browser
        .wait(
            "return !!document.querySelector('button[id^=org-approve-request]');",
            WAIT,
        )
        .await?;
    browser.click("button[id^=org-approve-request]").await?;
    confirm(&browser).await?;
    ready(&browser).await?;
    member.click("#org-refresh").await?;
    ready(&member).await?;
    assert_eq!(company_id(&member, &company_b).await?, id_b);
    select_company(&member, &id_b).await?;
    ready(&member).await?;
    let remove_colleague=browser.execute(&format!("return [...document.querySelectorAll('#org-member-list > li')].find(row=>row.textContent.includes({}))?.querySelector('button[id^=org-member-remove]')?.id;",serde_json::to_string(&colleague.email)?)).await?.as_str().ok_or("Colleague membership action missing")?.to_owned();
    browser.click(&format!("#{remove_colleague}")).await?;
    confirm(&browser).await?;
    ready(&browser).await?;
    member.click("#org-refresh").await?;
    ready(&member).await?;
    assert_eq!(
        member.text("#org-active-name").await?,
        "Escolha uma empresa"
    );
    assert!(!member.text("#org-selection").await?.contains(&company_b));

    // An old administrator response must not reappear after login changes in
    // another tab. Fixture cookies arrange the identity change; WebDriver tab
    // switching triggers the same focus event used by normal account access.
    let original_handle = browser
        .request(reqwest::Method::GET, "window", None)
        .await?
        .as_str()
        .ok_or("Window handle unavailable")?
        .to_owned();
    let tab = browser.post("window/new", json!({"type":"tab"})).await?;
    let other_handle = tab["handle"].as_str().ok_or("New tab handle unavailable")?;
    browser
        .post("window", json!({"handle":other_handle}))
        .await?;
    browser.navigate(&h.app_url).await?;
    browser.cdp("Page.bringToFront", json!({})).await?;
    browser
        .post("window", json!({"handle":original_handle}))
        .await?;
    browser.cdp("Page.bringToFront", json!({})).await?;
    ready(&browser).await?;
    browser
        .fill("#org-invite-email", "late-response@example.invalid")
        .await?;
    browser
        .cdp(
            "Network.emulateNetworkConditionsByRule",
            json!({"offline":false,"matchedNetworkConditions":[{
                "urlPattern":format!("{}/api/organizations/command",h.app_url),
                "latency":10000,"downloadThroughput":-1,"uploadThroughput":-1
            }]}),
        )
        .await?;
    let command_started = browser
        .execute("return performance.now();")
        .await?
        .as_f64()
        .ok_or("Clock unavailable")?;
    // Observation only: record actual browser lifecycle events, never dispatch
    // synthetic focus or visibility events to make the product respond.
    browser.execute("window.__orgLifecycle=[];for(const name of ['focus','blur','pageshow','pagehide'])window.addEventListener(name,()=>window.__orgLifecycle.push({name,time:performance.now(),visible:document.visibilityState}));document.addEventListener('visibilitychange',()=>window.__orgLifecycle.push({name:'visibilitychange',time:performance.now(),visible:document.visibilityState}));return true;").await?;
    browser.click("#org-invite-submit").await?;
    browser
        .wait(
            "return document.querySelector('[data-org-busy]')?.dataset.orgBusy === 'true';",
            WAIT,
        )
        .await?;
    browser
        .post("window", json!({"handle":other_handle}))
        .await?;
    browser.cdp("Page.bringToFront", json!({})).await?;
    browser
        .post(
            "cookie",
            json!({"cookie":{
                "name":"__Host-canteiro-session","value":colleague.session,
                "path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"
            }}),
        )
        .await?;
    browser
        .post("window", json!({"handle":original_handle}))
        .await?;
    browser.cdp("Page.bringToFront", json!({})).await?;
    if let Err(error) = browser.wait("return document.querySelector('[data-org-ready]')?.dataset.orgReady === 'false' && !document.querySelector('#org-invite-heading');",WAIT).await {
        let observed = browser.execute("return {now:performance.now(),ready:document.querySelector('[data-org-ready]')?.dataset.orgReady,busy:document.querySelector('[data-org-busy]')?.dataset.orgBusy,admin:!!document.querySelector('#org-invite-heading'),visible:document.visibilityState,focused:document.hasFocus(),events:window.__orgLifecycle,resources:performance.getEntriesByType('resource').filter(e=>e.name.endsWith('/api/organizations/command')||e.name.endsWith('/api/organizations/context')).map(e=>({kind:e.name.endsWith('/context')?'context':'command',start:e.startTime,end:e.responseEnd}))};").await?;
        write_json(&h.evidence.join("organizations-lifecycle-failure.json"), &observed)?;
        return Err(error);
    }
    ready(&browser).await?;
    let reauthorized = browser
        .execute("return performance.now();")
        .await?
        .as_f64()
        .ok_or("Clock unavailable")?;
    let delayed_command=browser.execute(&format!("return performance.getEntriesByType('resource').filter(e=>e.name.endsWith('/api/organizations/command')&&e.startTime>{command_started}).map(e=>({{start:e.startTime,end:e.responseEnd}}));")).await?;
    assert!(
        delayed_command
            .as_array()
            .is_some_and(|rows| rows.len() == 1
                && rows[0]["end"]
                    .as_f64()
                    .is_some_and(|end| end > command_started + 9000.0 && end <= reauthorized)),
        "The pending command did not finish before context revalidation"
    );
    assert_eq!(browser.execute("return document.querySelector('#org-create-submit').disabled && !document.querySelector('#org-invite-heading') && !document.querySelector('#org-invitation-code');").await?,true);
    assert!(!browser.text("#org-selection").await?.contains(&company_b));
    assert!(
        !browser
            .text(".organizations-page")
            .await?
            .contains(&owner.email)
    );
    browser
        .cdp(
            "Network.emulateNetworkConditionsByRule",
            json!({"offline":false,"matchedNetworkConditions":[]}),
        )
        .await?;

    let storage=browser.execute("return {local:Object.keys(localStorage),session:Object.keys(sessionStorage),url:location.pathname+location.search+location.hash};").await?;
    assert!(
        storage["local"].as_array().unwrap().is_empty()
            && storage["session"].as_array().unwrap().is_empty()
    );
    assert_eq!(storage["url"], "/organizacoes");
    let logs = browser.logs().await?;
    assert!(
        !logs.to_string().contains("panicked") && !logs.to_string().contains("Hydration"),
        "Browser runtime error"
    );
    write_json(
        &h.evidence.join("organizations-interface.json"),
        &json!({
            "status":"passed","environment":h.environment,"layouts":layouts,
            "delayedSearch":late,"selectionConfirmedAt":switched,
        "delayedCommand":delayed_command,"identityRevalidatedAt":reauthorized,
        "scenarios":["public SSR without private data","explicit company selection","MFA administration","invitation preview and acceptance","member scope","last administrator refused","network error retains email","dialog Escape and keyboard confirmation","context change clears drafts","late search cannot restore old company","access request and decision","membership removal applies on refresh","account change while old mutation response is delayed","no private browser storage"]
        }),
    )?;
    browser.close().await?;
    member.close().await?;
    Ok(())
}
