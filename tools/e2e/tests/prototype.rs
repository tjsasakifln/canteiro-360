//! Issue 3 acceptance observes the real Rust/Leptos application in Chromium.
//! Script strings inspect DOM, styles and accessibility semantics only. User
//! operations use WebDriver keys/clicks, including native selects and dialogs.
//! This suite is not a substitute for the required three professional users.

mod support;

use serde_json::{Value, json};
use std::{fs, sync::Arc, time::Duration};
use support::{
    ALT, Browser, DOWN, ENTER, ESCAPE, HOME, Harness, SHIFT, TAB, TestResult, write_json,
};

const READY: &str = "return document.querySelector('#prototype-shell')?.dataset.ready === 'true';";
const ROUTES: [&str; 5] = ["painel", "orcamento", "medicao", "aprovacoes", "documentos"];
const SHORT_NOTE: &str = "Viga 3";
const DRAFT_NOTE: &str = "Conferido em campo: amostra A-17, três vigas.";

#[tokio::test]
#[ignore = "Requires release SSR/WASM, real PostgreSQL, Chromium and matching ChromeDriver"]
async fn prototype_accessibility_context_and_recovery() -> TestResult {
    let h = Arc::new(Harness::start("prototype-functional").await?);
    let browser = Arc::new(Browser::start(&h).await?);
    let task_h = Arc::clone(&h);
    let task_browser = Arc::clone(&browser);
    // Keep the live browser and its process guards here while an assertion runs
    // in a child task. A panic must fail the suite, but must not erase its proof.
    let result: TestResult =
        match tokio::spawn(async move { acceptance(&task_h, &task_browser).await }).await {
            Ok(result) => result,
            Err(error) => Err(format!("Prototype acceptance task failed: {error}").into()),
        };
    if let Err(error) = &result {
        let screenshot_error = browser
            .screenshot(&h.evidence.join("prototype-failure.png"))
            .await
            .err()
            .map(|error| error.to_string());
        let snapshot = browser.execute("return {url:location.href,title:document.title,focus:{tag:document.activeElement?.tagName,id:document.activeElement?.id},ready:document.querySelector('#prototype-shell')?.dataset.ready,openDialogs:[...document.querySelectorAll('dialog[open]')].map(e=>e.id),fields:[...document.querySelectorAll('input,select,textarea')].map(e=>({id:e.id,value:e.value})),visibleText:document.body?.innerText.slice(0,10000)};").await;
        let console = browser.logs().await;
        let html_error = match browser
            .execute("return document.documentElement.outerHTML;")
            .await
        {
            Ok(html) => match html.as_str() {
                Some(html) => fs::write(h.evidence.join("prototype-failure.html"), html)
                    .err()
                    .map(|error| error.to_string()),
                None => Some("DOM snapshot was not a string".into()),
            },
            Err(error) => Some(error.to_string()),
        };
        write_json(
            &h.evidence.join("prototype-functional.json"),
            &json!({
                "status":"failed","error":error.to_string(),"versions":browser.versions,
                "snapshot":snapshot.as_ref().ok(),"snapshotError":snapshot.as_ref().err().map(ToString::to_string),
                "console":console.as_ref().ok(),"consoleError":console.as_ref().err().map(ToString::to_string),
                "screenshotError":screenshot_error,"htmlError":html_error
            }),
        )?;
    }
    let closing = browser.close().await;
    result?;
    closing?;
    Ok(())
}

