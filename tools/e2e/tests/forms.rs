//! Issue 4 acceptance operates the real Rust/Leptos interface through WebDriver.
//! Script execution only observes DOM, focus, semantics and resource requests;
//! the isolated laboratory observer also measures feedback without changing UI.
//! These examples contain fictitious data and never certify commercial readiness.

mod support;

use serde_json::{Value, json};
use std::{fs, sync::Arc, time::Duration};
use support::{
    ALT, Browser, DOWN, ENTER, ESCAPE, HOME, Harness, SHIFT, TAB, TestResult, write_json,
};

const READY: &str = "return document.querySelector('#forms-shell')?.dataset.ready === 'true';";
const PAGE_READY: &str = "return document.querySelector('#forms-shell')?.dataset.ready === 'true' && document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length > 0 && !document.querySelector('#measurement-grid')?.closest('[aria-busy=true]');";
const NOTE: &str = "Conferência fictícia do pavimento 2, trecho entre os eixos A e B.";

#[tokio::test]
#[ignore = "Requires release SSR/WASM, real PostgreSQL, Chromium and matching ChromeDriver"]
async fn forms_tables_preserve_exact_values_context_and_keyboard() -> TestResult {
    let h = Arc::new(Harness::start("forms-functional").await?);
    let browser = Arc::new(Browser::start(&h).await?);
    let task_h = Arc::clone(&h);
    let task_browser = Arc::clone(&browser);
    let result: TestResult =
        match tokio::spawn(async move { acceptance(&task_h, &task_browser).await }).await {
            Ok(result) => result,
            Err(error) => Err(format!("Forms acceptance task failed: {error}").into()),
        };
    if let Err(error) = &result {
        let screenshot_error = browser
            .screenshot(&h.evidence.join("forms-failure.png"))
            .await
            .err()
            .map(|error| error.to_string());
        let snapshot = browser.execute("return {url:location.href,title:document.title,focus:{tag:document.activeElement?.tagName,id:document.activeElement?.id},formsReady:document.querySelector('#forms-shell')?.dataset.ready,openDialogs:[...document.querySelectorAll('dialog[open]')].map(e=>e.id),fields:[...document.querySelectorAll('input,select,textarea')].map(e=>({id:e.id,value:e.value})),visibleText:document.body?.innerText.slice(0,20000)};").await;
        let console = browser.logs().await;
        let html_error = match browser
            .execute("return document.documentElement.outerHTML;")
            .await
        {
            Ok(html) => match html.as_str() {
                Some(html) => fs::write(h.evidence.join("forms-failure.html"), html)
                    .err()
                    .map(|error| error.to_string()),
                None => Some("DOM snapshot was not a string".into()),
            },
            Err(error) => Some(error.to_string()),
        };
        write_json(
            &h.evidence.join("forms-functional.json"),
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
    let server = server_pagination_and_exact_preview(h).await?;
    let viewports = reflow_and_semantics(h, browser).await?;
    let local_response = local_response_performance(h, browser).await?;
    keyboard_pagination_and_search(h, browser).await?;
    page_selection_and_copy(h, browser).await?;
    exact_fields_and_error_recovery(h, browser).await?;
    same_company_history_keeps_draft(browser).await?;
    delayed_preview_cannot_replace_newer_input(h, browser).await?;
    remote_failures_keep_work(h, browser).await?;
    context_guard_preserves_or_discards(h, browser).await?;
    let console = browser.logs().await?;
    for entry in console.as_array().ok_or("Browser logs must be an array")? {
        // Two expected network failures below are injected by the browser's
        // network layer, never by mutating the application or its request code.
        let expected_negative_response = entry["message"].as_str().is_some_and(|message| {
            message.contains("/api/prototipo/")
                && (message.contains("ERR_BLOCKED_BY_CLIENT")
                    || (message.contains("/medicoes/preview") && message.contains("status of 422")))
        });
        assert!(
            entry["level"] != "SEVERE" || expected_negative_response,
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
        &h.evidence.join("forms-functional.json"),
        &json!({
            "status":"passed", "versions":browser.versions, "environment":h.environment,
            "checks":[
                "server_page_bounded_with_10000_records", "stable_server_search_and_sort",
                "pt_br_exact_decimal_canonicalization", "ambiguous_value_rejected_without_coercion",
                "server_20_to_35_means_15", "server_cumulative_truncation_0_335",
                "reflow_360_and_1280", "table_and_controls_accessibly_named",
                "keyboard_pagination_and_explicit_edit", "page_selection_distinct_from_all_results",
                "copy_selected_page_rows_with_headers", "invalid_field_keeps_other_inputs",
                "field_error_association_and_correction_focus", "conditional_note_is_required_and_reachable", "draft_survives_filter_and_pagination", "history_keeps_grade_draft",
                "local_selection_and_expansion_feedback_measured_inside_browser", "late_preview_cannot_replace_newer_input",
                "actual_preview_network_failure_preserves_inputs", "table_retry_after_actual_network_failure",
                "context_guard_cancellation_preserves_grade_draft", "context_confirm_discards_only_with_consent"
            ],
            "server":server, "viewports":viewports, "localResponse":local_response, "console":console,
            "limits":"Fictitious prototype, desktop Chromium with simulated CSS widths. Does not represent a professional-user trial, assistive-technology session, physical mobile device or commercial validation."
        }),
    )?;
    println!(
        "Forms acceptance passed; evidence: {}",
        h.evidence.display()
    );
    Ok(())
}

async fn open(h: &Harness, browser: &Browser) -> TestResult {
    browser
        .navigate(&format!("{}/prototipo/aurora/planilha", h.app_url))
        .await?;
    browser.wait(READY, Duration::from_secs(20)).await?;
    browser.wait(PAGE_READY, Duration::from_secs(20)).await
}

async fn server_page(h: &Harness, query: &str) -> TestResult<Value> {
    let response = h
        .http
        .get(format!(
            "{}/api/prototipo/aurora/medicoes{query}",
            h.app_url
        ))
        .send()
        .await?;
    assert_eq!(response.status(), 200, "Server query {query}");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = response.bytes().await?;
    assert!(
        bytes.len() < 100_000,
        "A page must not transport the full fixture"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

async fn server_pagination_and_exact_preview(h: &Harness) -> TestResult<Value> {
    let first = server_page(h, "?page=1&page_size=25&sort=code&direction=asc").await?;
    assert_eq!(first["total"], 10_000);
    assert_eq!(first["total_pages"], 400);
    assert_eq!(first["rows"].as_array().ok_or("Rows missing")?.len(), 25);
    assert_eq!(first["rows"][0]["id"], 1);
    assert_eq!(first["rows"][0]["code"], "AUR-00001");
    assert_eq!(first["rows"][0]["previous"], "20");
    assert_eq!(first["rows"][0]["current"], "35");
    assert_eq!(first["rows"][0]["difference"], "15");
    assert_eq!(first["rows"][0]["unit"], "m²");
    assert_eq!(first["rows"][0]["period_value"], "150.00");
    assert_eq!(first["rows"][0]["balance"], "65");
    let second = server_page(h, "?page=2&page_size=25&sort=code&direction=asc").await?;
    assert_eq!(second["rows"][0]["id"], 26);
    let descending = server_page(h, "?page=1&page_size=25&sort=code&direction=desc").await?;
    assert_eq!(descending["rows"][0]["id"], 10_000);
    let filtered = server_page(h, "?page=1&page_size=25&search=AUR-00001").await?;
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["rows"][0]["id"], 1);
    let url = format!("{}/api/prototipo/aurora/medicoes/preview", h.app_url);
    let payload = json!({"row_id":1,"current":"35","unit_price":"1.234,56","percent":"5","date":"12/11/2026","unit":"m²","notes":NOTE});
    let response = h.http.post(&url).json(&payload).send().await?;
    assert_eq!(response.status(), 200);
    let valid: Value = response.json().await?;
    assert_eq!(valid["canonical"]["unit_price"], "1234.56");
    assert_eq!(valid["canonical"]["date"], "2026-11-12");
    assert_eq!(valid["preview"]["difference"], "15");
    assert_eq!(valid["preview"]["period_value"], "18518.40");
    let mut ambiguous = payload.clone();
    ambiguous["unit_price"] = json!("1.234");
    let response = h.http.post(&url).json(&ambiguous).send().await?;
    assert_eq!(response.status(), 422);
    let invalid: Value = response.json().await?;
    assert!(invalid["preview"].is_null());
    assert!(
        invalid["errors"]
            .as_array()
            .ok_or("Field errors missing")?
            .iter()
            .any(|error| error["field"] == "unit_price")
    );
    let mut cumulative = payload;
    cumulative["row_id"] = json!(2);
    cumulative["current"] = json!("2");
    cumulative["unit_price"] = json!("0,335");
    cumulative["unit"] = first["rows"][1]["unit"].clone();
    let response = h.http.post(&url).json(&cumulative).send().await?;
    assert_eq!(response.status(), 200);
    let cumulative: Value = response.json().await?;
    assert_eq!(cumulative["preview"]["period_value"], "0.34");
    Ok(
        json!({"fixtureTotal":10000,"pageSize":25,"pages":400,"first":first["rows"][0],"secondPageFirstId":second["rows"][0]["id"],"descendingFirstId":descending["rows"][0]["id"],"filteredTotal":filtered["total"],"exactPreview":valid,"ambiguous":invalid,"cumulativePreview":cumulative}),
    )
}

async fn reflow_and_semantics(h: &Harness, browser: &Browser) -> TestResult<Vec<Value>> {
    let mut observations = Vec::new();
    // A no-script visit reads the server-rendered page and uses native GET links.
    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value":true}),
        )
        .await?;
    browser
        .navigate(&format!("{}/prototipo/aurora/planilha", h.app_url))
        .await?;
    assert_eq!(browser.execute("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length;").await?,25);
    assert_eq!(browser.execute("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId;").await?,"1");
    tab_to(browser, "#table-next").await?;
    browser.key(ENTER).await?;
    assert_eq!(browser.execute("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId;").await?,"26");
    browser
        .full_screenshot(&h.evidence.join("forms-ssr-without-scripts.png"))
        .await?;
    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value":false}),
        )
        .await?;
    for width in [360, 1280] {
        browser.viewport(width, 900, false).await?;
        open(h, browser).await?;
        let metrics = browser.execute("const controls=[...document.querySelectorAll('#forms-shell input:not([type=hidden]),#forms-shell textarea,#forms-shell select')];return {viewport:innerWidth,document:document.documentElement.scrollWidth,body:document.body.scrollWidth,rows:document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length,columns:[...document.querySelectorAll('#measurement-grid thead th')].map(e=>e.innerText),caption:document.querySelector('#measurement-grid caption')?.innerText,unlabelled:controls.filter(e=>!e.labels?.length&&!e.getAttribute('aria-label')&&!e.getAttribute('aria-labelledby')).map(e=>e.id)};").await?;
        assert_eq!(metrics["viewport"], width);
        assert!(
            metrics["document"]
                .as_u64()
                .ok_or("Missing document width")?
                <= u64::from(width) + 1
        );
        assert!(metrics["body"].as_u64().ok_or("Missing body width")? <= u64::from(width) + 1);
        assert_eq!(metrics["rows"], 25, "Only a single page may exist in DOM");
        assert_eq!(metrics["unlabelled"], json!([]));
        let headings = metrics["columns"].to_string().to_lowercase();
        for heading in [
            "anterior",
            "atual",
            "período",
            "saldo",
            "unidade",
            "contratad",
        ] {
            assert!(
                headings.contains(heading),
                "Missing explicit measurement heading {heading}: {metrics}"
            );
        }
        assert!(!metrics["caption"].as_str().unwrap_or_default().is_empty());
        assert!(
            browser
                .text("#table-summary")
                .await?
                .replace('.', "")
                .contains("10000")
        );
        browser
            .full_screenshot(&h.evidence.join(format!("forms-table-{width}.png")))
            .await?;
        observations.push(json!({"width":width,"metrics":metrics}));
    }
    Ok(observations)
}

async fn keyboard_pagination_and_search(h: &Harness, browser: &Browser) -> TestResult {
    open(h, browser).await?;
    tab_to(browser, "#table-next").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '26';",Duration::from_secs(10)).await?;
    browser
        .wait(
            "return document.activeElement?.id === 'table-summary';",
            Duration::from_secs(3),
        )
        .await?;
    assert_eq!(browser.execute("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length;").await?, 25);
    tab_to(browser, "#table-prev").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '1';",Duration::from_secs(10)).await?;
    choose_field(browser, "#table-direction", 1, "desc").await?;
    choose_field(browser, "#table-page-size", 1, "50").await?;
    tab_to(browser, "#table-submit").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length === 50 && document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '10000';",Duration::from_secs(10)).await?;
    assert_eq!(browser.property("#table-direction", "value").await?, "desc");
    choose_field(browser, "#table-direction", 0, "asc").await?;
    choose_field(browser, "#table-page-size", 0, "25").await?;
    tab_to(browser, "#table-submit").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length === 25 && document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '1';",Duration::from_secs(10)).await?;
    browser.fill("#table-search", "AUR-00001").await?;
    tab_to(browser, "#table-submit").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length === 1 && document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '1';",Duration::from_secs(10)).await?;
    let requests = browser.execute("return performance.getEntriesByType('resource').filter(e=>e.name.includes('/api/prototipo/aurora/medicoes')).map(e=>({name:e.name,transferSize:e.transferSize,encodedBodySize:e.encodedBodySize}));").await?;
    assert!(
        requests
            .as_array()
            .ok_or("Resource observations absent")?
            .iter()
            .any(|request| request["name"]
                .as_str()
                .is_some_and(|url| url.contains("search=AUR-00001"))),
        "Search must issue a server query: {requests}"
    );
    tab_to(browser, "#edit-row-1").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(browser.property("#form-current", "value").await?, "35");
    ensure_advanced(browser).await?;
    browser.fill("#form-notes", NOTE).await?;
    tab_to(browser, "#form-cancel").await?;
    browser.key(ENTER).await?;
    browser.wait("return !document.querySelector('#form-current') && !!document.querySelector('#edit-row-1');", Duration::from_secs(5)).await?;
    browser.fill("#table-search", "AUR-00002").await?;
    browser.click("#table-submit").await?;
    browser.wait("return !document.querySelector('#edit-row-1') && !!document.querySelector('#edit-row-2');", Duration::from_secs(10)).await?;
    browser.fill("#table-search", "AUR-00001").await?;
    browser.click("#table-submit").await?;
    browser.wait("return !!document.querySelector('#edit-row-1') && !document.querySelector('#edit-row-2');", Duration::from_secs(10)).await?;
    browser.click("#edit-row-1").await?;
    browser
        .wait(
            "return !!document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser.property("#form-notes", "value").await?,
        NOTE,
        "Filtering the edited row out of view must not discard its draft"
    );
    browser.fill("#table-search", "AUR-00002").await?;
    browser.click("#table-submit").await?;
    browser.wait("return !document.querySelector('#edit-row-1') && !!document.querySelector('#edit-row-2') && !!document.querySelector('#form-current');", Duration::from_secs(10)).await?;
    browser.click("#form-cancel").await?;
    browser
        .wait(
            "return !document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    browser
        .wait(
            "return document.activeElement?.id === 'table-summary';",
            Duration::from_secs(3),
        )
        .await?;
    browser.fill("#table-search", "").await?;
    browser.click("#table-submit").await?;
    browser.wait("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length === 25;", Duration::from_secs(10)).await?;
    write_json(&h.evidence.join("forms-server-requests.json"), &requests)?;
    Ok(())
}

async fn page_selection_and_copy(h: &Harness, browser: &Browser) -> TestResult {
    // Keep the same navigation session so the draft conservation case remains live.
    tab_to(browser, "#select-page").await?;
    browser.key(" ").await?;
    browser
        .wait(
            "return document.querySelector('#selection-status')?.textContent.includes('25');",
            Duration::from_secs(3),
        )
        .await?;
    let selected = browser.text("#selection-status").await?.to_lowercase();
    assert!(
        selected.contains("página"),
        "Page selection must name its scope: {selected}"
    );
    assert!(
        !selected.contains("10.000 selecionad"),
        "Page selection must not claim all results"
    );
    tab_to(browser, "#copy-selection").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return !!document.querySelector('#copy-data');",
            Duration::from_secs(3),
        )
        .await?;
    let copied = browser.property("#copy-data", "value").await?;
    let copied = copied.as_str().ok_or("Copy fallback is not text")?;
    assert!(copied.contains("AUR-00001"));
    assert!(copied.contains("AUR-00025"));
    assert!(!copied.contains("AUR-00026"));
    assert!(
        copied.contains("\t"),
        "Copied data needs structured columns"
    );
    assert!(copied.lines().count() >= 26 && copied.lines().count() < 40);
    assert!(copied.to_lowercase().contains("unidade"));
    tab_to(browser, "#select-all-results").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#selection-status')?.textContent.replaceAll('.','').includes('10000');",
            Duration::from_secs(3),
        )
        .await?;
    let selected = browser.text("#selection-status").await?.to_lowercase();
    assert!(
        selected.contains("resultado") || selected.contains("filtro"),
        "All-results selection must explicitly name the complete scope: {selected}"
    );
    assert_eq!(browser.execute("return document.querySelectorAll('#measurement-grid tbody tr[data-row-id]').length;").await?,25,"Selecting all does not mount 10000 DOM rows");
    let export = browser.attribute("#export-selection", "href").await?;
    let export = export.as_str().ok_or("All-results export link missing")?;
    let export = if export.starts_with('/') {
        format!("{}{export}", h.app_url)
    } else {
        export.to_string()
    };
    assert!(
        export.starts_with(&h.app_url),
        "The fixture export remains same-origin"
    );
    let response = h.http.get(export).send().await?;
    assert_eq!(response.status(), 200);
    assert!(
        response.headers()["content-type"]
            .to_str()?
            .starts_with("text/tab-separated-values")
    );
    let all_rows = response.text().await?;
    assert!(all_rows.contains("AUR-00001") && all_rows.contains("AUR-10000"));
    assert!(
        all_rows.lines().count() >= 10001 && all_rows.lines().count() < 10020,
        "Export must include all filtered results and its metadata"
    );
    browser
        .screenshot(&h.evidence.join("forms-selection-scope.png"))
        .await?;
    browser.click("#clear-selection").await?;
    Ok(())
}

async fn exact_fields_and_error_recovery(h: &Harness, browser: &Browser) -> TestResult {
    browser.click("#edit-row-1").await?;
    browser
        .wait(
            "return !!document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    assert!(browser.enabled("#form-preview").await?);
    browser.fill("#form-money", "1.234,56").await?;
    browser.fill("#form-current", "35").await?;
    browser.fill("#form-date", "12/11/2026").await?;
    ensure_advanced(browser).await?;
    browser.fill("#form-notes", "").await?;
    browser.click("#form-advanced-toggle").await?;
    assert_eq!(
        browser
            .execute("return document.querySelector('#form-advanced')?.open;")
            .await?,
        false
    );
    browser.click("#form-preview").await?;
    browser.wait("return document.querySelector('#form-notes')?.getAttribute('aria-invalid') === 'true' && document.querySelector('#form-advanced')?.open === true && document.activeElement?.id === 'form-notes';",Duration::from_secs(5)).await?;
    assert_eq!(browser.property("#form-money", "value").await?, "1.234,56");
    assert_eq!(browser.property("#form-current", "value").await?, "35");
    assert_eq!(
        browser.attribute("#form-notes", "aria-required").await?,
        "true"
    );
    browser.fill("#form-notes", NOTE).await?;
    browser.fill("#form-percent", "12,5").await?;
    browser.fill("#form-unit", "m2").await?;
    browser.fill("#form-date", "31/02/2026").await?;
    browser.click("#form-preview").await?;
    browser
        .wait(
            "return document.querySelector('#form-date')?.getAttribute('aria-invalid') === 'true';",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(browser.property("#form-money", "value").await?, "1.234,56");
    assert_eq!(browser.property("#form-notes", "value").await?, NOTE);
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "form-date"
    );
    browser.fill("#form-date", "12/11/2026").await?;
    browser.key(TAB).await?;
    browser.wait("return document.querySelector('#field-money-interpretation')?.dataset.canonical === '1234.56';", Duration::from_secs(3)).await?;
    assert_eq!(
        browser
            .attribute("#field-percent-interpretation", "data-canonical")
            .await?,
        "12.5"
    );
    assert_eq!(
        browser
            .attribute("#field-unit-interpretation", "data-canonical")
            .await?,
        "m²"
    );
    assert_eq!(
        browser
            .attribute("#field-date-interpretation", "data-canonical")
            .await?,
        "2026-11-12"
    );
    let local_preview = browser.text("#form-local-preview").await?;
    assert!(
        local_preview.contains("15 m²") && local_preview.contains("18.518,40"),
        "The preview updates locally while editing: {local_preview}"
    );
    tab_to(browser, "#form-preview").await?;
    browser.key(ENTER).await?;
    browser
        .wait(
            "return document.querySelector('#form-save')?.disabled === false;",
            Duration::from_secs(10),
        )
        .await?;
    assert!(browser.text("#forms-shell").await?.contains("18.518,40"));
    browser.fill("#form-money", "1.234").await?;
    browser.key(TAB).await?;
    tab_to(browser, "#form-preview").await?;
    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#form-money')?.getAttribute('aria-invalid') === 'true';", Duration::from_secs(5)).await?;
    assert_eq!(
        browser.property("#form-money", "value").await?,
        "1.234",
        "An ambiguous value must never be silently coerced"
    );
    assert_eq!(browser.property("#form-current", "value").await?, "35");
    assert_eq!(browser.property("#form-date", "value").await?, "12/11/2026");
    assert_eq!(browser.property("#form-notes", "value").await?, NOTE);
    assert_eq!(browser.execute("return !document.querySelector('#form-save') || document.querySelector('#form-save').disabled;").await?,true,"Invalidating the form must invalidate the former successful preview");
    let described_by = browser.attribute("#form-money", "aria-describedby").await?;
    assert!(
        described_by
            .as_str()
            .unwrap_or_default()
            .contains("field-money-error")
    );
    assert!(!browser.text("#field-money-error").await?.trim().is_empty());
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "form-money",
        "Focus must move to the invalid field after the submission"
    );
    browser.click("#form-errors a[href='#form-money']").await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "form-money",
        "The summary link must reach the actual correction"
    );
    browser
        .full_screenshot(&h.evidence.join("forms-ambiguous-error-1280.png"))
        .await?;
    browser.fill("#form-money", "10,00").await?;
    browser.fill("#form-current", "101").await?;
    browser.click("#form-preview").await?;
    browser.wait("return document.querySelector('#form-current')?.getAttribute('aria-invalid') === 'true';",Duration::from_secs(5)).await?;
    let limit = browser.text("#field-current-error").await?;
    assert!(
        limit.contains("100"),
        "Excess must explain the contracted limit: {limit}"
    );
    browser.fill("#form-current", "35").await?;
    browser.click("#form-preview").await?;
    browser
        .wait(
            "return document.querySelector('#form-save')?.disabled === false;",
            Duration::from_secs(10),
        )
        .await?;
    browser.click("#form-save").await?;
    browser
        .wait(
            "return document.querySelector('#draft-status')?.dataset.state === 'saved';",
            Duration::from_secs(5),
        )
        .await?;
    let status = browser.text("#draft-status").await?.to_lowercase();
    assert!(
        status.contains("aba") || status.contains("local"),
        "Draft state must identify its local scope: {status}"
    );
    browser
        .screenshot(&h.evidence.join("forms-draft-saved.png"))
        .await?;
    Ok(())
}

