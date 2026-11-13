#[allow(dead_code)]
mod support;

use base64::Engine;
use canteiro_contracts::auth::AuthCommand;
use hmac::{Hmac, Mac};
use reqwest::Method;
use serde_json::json;
use sha1::Sha1;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use support::{Browser, ENTER, Harness, TestResult, write_json};

const WAIT: Duration = Duration::from_secs(30);
fn password_fixture() -> TestResult<String> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| "Unable to generate isolated fixture credentials")?;
    Ok(format!(
        "Acesso de teste {}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random)
    ))
}

async fn ready(browser: &Browser) -> TestResult {
    browser.wait("return document.querySelector('[data-auth-ready]')?.dataset.authReady === 'true' && !document.querySelector('.access-progress')?.textContent.trim()", WAIT).await
}

async fn state(browser: &Browser, state: &str) -> TestResult {
    browser.wait(&format!("return document.querySelector('[data-auth-state]')?.dataset.authState === {} && !document.querySelector('.access-progress')?.textContent.trim()", serde_json::to_string(state)?), WAIT).await
}

async fn visible(browser: &Browser, selector: &str) -> TestResult {
    browser.wait(&format!("const e=document.querySelector({});return !!e && e.getClientRects().length>0 && !e.disabled && !document.querySelector('.access-progress')?.textContent.trim();", serde_json::to_string(selector)?), WAIT).await
}

// Secret fixture input never appears in an assertion's failure message.
async fn fill_secret(browser: &Browser, selector: &str, value: &str) -> TestResult {
    browser.click(selector).await?;
    browser.chord(&["\u{e009}"], "a").await?;
    browser.key("\u{e003}").await?;
    let id = browser.element(selector).await?;
    browser
        .post(&format!("element/{id}/value"), json!({"text":value}))
        .await?;
    assert!(
        browser.property(selector, "value").await?.as_str() == Some(value),
        "Secret fixture input did not reach its field"
    );
    Ok(())
}