async fn acceptance(h: &Harness, browser: &Browser) -> TestResult {
    server_rendering(h, browser).await?;
    let viewports = reflow_and_semantics(h, browser).await?;
    keyboard_and_filter(h, browser).await?;
    display_states(h, browser).await?;
    reduced_motion_and_density(h, browser).await?;
    draft_and_context(h, browser).await?;
    browser_history_context_guard(h, browser).await?;
    approval_dialog(h, browser).await?;
    let console = browser.logs().await?;
    for entry in console.as_array().ok_or("Browser logs must be an array")? {
        assert_ne!(
            entry["level"], "SEVERE",
            "Unexpected browser error: {entry}"
        );
        let message = entry["message"].as_str().unwrap_or_default().to_lowercase();
        assert!(
            !message.contains("hydration mismatch")
                && !message.contains("hydration error")
                && !message.contains("panicked at"),
            "Hydration or runtime error: {entry}"
        );
    }
    write_json(
        &h.evidence.join("prototype-functional.json"),
        &json!({
            "status":"passed",
            "versions":browser.versions,
            "environment":h.environment,
            "checks":[
                "direct_ssr_all_routes_and_two_companies", "useful_navigation_without_scripts",
                "five_routes_reflow_320_360_1280", "explicit_control_labels", "keyboard_navigation_without_forced_sequence",
                "filter_detail_and_return_preserve_query", "loading_empty_error_forbidden_states", "state_recovery_by_keyboard", "computed_text_contrast_actual_dom",
                "forbidden_removes_business_dom", "invalid_draft_preserves_note_and_period",
                "cancel_context_keeps_draft", "confirm_context_discards_only_with_consent",
                "cross_company_context_does_not_retain_previous_draft", "back_history_context_cancel_and_confirm", "approval_native_modal_focus_trap",
                "escape_restores_trigger", "keyboard_confirmation_explicitly_simulated",
                "shortcut_announced_and_operable", "reduced_motion", "density_control"
            ],
            "viewports":viewports,
            "console":console,
            "human_professionals":"Not executed. Three real professional participants remain a separate acceptance criterion.",
            "screen_reader":"Not executed. This suite verifies semantic DOM and keyboard interaction."
        }),
    )?;
    println!(
        "Prototype acceptance passed; evidence: {}",
        h.evidence.display()
    );
    Ok(())
}

async fn open(h: &Harness, browser: &Browser, route: &str) -> TestResult {
    browser
        .navigate(&format!("{}/prototipo/aurora/{route}", h.app_url))
        .await?;
    browser.wait(READY, Duration::from_secs(20)).await
}

async fn server_rendering(h: &Harness, browser: &Browser) -> TestResult {
    for company in ["aurora", "horizonte"] {
        for route in ROUTES {
            let response = h
                .http
                .get(format!("{}/prototipo/{company}/{route}", h.app_url))
                .send()
                .await?;
            assert_eq!(response.status(), 200, "Deep route {company}/{route}");
            let html = response.text().await?;
            assert!(html.contains("id=\"prototype-shell\""));
            assert!(html.contains("aria-current=\"page\""));
            assert!(html.contains("CT-014/2026") == (company == "aurora"));
            assert!(html.contains("CT-027/2026") == (company == "horizonte"));
            assert!(html.contains("id=\"nav-documentos\""));
            assert!(html.contains("id=\"organization\""));
        }
    }
    for path in [
        "/prototipo/desconhecida/painel",
        "/prototipo/aurora/tarefa-inexistente",
    ] {
        let response = h.http.get(format!("{}{path}", h.app_url)).send().await?;
        assert_eq!(
            response.status(),
            404,
            "Unknown prototype context must not receive a success page"
        );
        let html = response.text().await?;
        assert!(!html.contains("CT-014/2026") && !html.contains("CT-027/2026"));
        assert!(!html.contains("id=\"organization\""));
    }
    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value":true}),
        )
        .await?;
    browser
        .navigate(&format!("{}/prototipo/aurora/painel", h.app_url))
        .await?;
    assert!(!browser.text("h1").await?.trim().is_empty());
    assert!(!browser.enabled("#organization").await?);
    assert!(browser.text("main").await?.contains("CT-014/2026"));
    browser.tab_to("#nav-documentos").await?;
    browser.key(ENTER).await?;
    assert_eq!(
        browser.attribute("#nav-documentos", "aria-current").await?,
        "page",
        "Documents must be reachable before walking preceding menus"
    );
    browser
        .full_screenshot(&h.evidence.join("prototype-ssr-no-scripts.png"))
        .await?;
    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value":false}),
        )
        .await?;
    Ok(())
}