async fn remote_failures_keep_work(h: &Harness, browser: &Browser) -> TestResult {
    if browser
        .execute("return !!document.querySelector('#form-current');")
        .await?
        != true
    {
        browser.click("#edit-row-1").await?;
        browser
            .wait(
                "return !!document.querySelector('#form-current');",
                Duration::from_secs(5),
            )
            .await?;
    }
    browser.fill("#form-money", "12,00").await?;
    ensure_advanced(browser).await?;
    browser.fill("#form-notes", NOTE).await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp(
            "Network.setBlockedURLs",
            json!({"urls":["*/api/prototipo/aurora/medicoes/preview*"]}),
        )
        .await?;
    browser.click("#form-preview").await?;
    browser.wait("const e=document.querySelector('#form-server-error');return !!e && e.getClientRects().length > 0 && e.textContent.trim().length > 0;",Duration::from_secs(10)).await?;
    assert_eq!(browser.property("#form-money", "value").await?, "12,00");
    assert_eq!(browser.property("#form-notes", "value").await?, NOTE);
    assert_eq!(browser.execute("return !document.querySelector('#form-save') || document.querySelector('#form-save').disabled;").await?,true,"Network failure cannot reuse an older successful preview");
    browser
        .screenshot(&h.evidence.join("forms-preview-network-error.png"))
        .await?;
    browser
        .cdp("Network.setBlockedURLs", json!({"urls":[]}))
        .await?;
    browser.click("#form-preview").await?;
    browser
        .wait(
            "return document.querySelector('#form-save')?.disabled === false;",
            Duration::from_secs(10),
        )
        .await?;
    browser.viewport(360, 900, false).await?;
    let metrics = browser.execute("return {viewport:innerWidth,width:document.documentElement.scrollWidth,unlabelled:[...document.querySelectorAll('#forms-shell input,#forms-shell textarea,#forms-shell select')].filter(e=>!e.labels?.length&&!e.getAttribute('aria-label')&&!e.getAttribute('aria-labelledby')).map(e=>e.id)};").await?;
    assert!(
        metrics["width"]
            .as_u64()
            .ok_or("Missing mobile editor width")?
            <= 361
    );
    assert_eq!(metrics["unlabelled"], json!([]));
    browser
        .full_screenshot(&h.evidence.join("forms-edit-360.png"))
        .await?;
    browser.viewport(1280, 900, false).await?;
    browser.click("#form-cancel").await?;
    browser
        .wait(
            "return !document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    browser
        .cdp(
            "Network.setBlockedURLs",
            json!({"urls":["*/api/prototipo/aurora/medicoes?*"]}),
        )
        .await?;
    browser.click("#table-next").await?;
    browser.wait("const e=document.querySelector('#table-error');return !!e && e.getClientRects().length > 0 && e.textContent.trim().length > 0;", Duration::from_secs(10)).await?;
    browser
        .cdp("Network.setBlockedURLs", json!({"urls":[]}))
        .await?;
    browser.click("#table-retry").await?;
    browser.wait("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '26';",Duration::from_secs(10)).await?;
    browser.click("#table-prev").await?;
    browser
        .wait(
            "return !!document.querySelector('#edit-row-1');",
            Duration::from_secs(10),
        )
        .await?;
    browser.click("#edit-row-1").await?;
    browser
        .wait(
            "return !!document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser.property("#form-money", "value").await?,
        "12,00",
        "Pagination and a failed page request must preserve the edited row"
    );
    assert_eq!(browser.property("#form-notes", "value").await?, NOTE);
    Ok(())
}

async fn same_company_history_keeps_draft(browser: &Browser) -> TestResult {
    browser.click("#nav-documentos").await?;
    browser.wait("return document.querySelector('#nav-documentos')?.getAttribute('aria-current') === 'page';",Duration::from_secs(5)).await?;
    browser.chord(&[ALT, SHIFT], "M").await?;
    browser
        .wait(
            "return document.activeElement?.id === 'prototype-main';",
            Duration::from_secs(3),
        )
        .await?;
    browser.history_back().await?;
    browser.wait(READY, Duration::from_secs(10)).await?;
    if browser
        .execute("return !!document.querySelector('#form-current');")
        .await?
        != true
    {
        browser.click("#edit-row-1").await?;
        browser
            .wait(
                "return !!document.querySelector('#form-current');",
                Duration::from_secs(5),
            )
            .await?;
    }
    assert_eq!(
        browser.property("#form-notes", "value").await?,
        NOTE,
        "Returning in browser history must preserve the draft"
    );
    assert_eq!(browser.property("#form-money", "value").await?, "10,00");
    Ok(())
}

async fn context_guard_preserves_or_discards(h: &Harness, browser: &Browser) -> TestResult {
    browser.choose("#organization", 1, "horizonte").await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(5),
        )
        .await?;
    assert_eq!(
        browser
            .execute("return document.querySelector('#context-dialog')?.matches(':modal');")
            .await?,
        true
    );
    for _ in 0..4 {
        browser.key(TAB).await?;
        assert_eq!(browser.execute("return document.querySelector('#context-dialog')?.contains(document.activeElement);").await?,true);
        browser.chord(&[SHIFT], TAB).await?;
        assert_eq!(browser.execute("return document.querySelector('#context-dialog')?.contains(document.activeElement);").await?,true);
    }
    browser.key(ESCAPE).await?;
    browser.wait("return document.querySelector('#context-dialog')?.open === false && document.querySelector('#organization')?.value === 'aurora';", Duration::from_secs(5)).await?;
    assert_eq!(browser.property("#form-money", "value").await?, "12,00");
    assert_eq!(browser.property("#form-notes", "value").await?, NOTE);
    browser.choose("#organization", 1, "horizonte").await?;
    browser
        .wait(
            "return document.querySelector('#context-dialog')?.open === true;",
            Duration::from_secs(5),
        )
        .await?;
    browser.click("#confirm-context").await?;
    browser.wait("return document.querySelector('#context-dialog')?.open === false && document.querySelector('#organization')?.value === 'horizonte';", Duration::from_secs(5)).await?;
    browser.click("#nav-planilha").await?;
    browser.wait(PAGE_READY, Duration::from_secs(10)).await?;
    let context = browser.text("#prototype-shell").await?;
    assert!(context.contains("CT-027/2026"));
    assert!(!context.contains("CT-014/2026") && !context.contains("AUR-00001"));
    browser.click("#edit-row-1").await?;
    browser
        .wait(
            "return !!document.querySelector('#form-current');",
            Duration::from_secs(5),
        )
        .await?;
    assert_ne!(
        browser.property("#form-notes", "value").await?,
        NOTE,
        "A grade draft must not leak to the next company"
    );
    browser
        .full_screenshot(&h.evidence.join("forms-context-horizonte.png"))
        .await?;
    Ok(())
}