async fn paste_secret(browser: &Browser, selector: &str, value: &str) -> TestResult {
    // Only the system clipboard is prepared by a browser API. Ctrl+V performs
    // the actual interaction; no field, event handler or product state is set.
    let copied = browser.post("execute/async", json!({
        "script":"const done=arguments[arguments.length-1];navigator.clipboard.writeText(arguments[0]).then(()=>done(true)).catch(()=>done(false));",
        "args":[value]
    })).await?;
    assert_eq!(
        copied, true,
        "Unable to prepare the isolated clipboard fixture"
    );
    browser.click(selector).await?;
    browser.chord(&["\u{e009}"], "a").await?;
    browser.key("\u{e003}").await?;
    browser.chord(&["\u{e009}"], "v").await?;
    let started = Instant::now();
    loop {
        if browser.property(selector, "value").await?.as_str() == Some(value) {
            return Ok(());
        }
        if started.elapsed() > Duration::from_secs(5) {
            return Err("Pasted code did not reach the field".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn delivered_code(h: &Harness, email: &str, kind: &str) -> TestResult<String> {
    let messages = h.auth.pending_mail(25).await?;
    let mut result = None;
    for message in messages {
        if message.email == email && message.kind == kind {
            assert!(
                result.is_none(),
                "Expected a single mail delivery for the requested operation"
            );
            result = Some(message.token.clone());
        }
        h.auth.mark_mail_delivered(&message.id).await?;
    }
    result.ok_or_else(|| "Expected encrypted mail delivery was not available".into())
}

async fn cookie(browser: &Browser) -> TestResult<String> {
    let values = browser.request(Method::GET, "cookie", None).await?;
    values
        .as_array()
        .and_then(|rows| rows.iter().find(|c| c["name"] == "__Host-canteiro-session"))
        .and_then(|c| c["value"].as_str())
        .map(str::to_owned)
        .ok_or_else(|| "Secure session cookie was not present".into())
}

async fn login(browser: &Browser, email: &str, password: &str) -> TestResult {
    visible(browser, "#auth-login-email").await?;
    browser.fill("#auth-login-email", email).await?;
    fill_secret(browser, "#auth-login-password", password).await?;
    browser.tab_to("#auth-login-submit").await?;
    browser.key(ENTER).await?;
    ready(browser).await
}

fn totp(secret: &str) -> TestResult<String> {
    // Independent RFC 6238 reference calculation used only by the test client.
    let bytes = data_encoding::BASE32_NOPAD.decode(secret.as_bytes())?;
    let counter = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&bytes)?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 15);
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into()?) & 0x7fff_ffff;
    Ok(format!("{:06}", value % 1_000_000))
}

async fn enrollment(browser: &Browser, replacing: bool, password: &str) -> TestResult<Vec<String>> {
    if replacing {
        browser.click("#auth-replace-details > summary").await?;
    }
    visible(browser, "#auth-setup-password").await?;
    fill_secret(browser, "#auth-setup-password", password).await?;
    browser.click("#auth-enroll-submit").await?;
    visible(browser, "#auth-setup-key").await?;
    let key = browser
        .property("#auth-setup-key", "value")
        .await?
        .as_str()
        .ok_or("Setup key absent")?
        .to_owned();
    let code = totp(&key)?;
    paste_secret(browser, "#auth-confirm-code", &code).await?;
    browser.tab_to("#auth-confirm-mfa-submit").await?;
    browser.key(ENTER).await?;
    visible(browser, "#auth-recovery-copy").await?;
    state(browser, "authenticated").await?;
    let codes: Vec<String> = browser
        .property("#auth-recovery-copy", "value")
        .await?
        .as_str()
        .ok_or("Backup codes absent")?
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(codes.len(), 10);
    assert!(codes.iter().all(|code| code.len() == 43));
    // Save no screenshot or report while setup material is displayed.
    browser.click("#auth-hide-codes").await?;
    assert_eq!(browser.execute("return document.querySelector('#auth-recovery-copy') === null && document.querySelector('#auth-setup-key') === null;").await?, true);
    Ok(codes)
}

async fn backup(browser: &Browser, code: &str) -> TestResult {
    browser.click("#auth-backup-details > summary").await?;
    paste_secret(browser, "#auth-backup-code", code).await?;
    browser.click("#auth-backup-submit").await?;
    state(browser, "authenticated").await
}

#[tokio::test]
#[ignore = "requires built server, PostgreSQL, encrypted mail fixture and WebDriver"]
async fn account_access_recovers_without_losing_input_and_preserves_mfa() -> TestResult {
    let password = password_fixture()?;
    let replacement_password = password_fixture()?;
    let h = Harness::start("auth-interface").await?;
    let browser = Browser::start(&h).await?;
    browser.cdp("Browser.grantPermissions", json!({"origin":h.app_url,"permissions":["clipboardReadWrite","clipboardSanitizedWrite"]})).await?;
    let email = format!(
        "auth-interface-{}@example.invalid",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    let public_html = h
        .http
        .get(format!("{}/acesso", h.app_url))
        .send()
        .await?
        .text()
        .await?;
    assert!(public_html.contains("Entrar na sua conta") && public_html.contains("method=\"post\""));
    assert!(public_html.contains("autocomplete=\"current-password\""));
    assert!(!public_html.contains(password.as_str()));
    let mut layouts = vec![];
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser.navigate(&format!("{}/acesso", h.app_url)).await?;
        ready(&browser).await?;
        let dimensions = browser.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,fields:[...document.querySelectorAll('input')].map(e=>({id:e.id,label:!!document.querySelector('label[for=\"'+e.id+'\"]'),height:e.getBoundingClientRect().height,autocomplete:e.autocomplete}))};").await?;
        assert!(dimensions["scroll"].as_u64().unwrap() <= dimensions["width"].as_u64().unwrap());
        assert!(
            dimensions["fields"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["label"] == true && f["height"].as_f64().unwrap() >= 44.0)
        );
        browser
            .full_screenshot(&h.evidence.join(format!("auth-login-{width}.png")))
            .await?;
        layouts.push(dimensions);
    }
    assert_eq!(
        browser
            .attribute("#auth-login-email", "autocomplete")
            .await?,
        "username"
    );
    assert_eq!(
        browser
            .attribute("#auth-login-password", "autocomplete")
            .await?,
        "current-password"
    );
    browser.tab_to("#auth-open-register").await?;
    browser.key(ENTER).await?;
    visible(&browser, "#auth-register-email").await?;
    browser.fill("#auth-register-email", &email).await?;
    fill_secret(&browser, "#auth-register-password", password.as_str()).await?;
    browser.tab_to("#auth-register-submit").await?;
    browser.key(ENTER).await?;
    visible(&browser, "#auth-verify-token").await?;
    let verification = delivered_code(&h, &email, "verify_email").await?;
    assert!(
        browser
            .property("#auth-verify-password", "value")
            .await?
            .as_str()
            == Some(password.as_str())
    );
    paste_secret(
        &browser,
        "#auth-verify-token",
        &format!("  {verification}  "),
    )
    .await?;
    browser.click("#auth-verify-submit").await?;
    visible(&browser, "#auth-login-email").await?;

    // A stale pre-login cookie is revoked by a separate authenticated service
    // instance. The browser must preserve inputs and must not replay login.
    browser.fill("#auth-login-email", &email).await?;
    fill_secret(&browser, "#auth-login-password", password.as_str()).await?;
    let stale = cookie(&browser).await?;
    h.auth
        .execute(&stale, AuthCommand::Logout, "auth-browser-expiry-fixture")
        .await?;
    browser.click("#auth-login-submit").await?;
    visible(&browser, "#auth-reconnect").await?;
    assert!(
        browser
            .property("#auth-login-password", "value")
            .await?
            .as_str()
            == Some(password.as_str())
    );
    assert!(
        browser
            .property("#auth-login-email", "value")
            .await?
            .as_str()
            == Some(email.as_str())
    );
    browser.click("#auth-reconnect").await?;
    ready(&browser).await?;
    state(&browser, "anonymous").await?;
    assert!(
        browser
            .property("#auth-login-password", "value")
            .await?
            .as_str()
            == Some(password.as_str())
    );
    browser.tab_to("#auth-login-submit").await?;
    browser.key(ENTER).await?;
    state(&browser, "authenticated").await?;
    assert_eq!(browser.text("#auth-account-email").await?, email);

    let other = Browser::start(&h).await?;
    other.navigate(&format!("{}/acesso", h.app_url)).await?;
    ready(&other).await?;
    login(&other, &email, password.as_str()).await?;
    state(&other, "authenticated").await?;
    browser.click("#auth-refresh").await?;
    browser
        .wait(
            "return document.querySelectorAll('#auth-sessions > li').length === 2;",
            WAIT,
        )
        .await?;
    browser
        .click("#auth-sessions button[aria-label='Encerrar outra sessão']")
        .await?;
    browser
        .wait(
            "return document.querySelectorAll('#auth-sessions > li').length === 1;",
            WAIT,
        )
        .await?;
    other.click("#auth-logout").await?;
    visible(&other, "#auth-reconnect").await?;
    other.click("#auth-reconnect").await?;
    state(&other, "anonymous").await?;
    other.close().await?;

    let old_backups = enrollment(&browser, false, &password).await?;
    browser.click("#auth-logout").await?;
    state(&browser, "anonymous").await?;
    login(&browser, &email, password.as_str()).await?;
    state(&browser, "mfa_pending").await?;
    backup(&browser, &old_backups[0]).await?;
    let new_backups = enrollment(&browser, true, &password).await?;
    assert!(new_backups != old_backups);
    browser.click("#auth-logout").await?;
    state(&browser, "anonymous").await?;
    login(&browser, &email, password.as_str()).await?;
    state(&browser, "mfa_pending").await?;
    browser.click("#auth-backup-details > summary").await?;
    paste_secret(&browser, "#auth-backup-code", &old_backups[1]).await?;
    browser.click("#auth-backup-submit").await?;
    browser.wait("return document.querySelector('#auth-feedback')?.classList.contains('danger') && !document.querySelector('.access-progress')?.textContent.trim();", WAIT).await?;
    state(&browser, "mfa_pending").await?;
    assert!(
        browser
            .property("#auth-backup-code", "value")
            .await?
            .as_str()
            == Some(old_backups[1].as_str())
    );
    paste_secret(&browser, "#auth-backup-code", &new_backups[0]).await?;
    browser.click("#auth-backup-submit").await?;
    state(&browser, "authenticated").await?;

    // Recovery happens in a separate browser session, as it would after losing
    // an original session. A password reset must revoke that original session.
    let recovery = Browser::start(&h).await?;
    recovery.cdp("Browser.grantPermissions", json!({"origin":h.app_url,"permissions":["clipboardReadWrite","clipboardSanitizedWrite"]})).await?;
    recovery.navigate(&format!("{}/acesso", h.app_url)).await?;
    ready(&recovery).await?;
    recovery.click("#auth-open-recovery").await?;
    recovery.fill("#auth-recovery-email", &email).await?;
    recovery.click("#auth-recovery-submit").await?;
    visible(&recovery, "#auth-reset-token").await?;
    let reset = delivered_code(&h, &email, "reset_password").await?;
    paste_secret(&recovery, "#auth-reset-token", &format!(" {reset} ")).await?;
    fill_secret(
        &recovery,
        "#auth-reset-password",
        replacement_password.as_str(),
    )
    .await?;
    recovery.click("#auth-reset-submit").await?;
    visible(&recovery, "#auth-login-email").await?;
    state(&recovery, "anonymous").await?;
    recovery.close().await?;
    browser.click("#auth-refresh").await?;
    state(&browser, "anonymous").await?;
    login(&browser, &email, password.as_str()).await?;
    browser.wait("return document.querySelector('#auth-feedback')?.classList.contains('danger') && !document.querySelector('.access-progress')?.textContent.trim();", WAIT).await?;
    state(&browser, "anonymous").await?;
    assert!(
        browser
            .property("#auth-login-password", "value")
            .await?
            .as_str()
            == Some(password.as_str())
    );
    login(&browser, &email, replacement_password.as_str()).await?;
    state(&browser, "mfa_pending").await?;
    backup(&browser, &new_backups[1]).await?;
    state(&browser, "authenticated").await?;

    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        let dimensions = browser.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,hasSecrets:!!document.querySelector('#auth-setup-key,#auth-recovery-copy')};").await?;
        assert!(dimensions["scroll"].as_u64().unwrap() <= dimensions["width"].as_u64().unwrap());
        assert_eq!(dimensions["hasSecrets"], false);
        browser
            .full_screenshot(&h.evidence.join(format!("auth-account-{width}.png")))
            .await?;
        layouts.push(dimensions);
    }
    browser.click("#auth-revoke-all-details > summary").await?;
    browser.tab_to("#auth-revoke-all").await?;
    browser.key(ENTER).await?;
    state(&browser, "anonymous").await?;
    let final_html = h
        .http
        .get(format!("{}/acesso", h.app_url))
        .send()
        .await?
        .text()
        .await?;
    let console = browser.logs().await?;
    let logs = serde_json::to_string(&console)?;
    for secret in [
        password.as_str(),
        replacement_password.as_str(),
        verification.as_str(),
        reset.as_str(),
        old_backups[0].as_str(),
        new_backups[0].as_str(),
    ] {
        assert!(
            !final_html.contains(secret),
            "A private fixture value reached SSR"
        );
        assert!(
            !logs.contains(secret),
            "A private fixture value reached browser logs"
        );
    }
    assert_eq!(
        browser
            .execute("return {local:localStorage.length,session:sessionStorage.length};")
            .await?,
        json!({"local":0,"session":0})
    );
    write_json(
        &h.evidence.join("auth-interface.json"),
        &json!({
            "status":"passed","versions":browser.versions,"environment":h.environment,"layouts":layouts,
            "checks":["SSR without credentials","mobile and desktop layout","keyboard submission","password-manager autocomplete metadata","native clipboard paste of long codes","email confirmation with owner-selected password","expired session preserves fields","reconnect does not replay login","session list and remote revocation","authenticator enrollment","backup-code access","authenticator replacement keeps protection","old backup codes rejected after replacement","password reset revokes original session","reset preserves MFA","revoke all sessions","no credentials in HTML/browser logs/localStorage/sessionStorage"],
            "console":console,
            "note":"Negative HTTP responses are expected in the deliberately invalid credential, revoked session and obsolete recovery-code scenarios. Test credentials and codes are omitted from reports and screenshots."
        }),
    )?;
    browser.close().await?;
    Ok(())
}