async fn reflow_and_semantics(h: &Harness, browser: &Browser) -> TestResult<Vec<Value>> {
    let mut evidence = Vec::new();
    for width in [320, 360, 1280] {
        browser.viewport(width, 850, false).await?;
        for route in ROUTES {
            open(h, browser, route).await?;
            let metrics = browser.execute(
                "const controls=[...document.querySelectorAll('main input:not([type=hidden]),main textarea,main select')];return {viewport:innerWidth,document:document.documentElement.scrollWidth,body:document.body.scrollWidth,h1:document.querySelectorAll('h1').length,current:document.querySelectorAll('[aria-current=page]').length,unlabelled:controls.filter(e=>!e.labels?.length&&!e.getAttribute('aria-label')&&!e.getAttribute('aria-labelledby')).map(e=>e.id),heading:document.querySelector('h1')?.innerText};",
            ).await?;
            assert_eq!(metrics["viewport"], width);
            assert!(
                number(&metrics, "document")? <= f64::from(width) + 1.0,
                "Page overflow at {width}px, {route}: {metrics}"
            );
            assert!(
                number(&metrics, "body")? <= f64::from(width) + 1.0,
                "Body overflow at {width}px, {route}: {metrics}"
            );
            assert_eq!(metrics["h1"], 1, "One main title per screen");
            assert_eq!(metrics["current"], 1, "Exactly one current task");
            assert_eq!(
                metrics["unlabelled"],
                json!([]),
                "Every field needs an accessible name"
            );
            assert_eq!(
                browser
                    .attribute(&format!("#nav-{route}"), "aria-current")
                    .await?,
                "page"
            );
            assert_eq!(browser.property("#organization", "value").await?, "aurora");
            check_text_contrast(h, browser, &format!("{route}-{width}"), "#prototype-shell")
                .await?;
            browser
                .full_screenshot(&h.evidence.join(format!("prototype-{route}-{width}.png")))
                .await?;
            evidence.push(json!({"route":route,"width":width,"metrics":metrics}));
        }
    }
    Ok(evidence)
}

async fn keyboard_and_filter(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "orcamento").await?;
    browser.tab_to("#budget-filter").await?;
    browser.fill("#budget-filter", "concreto").await?;
    browser.tab_to("#budget-submit").await?;
    assert_visible_focus(browser).await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return new URL(location.href).searchParams.get('busca') === 'concreto';",
            Duration::from_secs(5),
        )
        .await?;
    browser.wait(READY, Duration::from_secs(20)).await?;
    assert_eq!(
        browser.property("#budget-filter", "value").await?,
        "concreto"
    );
    browser.tab_to("a[href*='item=01']").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return new URL(location.href).searchParams.get('item') === '01';",
            Duration::from_secs(5),
        )
        .await?;
    assert!(
        browser
            .text("main")
            .await?
            .contains("Concreto para estrutura")
    );
    browser.tab_to("#back-to-budget").await?;
    browser.key(ENTER).await?;
    browser.wait("return !new URL(location.href).searchParams.has('item') && new URL(location.href).searchParams.get('busca') === 'concreto';", Duration::from_secs(5)).await?;
    assert_eq!(
        browser.property("#budget-filter", "value").await?,
        "concreto"
    );
    browser.tab_to("#nav-documentos").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#nav-documentos')?.getAttribute('aria-current') === 'page';", Duration::from_secs(5)).await?;
    assert!(browser.text("main").await?.contains("CT-014/2026"));
    browser.ensure_options().await?;
    browser.tab_to("#shortcuts-toggle").await?;
    browser.key(ENTER).await?;
    assert_eq!(
        browser
            .execute(
                "return document.querySelector('#shortcuts-toggle')?.closest('details')?.open;"
            )
            .await?,
        true
    );
    let instructions = browser.text("main").await?;
    assert!(
        instructions.contains("Alt")
            && instructions.contains("Shift")
            && instructions.contains('M')
    );
    browser.chord(&[ALT, SHIFT], "M").await?;
    browser
        .wait(
            "return document.activeElement?.id === 'prototype-main';",
            Duration::from_secs(3),
        )
        .await?;
    browser
        .screenshot(&h.evidence.join("prototype-keyboard-shortcut.png"))
        .await?;
    Ok(())
}