async fn tab_to(browser: &Browser, selector: &str) -> TestResult {
    let expression = format!(
        "return document.activeElement?.matches({});",
        serde_json::to_string(selector)?
    );
    // A real page of 25 rows exposes select/edit controls for each row. Walking
    // the page is intentional; no script focuses or clicks an application node.
    for _ in 0..160 {
        browser.key(TAB).await?;
        if browser.execute(&expression).await? == true {
            return Ok(());
        }
    }
    Err(format!("Control not keyboard reachable: {selector}").into())
}

async fn choose_field(
    browser: &Browser,
    selector: &str,
    index: usize,
    expected: &str,
) -> TestResult {
    tab_to(browser, selector).await?;
    browser.key(" ").await?;
    browser.key(HOME).await?;
    for _ in 0..index {
        browser.key(DOWN).await?;
    }
    browser.key(ENTER).await?;
    let expression = format!(
        "return document.querySelector({})?.value === {};",
        serde_json::to_string(selector)?,
        serde_json::to_string(expected)?
    );
    browser.wait(&expression, Duration::from_secs(3)).await
}

async fn ensure_advanced(browser: &Browser) -> TestResult {
    if browser.execute("return document.querySelector('#form-advanced-toggle')?.closest('details')?.open === false;").await? == true {
        tab_to(browser,"#form-advanced-toggle").await?;
        browser.key(ENTER).await?;
        browser.wait("return document.querySelector('#form-advanced-toggle')?.closest('details')?.open === true;",Duration::from_secs(3)).await?;
    }
    Ok(())
}

