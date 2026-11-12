//! Issue 3 acceptance observes the real Rust/Leptos application in Chromium.
//! Script strings inspect DOM, styles and accessibility semantics only. User
//! operations use WebDriver keys/clicks, including native selects and dialogs.
//! This suite is not a substitute for the required three professional users.

mod support;

use serde_json::{Value, json};
use std::time::Duration;
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
    let h = Harness::start("prototype-functional").await?;
    let browser = Browser::start(&h).await?;
    let result = acceptance(&h, &browser).await;
    if let Err(error) = &result {
        let _ = browser
            .screenshot(&h.evidence.join("prototype-failure.png"))
            .await;
        write_json(
            &h.evidence.join("prototype-functional.json"),
            &json!({"status":"failed","error":error.to_string(),"versions":browser.versions}),
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
                "filter_detail_and_return_preserve_query", "loading_empty_error_forbidden_states",
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
            assert_eq!(browser.attribute("#organization", "value").await?, "aurora");
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
        browser.attribute("#budget-filter", "value").await?,
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
        browser.attribute("#budget-filter", "value").await?,
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
        browser
            .full_screenshot(&h.evidence.join(format!("prototype-state-{state}.png")))
            .await?;
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
    let original_start = browser.attribute("#period-start", "value").await?;
    let original_end = browser.attribute("#period-end", "value").await?;
    browser.tab_to("#period-start").await?;
    browser.key(DOWN).await?;
    browser.tab_to("#period-end").await?;
    browser.key(DOWN).await?;
    let start = browser.attribute("#period-start", "value").await?;
    let end = browser.attribute("#period-end", "value").await?;
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
        browser.attribute("#measurement-note", "value").await?,
        SHORT_NOTE
    );
    assert_eq!(browser.attribute("#period-start", "value").await?, start);
    assert_eq!(browser.attribute("#period-end", "value").await?, end);
    browser.choose("#display-state", 3, "error").await?;
    browser.choose("#display-state", 0, "content").await?;
    assert_eq!(
        browser.attribute("#measurement-note", "value").await?,
        SHORT_NOTE,
        "Returning from a load error must preserve work in progress"
    );
    assert_eq!(browser.attribute("#period-start", "value").await?, start);
    assert_eq!(browser.attribute("#period-end", "value").await?, end);

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
        browser.attribute("#measurement-note", "value").await?,
        DRAFT_NOTE
    );
    assert_eq!(browser.attribute("#period-start", "value").await?, start);
    assert_eq!(browser.attribute("#period-end", "value").await?, end);

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
            "return !document.querySelector('#context-dialog')?.open;",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(browser.attribute("#organization", "value").await?, "aurora");
    assert_eq!(
        browser.attribute("#measurement-note", "value").await?,
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
    assert_eq!(
        browser.attribute("#measurement-note", "value").await?,
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
        browser.attribute("#measurement-note", "value").await?,
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
        browser.attribute("#organization", "value").await?,
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
            "return document.querySelector('#context-dialog')?.open === false;",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(
        browser.attribute("#organization", "value").await?,
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
        browser.attribute("#measurement-note", "value").await?,
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
        browser.attribute("#measurement-note", "value").await?,
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
    browser
        .screenshot(&h.evidence.join("prototype-approval-modal.png"))
        .await?;
    browser.key(ESCAPE).await?;
    browser
        .wait(
            "return !document.querySelector('#approval-dialog')?.open;",
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
            "return !document.querySelector('#approval-dialog')?.open;",
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
            "return document.querySelector('#approval-feedback')?.hidden === false;",
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
        number(&focus, "outline")? >= 2.0,
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