async fn display_states(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "medicao").await?;
    for (index, state) in [(1, "empty"), (2, "loading"), (3, "error"), (4, "forbidden")] {
        browser.choose("#display-state", index, state).await?;
        browser
            .wait(
                &format!("return !!document.querySelector('#state-{state}');"),
                Duration::from_secs(3),
            )
            .await?;
        let text = browser.text(&format!("#state-{state}")).await?;
        assert!(!text.trim().is_empty(), "State cannot rely on color alone");
        if state == "error" {
            assert_eq!(browser.execute("return !!document.querySelector('#state-error[role=alert],#state-error [role=alert]');").await?, true);
        }
        if state == "forbidden" {
            let leaked = browser.execute(
                "return {fields:document.querySelectorAll('#measurement-note,#period-start,#period-end,#save-draft,#review-approval,#budget-filter').length,financial:document.querySelector('main').textContent.includes('18.200,00')};",
            ).await?;
            assert_eq!(
                leaked["fields"], 0,
                "Forbidden must remove business fields, not merely hide them"
            );
            assert_eq!(
                leaked["financial"], false,
                "Forbidden must not retain financial content in DOM"
            );
        }
        check_text_contrast(
            h,
            browser,
            &format!("state-{state}"),
            &format!("#state-{state}"),
        )
        .await?;
        browser
            .full_screenshot(&h.evidence.join(format!("prototype-state-{state}.png")))
            .await?;
        browser.tab_to(&format!("#state-{state} button")).await?;
        browser.key(ENTER).await?;
        browser.wait("return document.querySelector('#display-state')?.value === 'content' && !!document.querySelector('#measurement-note');", Duration::from_secs(3)).await?;
        assert_eq!(
            browser
                .execute(&format!(
                    "return !!document.querySelector('#state-{state}');"
                ))
                .await?,
            false,
            "The state's recovery action must actually return to task content"
        );
    }
    browser.choose("#display-state", 0, "content").await?;
    assert!(browser.enabled("#measurement-note").await?);
    for route in ROUTES {
        open(h, browser, route).await?;
        browser.choose("#display-state", 4, "forbidden").await?;
        let content = browser
            .execute("return document.querySelector('main').textContent;")
            .await?;
        let content = content.as_str().ok_or("Main text must be a string")?;
        assert!(
            !content.contains("18.200,00")
                && !content.contains("240.000,00")
                && !content.contains("Concreto para estrutura"),
            "Forbidden {route} must not retain business information, including hidden dialog contents"
        );
        assert_eq!(browser.execute("return document.querySelectorAll('#measurement-note,#period-start,#period-end,#save-draft,#review-approval,#budget-filter').length;").await?, 0);
    }
    Ok(())
}

async fn draft_and_context(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "medicao").await?;
    let original_start = browser.property("#period-start", "value").await?;
    let original_end = browser.property("#period-end", "value").await?;
    browser.tab_to("#period-start").await?;
    browser.key(DOWN).await?;
    browser.tab_to("#period-end").await?;
    browser.key(DOWN).await?;
    let start = browser.property("#period-start", "value").await?;
    let end = browser.property("#period-end", "value").await?;
    assert_ne!(
        start, original_start,
        "The negative case must really edit the period"
    );
    assert_ne!(
        end, original_end,
        "The negative case must really edit the period"
    );
    browser.fill("#measurement-note", SHORT_NOTE).await?;
    browser.tab_to("#save-draft").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#measurement-error')?.hidden === false && document.querySelector('#measurement-error').textContent.trim().length > 0;",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser.attribute("#measurement-error", "role").await?,
        "alert"
    );
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        SHORT_NOTE
    );
    assert_eq!(browser.property("#period-start", "value").await?, start);
    assert_eq!(browser.property("#period-end", "value").await?, end);
    check_text_contrast(h, browser, "invalid-draft", "#prototype-main").await?;
    browser.choose("#display-state", 3, "error").await?;
    browser.tab_to("#retry-content").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#measurement-note');",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        SHORT_NOTE,
        "Returning from a load error must preserve work in progress"
    );
    assert_eq!(browser.property("#period-start", "value").await?, start);
    assert_eq!(browser.property("#period-end", "value").await?, end);

    assert!(!browser.enabled("#complete-measurement").await?);
    let blocked_reason = browser
        .attribute("#complete-measurement", "aria-describedby")
        .await?;
    let reason_id = blocked_reason
        .as_str()
        .ok_or("Disabled conclusion needs an associated explanation")?;
    assert!(
        !browser
            .text(&format!("#{reason_id}"))
            .await?
            .trim()
            .is_empty()
    );
    browser.fill("#measurement-note", DRAFT_NOTE).await?;
    browser.tab_to("#nav-documentos").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#nav-documentos')?.getAttribute('aria-current') === 'page';", Duration::from_secs(5)).await?;
    browser.tab_to("#document-preview > summary").await?;
    browser.key(ENTER).await?;
    assert_eq!(
        browser
            .execute("return document.querySelector('#document-preview')?.open;")
            .await?,
        true
    );
    assert!(
        browser
            .text("#document-preview")
            .await?
            .contains("Memorial demonstrativo")
    );
    browser.tab_to("#nav-medicao").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#measurement-note');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        DRAFT_NOTE
    );
    assert_eq!(browser.property("#period-start", "value").await?, start);
    assert_eq!(browser.property("#period-end", "value").await?, end);

    request_company_change(browser).await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(3),
        )
        .await?;
    assert_dialog_traps_focus(browser, "#context-dialog").await?;
    browser.key(ESCAPE).await?;
    browser
        .wait(
            // HTML closes the dialog before it queues the `close` event. Wait
            // for the user-visible restoration performed by that event too.
            // https://html.spec.whatwg.org/multipage/interactive-elements.html#close-the-dialog
            "return document.querySelector('#context-dialog')?.open === false && document.querySelector('#organization')?.value === 'aurora' && document.activeElement?.id === 'organization';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(browser.property("#organization", "value").await?, "aurora");
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        DRAFT_NOTE
    );
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "organization"
    );
    request_company_change(browser).await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(3),
        )
        .await?;
    browser.tab_to("#cancel-context").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === false && document.querySelector('#organization')?.value === 'aurora' && document.activeElement?.id === 'organization';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        DRAFT_NOTE
    );
    browser.tab_to("#save-draft").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#draft-feedback')?.hidden === false;",
            Duration::from_secs(3),
        )
        .await?;
    let feedback = browser.text("#draft-feedback").await?.to_lowercase();
    assert!(
        (feedback.contains("simula") || feedback.contains("demonstra"))
            && feedback.contains("servidor"),
        "Draft result must disclose demonstration and lack of server persistence: {feedback}"
    );
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        DRAFT_NOTE
    );
    browser
        .fill(
            "#measurement-note",
            "Rascunho exclusivo da empresa Aurora, item D-72.",
        )
        .await?;
    request_company_change(browser).await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(3),
        )
        .await?;
    browser.tab_to("#confirm-context").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return location.pathname.includes('/prototipo/horizonte/');",
            Duration::from_secs(5),
        )
        .await?;
    browser.wait(READY, Duration::from_secs(20)).await?;
    assert_eq!(
        browser.property("#organization", "value").await?,
        "horizonte"
    );
    let main = browser.text("main").await?;
    assert!(main.contains("CT-027/2026"));
    assert!(!main.contains("CT-014/2026"));
    assert!(!main.contains("Escola Vila Nova"));
    assert!(!main.contains("18.200,00"));
    assert!(!main.contains("Rascunho exclusivo"));
    let note = browser
        .execute("return document.querySelector('#measurement-note')?.value ?? ''; ")
        .await?;
    assert!(
        !note
            .as_str()
            .unwrap_or_default()
            .contains("Rascunho exclusivo")
    );
    browser
        .screenshot(&h.evidence.join("prototype-company-changed.png"))
        .await?;
    Ok(())
}