// Laboratory-only instrumentation, following browser.rs. It neither dispatches
// events nor changes application DOM/state. All actions are real WebDriver input.
// The second animation frame is a conservative feedback proxy, not real INP.
const OBSERVE_LOCAL_RESPONSE: &str = r#"(() => {
  const lab = globalThis.__canteiroFormsLab = {samples:[], pending:0};
  document.addEventListener('click', event => {
    const target = event.target.closest?.('#select-page,#form-advanced-toggle');
    if (!target) return;
    const kind = target.id === 'select-page' ? 'selection' : 'expansion';
    const expected = kind === 'selection' ? target.checked : !target.closest('details').open;
    const started = event.timeStamp;
    const dispatched = performance.now();
    lab.pending++;
    requestAnimationFrame(() => {
      const firstFrame = performance.now();
      requestAnimationFrame(() => {
        const completed = performance.now();
        const rows = [...document.querySelectorAll('#measurement-grid tbody tr[data-row-id]')];
        const effectObserved = kind === 'selection'
          ? rows.length === 25 && rows.every(row => row.dataset.selected === String(expected))
          : document.querySelector('#form-advanced')?.open === expected;
        lab.samples.push({kind, expected, eventTimeMs:started, dispatchedAtMs:dispatched,
          firstFrameAtMs:firstFrame, secondFrameAtMs:completed,
          feedbackUpperBoundMs:completed-started, effectObserved, rowsInDom:rows.length});
        lab.pending--;
      });
    });
  }, true);
})();"#;

async fn local_response_performance(h: &Harness, browser: &Browser) -> TestResult<Value> {
    write_json(
        &h.evidence.join("forms-local-response.json"),
        &json!({"status":"running"}),
    )?;
    let observer = browser
        .cdp(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source":OBSERVE_LOCAL_RESPONSE}),
        )
        .await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp("Emulation.setCPUThrottlingRate", json!({"rate":4}))
        .await?;
    browser.cdp("Network.emulateNetworkConditions",json!({"offline":false,"latency":150,"downloadThroughput":500000,"uploadThroughput":125000,"connectionType":"cellular4g"})).await?;
    let mut profiles = Vec::new();
    let mut violations = Vec::new();
    for (profile, width, height, mobile) in [
        ("desktop", 1366, 768, false),
        ("mobile-css", 360, 800, true),
    ] {
        browser.viewport(width, height, mobile).await?;
        open(h, browser).await?;
        for count in 1..=20 {
            browser.click("#select-page").await?;
            browser.wait(&format!("return globalThis.__canteiroFormsLab?.samples.filter(s=>s.kind === 'selection').length === {count} && globalThis.__canteiroFormsLab.pending === 0;"),Duration::from_secs(5)).await?;
        }
        browser.click("#edit-row-1").await?;
        browser
            .wait(
                "return !!document.querySelector('#form-advanced-toggle');",
                Duration::from_secs(5),
            )
            .await?;
        for count in 1..=20 {
            browser.click("#form-advanced-toggle").await?;
            browser.wait(&format!("return globalThis.__canteiroFormsLab?.samples.filter(s=>s.kind === 'expansion').length === {count} && globalThis.__canteiroFormsLab.pending === 0;"),Duration::from_secs(5)).await?;
        }
        let samples = browser
            .execute("return globalThis.__canteiroFormsLab.samples;")
            .await?;
        let samples = samples.as_array().ok_or("Local response samples absent")?;
        let mut summaries = Vec::new();
        for kind in ["selection", "expansion"] {
            let observed = samples
                .iter()
                .filter(|sample| sample["kind"] == kind)
                .collect::<Vec<_>>();
            assert_eq!(
                observed.len(),
                20,
                "Each action needs its own actual observations"
            );
            let mut timings = Vec::new();
            for sample in &observed {
                let duration = sample["feedbackUpperBoundMs"]
                    .as_f64()
                    .ok_or("Local response duration absent")?;
                assert!(
                    duration.is_finite() && duration >= 0.0,
                    "Invalid browser timing {sample}"
                );
                if sample["effectObserved"] != true || duration > 100.0 {
                    violations.push(json!({"profile":profile,"sample":sample}));
                }
                timings.push(duration);
            }
            timings.sort_by(f64::total_cmp);
            summaries.push(json!({"action":kind,"samples":20,"minimumMs":timings[0],"p75Ms":timings[14],"maximumMs":timings[19],"limitMs":100}));
        }
        profiles.push(json!({"profile":profile,"viewport":{"width":width,"height":height,"mobileEmulation":mobile},"summaries":summaries,"samples":samples}));
    }
    browser
        .cdp("Emulation.setCPUThrottlingRate", json!({"rate":1}))
        .await?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.cdp("Page.removeScriptToEvaluateOnNewDocument",json!({"identifier":observer["identifier"].as_str().ok_or("Observer identifier missing")?})).await?;
    browser.viewport(1280, 900, false).await?;
    let report = json!({
        "status":if violations.is_empty(){"passed"}else{"failed"},"versions":browser.versions,"environment":h.environment,
        "method":"Browser event.timeStamp to second requestAnimationFrame, with expected DOM effect observed. No WebDriver or tool round-trip latency is included. This is a conservative laboratory feedback proxy, not visitor INP or a hardware display measurement.",
        "conditions":{"cpuThrottle":4,"networkLatencyMs":150,"downloadBytesPerSecond":500000,"uploadBytesPerSecond":125000},
        "scope":"20 selection toggles and 20 progressive-disclosure toggles per profile, after hydration, one fixture page of 25 rows. Does not claim 20 navigations per route, exclusive 4 vCPU allocation, physical Android or the full integrated 9.3 protocol.",
        "profiles":profiles,"violations":violations
    });
    write_json(&h.evidence.join("forms-local-response.json"), &report)?;
    assert!(
        violations.is_empty(),
        "Local feedback exceeded 100 ms or failed to show its effect; inspect forms-local-response.json"
    );
    Ok(report)
}