async fn request_company_change(browser: &Browser) -> TestResult {
    browser.tab_to("#organization").await?;
    browser.key(" ").await?;
    browser.key(HOME).await?;
    browser.key(DOWN).await?;
    browser.key(ENTER).await?;
    Ok(())
}

async fn browser_history_context_guard(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "painel").await?;
    browser.choose("#organization", 1, "horizonte").await?;
    browser
        .wait(
            "return location.pathname === '/prototipo/horizonte/painel';",
            Duration::from_secs(5),
        )
        .await?;
    browser.tab_to("#nav-medicao").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#measurement-note');",
            Duration::from_secs(5),
        )
        .await?;
    const HISTORY_NOTE: &str = "Rascunho de Horizonte preservado ao voltar pelo histórico.";
    browser.fill("#measurement-note", HISTORY_NOTE).await?;
    browser.history_back().await?;
    browser
        .wait(
            "return location.pathname === '/prototipo/horizonte/painel';",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser
            .execute("return document.querySelector('#context-dialog')?.open === true;")
            .await?,
        false,
        "Back within the same company must preserve the local draft without asking to discard"
    );
    browser.history_back().await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(browser.text("#current-contract").await?, "CT-027/2026");
    browser.tab_to("#cancel-context").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === false && document.querySelector('#organization')?.value === 'horizonte' && document.activeElement?.id === 'organization';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser.property("#organization", "value").await?,
        "horizonte"
    );
    assert_eq!(browser.text("#current-contract").await?, "CT-027/2026");
    browser.tab_to("#nav-medicao").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#measurement-note');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        HISTORY_NOTE
    );
    browser.history_back().await?;
    browser
        .wait(
            "return location.pathname === '/prototipo/horizonte/painel';",
            Duration::from_secs(5),
        )
        .await?;
    browser.history_back().await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(5),
        )
        .await?;
    browser.tab_to("#confirm-context").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return location.pathname.startsWith('/prototipo/aurora/');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(browser.text("#current-contract").await?, "CT-014/2026");
    browser.tab_to("#nav-medicao").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#measurement-note');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser.property("#measurement-note", "value").await?,
        "",
        "A draft discarded through history confirmation cannot follow another company"
    );
    browser
        .screenshot(&h.evidence.join("prototype-history-context-guard.png"))
        .await?;
    Ok(())
}