async fn delayed_preview_cannot_replace_newer_input(h: &Harness, browser: &Browser) -> TestResult {
    browser.fill("#form-current", "36").await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser.cdp("Network.emulateNetworkConditions",json!({"offline":false,"latency":1200,"downloadThroughput":500000,"uploadThroughput":125000,"connectionType":"cellular4g"})).await?;
    browser.click("#form-preview").await?;
    browser
        .wait(
            "return document.querySelector('#form-preview')?.disabled === true;",
            Duration::from_secs(2),
        )
        .await?;
    browser.fill("#form-current", "37").await?;
    assert_eq!(
        browser
            .execute("return document.querySelector('#form-preview')?.disabled;")
            .await?,
        true,
        "The old request must still be in flight when its input changes"
    );
    browser
        .wait(
            "return document.querySelector('#form-preview')?.disabled === false;",
            Duration::from_secs(12),
        )
        .await?;
    assert_eq!(browser.property("#form-current", "value").await?, "37");
    assert_eq!(browser.execute("return !document.querySelector('#form-comparison') && (!document.querySelector('#form-save') || document.querySelector('#form-save').disabled);").await?,true,"A late response for 36 must not certify the newer 37 input");
    let requests = browser.execute("return performance.getEntriesByType('resource').filter(e=>e.name.includes('/medicoes/preview')).map(e=>({url:e.name,durationMs:e.duration}));").await?;
    let delayed = requests
        .as_array()
        .and_then(|requests| requests.last())
        .ok_or("Delayed request timing absent")?;
    assert!(
        delayed["durationMs"]
            .as_f64()
            .ok_or("Delayed duration missing")?
            >= 900.0,
        "The network delay must actually occur: {delayed}"
    );
    browser
        .screenshot(&h.evidence.join("forms-late-preview-ignored.png"))
        .await?;
    write_json(
        &h.evidence.join("forms-delayed-preview.json"),
        &json!({"status":"passed","sentCurrent":"36","editedWhilePending":"37","lateResponseApplied":false,"request":delayed,"method":"CDP network latency of 1200 ms while the real HTTP request executes. Input changed through WebDriver before completion; no application state or response was mocked."}),
    )?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#form-preview").await?;
    browser
        .wait(
            "return document.querySelector('#form-save')?.disabled === false;",
            Duration::from_secs(10),
        )
        .await?;
    assert_eq!(browser.text("#preview-difference").await?, "17 m²");
    assert_eq!(browser.text("#preview-value").await?, "R$ 170,00");
    browser.fill("#form-current", "35").await?;
    Ok(())
}