async fn approval_dialog(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "aprovacoes").await?;
    browser.tab_to("#review-approval").await?;
    assert_visible_focus(browser).await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#approval-dialog')?.open === true;",
            Duration::from_secs(3),
        )
        .await?;
    assert_dialog_traps_focus(browser, "#approval-dialog").await?;
    let title = browser
        .attribute("#approval-dialog", "aria-labelledby")
        .await?;
    assert!(
        !title.as_str().unwrap_or_default().is_empty(),
        "Native dialog needs an accessible name"
    );
    check_text_contrast(h, browser, "approval-modal", "#approval-dialog").await?;
    browser
        .screenshot(&h.evidence.join("prototype-approval-modal.png"))
        .await?;
    browser.key(ESCAPE).await?;
    browser
        .wait(
            "return document.querySelector('#approval-dialog')?.open === false && document.activeElement?.id === 'review-approval';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "review-approval"
    );
    assert_eq!(
        browser
            .execute("return document.querySelector('#approval-feedback')?.hidden === false;")
            .await?,
        false,
        "Cancel must not claim an approval"
    );
    browser.key(ENTER).await?;
    browser.tab_to("#cancel-approval").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#approval-dialog')?.open === false && document.activeElement?.id === 'review-approval';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "review-approval"
    );
    browser.key(ENTER).await?;
    browser.tab_to("#confirm-approval").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#approval-feedback')?.hidden === false && document.querySelector('#approval-dialog')?.open === false && document.activeElement?.id === 'approval-feedback';",
            Duration::from_secs(3),
        )
        .await?;
    let feedback = browser.text("#approval-feedback").await?.to_lowercase();
    assert!(
        feedback.contains("simula"),
        "Approval outcome must disclose simulation: {feedback}"
    );
    assert!(
        !browser
            .execute("return document.querySelector('#approval-dialog')?.open === true;")
            .await?
            .as_bool()
            .unwrap_or(true)
    );
    check_text_contrast(h, browser, "approval-result", "#approval-feedback").await?;
    browser
        .screenshot(&h.evidence.join("prototype-approval-result.png"))
        .await?;
    browser.tab_to("#nav-painel").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#nav-painel')?.getAttribute('aria-current') === 'page';", Duration::from_secs(5)).await?;
    let dashboard = browser.text("#prototype-main").await?;
    assert!(dashboard.contains("Simulação concluída"));
    assert!(
        !dashboard.contains("1 em análise") && !dashboard.contains("1 decisão aguardando revisão"),
        "Dashboard must not still present an already simulated decision as pending"
    );
    Ok(())
}

async fn assert_dialog_traps_focus(browser: &Browser, selector: &str) -> TestResult {
    let quoted = serde_json::to_string(selector)?;
    assert_eq!(
        browser
            .execute(&format!(
                "return document.querySelector({quoted})?.matches(':modal');"
            ))
            .await?,
        true,
        "Dialog must make the background inert"
    );
    let contains_focus =
        format!("return document.querySelector({quoted})?.contains(document.activeElement);");
    assert_eq!(browser.execute(&contains_focus).await?, true);
    for _ in 0..8 {
        browser.key(TAB).await?;
        assert_eq!(
            browser.execute(&contains_focus).await?,
            true,
            "Forward Tab escaped {selector}"
        );
    }
    for _ in 0..8 {
        browser.chord(&[SHIFT], TAB).await?;
        assert_eq!(
            browser.execute(&contains_focus).await?,
            true,
            "Reverse Tab escaped {selector}"
        );
    }
    Ok(())
}

async fn reduced_motion_and_density(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser, "painel").await?;
    browser.ensure_options().await?;
    browser.click("#shortcuts-toggle").await?;
    assert_eq!(
        browser
            .execute(
                "return document.querySelector('#shortcuts-toggle')?.closest('details')?.open;"
            )
            .await?,
        true
    );
    browser.click("#shortcuts-toggle").await?;
    assert_eq!(
        browser
            .execute(
                "return document.querySelector('#shortcuts-toggle')?.closest('details')?.open;"
            )
            .await?,
        false
    );
    browser.choose("#density", 1, "compact").await?;
    assert_eq!(
        browser
            .attribute("#prototype-shell", "data-density")
            .await?,
        "compact"
    );
    browser.choose("#density", 0, "comfortable").await?;
    assert_eq!(
        browser
            .attribute("#prototype-shell", "data-density")
            .await?,
        "comfortable"
    );
    browser
        .cdp(
            "Emulation.setEmulatedMedia",
            json!({"features":[{"name":"prefers-reduced-motion","value":"reduce"}]}),
        )
        .await?;
    assert_eq!(
        browser
            .execute("return matchMedia('(prefers-reduced-motion: reduce)').matches;")
            .await?,
        true
    );
    let animation = browser.execute("return [...document.querySelectorAll('#prototype-shell *')].filter(e=>e.getClientRects().length).flatMap(e=>{const s=getComputedStyle(e);return [...s.animationDuration.split(','),...s.transitionDuration.split(',')].map(v=>parseFloat(v));}).filter(n=>n>0.001);").await?;
    assert_eq!(
        animation,
        json!([]),
        "Reduced motion must suppress nonessential movement"
    );
    browser
        .screenshot(&h.evidence.join("prototype-reduced-motion.png"))
        .await?;
    Ok(())
}

async fn assert_visible_focus(browser: &Browser) -> TestResult {
    let focus = browser.execute("const e=document.activeElement,r=e.getBoundingClientRect(),s=getComputedStyle(e);return {outline:parseFloat(s.outlineWidth),style:s.outlineStyle,height:r.height,width:r.width,top:r.top,bottom:r.bottom,viewport:innerHeight};").await?;
    assert!(
        number(&focus, "outline")? >= 3.0,
        "Keyboard focus must be visible: {focus}"
    );
    assert_ne!(focus["style"], "none");
    assert!(
        number(&focus, "height")? >= 44.0,
        "Primary action touch target"
    );
    assert!(number(&focus, "width")? >= 44.0);
    assert!(number(&focus, "top")? >= 0.0);
    assert!(number(&focus, "bottom")? <= number(&focus, "viewport")?);
    Ok(())
}

fn number(value: &Value, key: &str) -> TestResult<f64> {
    value[key]
        .as_f64()
        .ok_or_else(|| format!("Missing numeric metric {key}").into())
}

/// Observes actual text and inherited CSS colors. It does not infer results
/// from token declarations and never changes DOM, styles or application state.
const CONTRAST_OBSERVER: &str = r#"
const root = document.querySelector(SCOPE);
if (!root) throw new Error('Contrast scope absent');
const samples = new Map();
const hasArea = rectangles => [...rectangles].some(rect => rect.width > 0 && rect.height > 0);
const add = (element, text, textNode) => {
  if (!text?.trim() || !hasArea(element.getClientRects()) || element.closest('[hidden],[aria-hidden=true],:disabled,[aria-disabled=true],script,style,noscript,option')) return;
  for(let ancestor=element;ancestor;ancestor=ancestor.parentElement) {
    if(ancestor.matches('details:not([open])') && !ancestor.querySelector(':scope > summary')?.contains(element)) return;
  }
  if(textNode) {
    const range=document.createRange();
    range.selectNodeContents(textNode);
    if(!hasArea(range.getClientRects())) return;
  }
  const style = getComputedStyle(element);
  if (style.visibility !== 'visible') return;
  const layers=[];
  for(let node=element;node;node=node.parentElement) {
    const css=getComputedStyle(node);
    if(css.display==='none' || css.visibility!=='visible' || Number(css.opacity)===0) return;
    layers.push({background:css.backgroundColor,image:css.backgroundImage,opacity:Number(css.opacity)});
  }
  const record={tag:element.tagName,id:element.id,text:text.trim().slice(0,100),foreground:style.color,fontSize:parseFloat(style.fontSize),fontWeight:parseFloat(style.fontWeight),layers,count:1};
  const key=JSON.stringify([record.foreground,record.fontSize,record.fontWeight,layers]);
  if(samples.has(key)) samples.get(key).count++; else samples.set(key,record);
};
const walker=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);
while(walker.nextNode()) add(walker.currentNode.parentElement,walker.currentNode.textContent,walker.currentNode);
for(const field of root.querySelectorAll('input,select,textarea')) add(field,field.tagName==='SELECT' ? field.selectedOptions[0]?.textContent : field.value);
return [...samples.values()];
"#;

async fn check_text_contrast(
    h: &Harness,
    browser: &Browser,
    label: &str,
    scope: &str,
) -> TestResult {
    let script = CONTRAST_OBSERVER.replace("SCOPE", &serde_json::to_string(scope)?);
    let observations = browser.execute(&script).await?;
    let samples = observations
        .as_array()
        .ok_or("Contrast samples must be an array")?;
    assert!(
        !samples.is_empty(),
        "Contrast check must observe real visible text: {label}"
    );
    let mut measurements = Vec::new();
    let mut failures = Vec::new();
    for sample in samples {
        let layers = sample["layers"]
            .as_array()
            .ok_or("Background ancestry absent")?;
        let mut background = [1.0, 1.0, 1.0];
        for layer in layers.iter().rev() {
            if layer["image"] != "none" || layer["opacity"] != 1.0 {
                return Err(format!(
                    "Contrast cannot be certified by solid-color calculation for {label}: {sample}"
                )
                .into());
            }
            background = composite(
                parse_rgb(
                    layer["background"]
                        .as_str()
                        .ok_or("Background color absent")?,
                )?,
                background,
            );
        }
        let foreground = composite(
            parse_rgb(
                sample["foreground"]
                    .as_str()
                    .ok_or("Foreground color absent")?,
            )?,
            background,
        );
        let ratio = contrast_ratio(foreground, background);
        let font_size = number(sample, "fontSize")?;
        let font_weight = number(sample, "fontWeight")?;
        let required =
            if font_size >= 24.0 || (font_size >= 14.0 * 96.0 / 72.0 && font_weight >= 700.0) {
                3.0
            } else {
                4.5
            };
        let passed = ratio >= required;
        let measurement = json!({"sample":sample,"effectiveForeground":foreground,"effectiveBackground":background,"ratio":ratio,"minimum":required,"passed":passed});
        if !passed {
            failures.push(measurement.clone());
        }
        measurements.push(measurement);
    }
    write_json(
        &h.evidence.join(format!("prototype-contrast-{label}.json")),
        &json!({
            "status":if failures.is_empty() {"passed"} else {"failed"},
            "scope":scope,"source":"https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html",
            "measurements":measurements,
            "limitation":"Computed text/solid-background colors of currently rendered samples. Does not certify all WCAG requirements, text in images, screen readers or physical devices. Inactive controls and decorative/hidden text are excluded."
        }),
    )?;
    if !failures.is_empty() {
        return Err(format!(
            "Computed text contrast failed for {label}: {}",
            serde_json::to_string(&failures)?
        )
        .into());
    }
    Ok(())
}

fn parse_rgb(css: &str) -> TestResult<[f64; 4]> {
    let inner = css
        .strip_prefix("rgba(")
        .or_else(|| css.strip_prefix("rgb("))
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| format!("Unsupported computed RGB color: {css}"))?;
    let values = inner
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != 3 && values.len() != 4 {
        return Err(format!("Malformed computed RGB color: {css}").into());
    }
    let alpha = values.get(3).copied().unwrap_or(1.0);
    if !values[..3]
        .iter()
        .all(|channel| (0.0..=255.0).contains(channel))
        || !(0.0..=1.0).contains(&alpha)
    {
        return Err(format!("Out-of-range computed RGB color: {css}").into());
    }
    Ok([
        values[0] / 255.0,
        values[1] / 255.0,
        values[2] / 255.0,
        alpha,
    ])
}

fn composite(color: [f64; 4], background: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| color[index] * color[3] + background[index] * (1.0 - color[3]))
}

// WCAG 2.2 relative luminance, for measured sRGB colors only:
// https://www.w3.org/TR/WCAG22/#dfn-relative-luminance
fn contrast_ratio(first: [f64; 3], second: [f64; 3]) -> f64 {
    let luminance = |rgb: [f64; 3]| {
        let linear = rgb.map(|channel| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    };
    let first = luminance(first);
    let second = luminance(second);
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}

#[test]
fn contrast_measurement_rejects_near_threshold_and_composites_alpha() -> TestResult {
    let white = [1.0; 3];
    assert!((contrast_ratio([0.0; 3], white) - 21.0).abs() < 0.000_001);
    let gray = composite(parse_rgb("rgb(119, 119, 119)")?, white);
    assert!(
        contrast_ratio(gray, white) < 4.5,
        "4.478 must not round up to passing 4.5"
    );
    let gray = composite(parse_rgb("rgb(118, 118, 118)")?, white);
    assert!(contrast_ratio(gray, white) > 4.5);
    assert_eq!(composite(parse_rgb("rgba(0, 0, 0, 0.5)")?, white), [0.5; 3]);
    assert!(parse_rgb("rgb(300, 0, 0)").is_err());
    assert!(parse_rgb("rgba(0, 0, 0, 2)").is_err());
    Ok(())
}
