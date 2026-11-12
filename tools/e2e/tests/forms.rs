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

// Separate test cases preserve independent failure reports. Serialize their
// browser workloads so the performance group is not measured under our own
// concurrent functional workload. A failed group releases this async guard.
static FORMS_BROWSER_RUN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Copy)]
enum Group {
    Presentation,
    LocalResponse,
    Functional,
}

impl Group {
    fn name(self) -> &'static str {
        match self {
            Self::Presentation => "forms-presentation",
            Self::LocalResponse => "forms-local-response",
            Self::Functional => "forms-functional",
        }
    }
}

#[tokio::test]
#[ignore = "Requires release SSR/WASM, real PostgreSQL, Chromium and matching ChromeDriver"]
async fn forms_http_ssr_and_presentation() -> TestResult {
    run_group(Group::Presentation).await
}

#[tokio::test]
#[ignore = "Requires release SSR/WASM, real PostgreSQL, Chromium and matching ChromeDriver"]
async fn forms_local_selection_and_expansion_feedback() -> TestResult {
    run_group(Group::LocalResponse).await
}

#[tokio::test]
#[ignore = "Requires release SSR/WASM, real PostgreSQL, Chromium and matching ChromeDriver"]
async fn forms_drafts_tables_and_context() -> TestResult {
    run_group(Group::Functional).await
}

async fn run_group(group: Group) -> TestResult {
    let _exclusive_browser = FORMS_BROWSER_RUN.lock().await;
    let name = group.name();
    let h = Arc::new(Harness::start(name).await?);
    let browser = Arc::new(Browser::start(&h).await?);
    let task_h = Arc::clone(&h);
    let task_browser = Arc::clone(&browser);
    let result: TestResult = match tokio::spawn(async move {
        match group {
            Group::Presentation => presentation_acceptance(&task_h, &task_browser).await,
            Group::LocalResponse => response_acceptance(&task_h, &task_browser).await,
            Group::Functional => functional_acceptance(&task_h, &task_browser).await,
        }
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("{name} acceptance task failed: {error}").into()),
    };
    if let Err(error) = &result {
        let screenshot_error = browser
            .screenshot(&h.evidence.join(format!("{name}-failure.png")))
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
                Some(html) => fs::write(h.evidence.join(format!("{name}-failure.html")), html)
                    .err()
                    .map(|error| error.to_string()),
                None => Some("DOM snapshot was not a string".into()),
            },
            Err(error) => Some(error.to_string()),
        };
        // Retain already measured samples if a performance assertion failed.
        // Harness::start replaced any older run's report before this attempt.
        let phase_evidence = fs::read(h.evidence.join(format!("{name}.json")))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        write_json(
            &h.evidence.join(format!("{name}.json")),
            &json!({
                "status":"failed","group":name,"error":error.to_string(),"versions":browser.versions,"environment":h.environment,
                "snapshot":snapshot.as_ref().ok(),"snapshotError":snapshot.as_ref().err().map(ToString::to_string),
                "console":console.as_ref().ok(),"consoleError":console.as_ref().err().map(ToString::to_string),
                "screenshotError":screenshot_error,"htmlError":html_error,"phaseEvidence":phase_evidence
            }),
        )?;
    }
    let closing = browser.close().await;
    result?;
    closing?;
    Ok(())
}

async fn presentation_acceptance(h: &Harness, browser: &Browser) -> TestResult {
    let server = server_pagination_and_exact_preview(h).await?;
    let viewports = reflow_and_semantics(h, browser).await?;
    let console = browser.logs().await?;
    assert_console(&console, false)?;
    write_json(
        &h.evidence.join("forms-presentation.json"),
        &json!({
            "status":"passed","group":"forms-presentation","versions":browser.versions,"environment":h.environment,
            "checks":["server_page_bounded_with_10000_records","stable_server_search_and_sort","server_exact_decimal_canonicalization","server_ambiguous_value_rejected","server_20_to_35_means_15","server_cumulative_truncation_0_335","server_rendered_page_and_pagination_without_scripts","reflow_360_and_1280","table_and_controls_accessibly_named","initial_row_identity_and_edit_visible_without_horizontal_scroll","row_selection_and_edit_targets_44px","document_ends_at_footer_without_escaped_grid_content"],
            "server":server,"viewports":viewports,"console":console,
            "limits":"HTTP contracts, no-script navigation and rendered layout only. Does not claim the independent draft/context journey, response-time group, assistive-technology use or professional-user trial."
        }),
    )?;
    Ok(())
}

async fn response_acceptance(h: &Harness, browser: &Browser) -> TestResult {
    let mut report = local_response_performance(h, browser).await?;
    let console = browser.logs().await?;
    assert_console(&console, false)?;
    report["group"] = json!("forms-local-response");
    report["console"] = console;
    write_json(&h.evidence.join("forms-local-response.json"), &report)
}

async fn functional_acceptance(h: &Harness, browser: &Browser) -> TestResult {
    browser.viewport(1280, 900, false).await?;
    keyboard_pagination_and_search(h, browser).await?;
    page_selection_and_copy(h, browser).await?;
    exact_fields_and_error_recovery(h, browser).await?;
    same_company_history_keeps_draft(browser).await?;
    delayed_preview_cannot_replace_newer_input(h, browser).await?;
    delayed_table_keeps_the_typing_focus(h, browser).await?;
    remote_failures_keep_work(h, browser).await?;
    context_guard_preserves_or_discards(h, browser).await?;
    sticky_grid_controls_remain_reachable(h, browser).await?;
    let console = browser.logs().await?;
    assert_console(&console, true)?;
    write_json(
        &h.evidence.join("forms-functional.json"),
        &json!({
            "status":"passed", "group":"forms-functional", "versions":browser.versions, "environment":h.environment,
            "checks":[
                "ui_exact_decimal_date_percent_and_unit_interpretation", "ambiguous_value_rejected_without_coercion",
                "keyboard_pagination_and_explicit_edit", "page_selection_distinct_from_all_results",
                "copy_selected_page_rows_with_headers", "invalid_field_keeps_other_inputs",
                "field_error_association_and_correction_focus", "conditional_note_is_required_and_reachable", "draft_survives_filter_and_pagination", "history_keeps_grade_draft",
                "late_preview_cannot_replace_newer_input", "late_table_response_does_not_steal_typing_focus",
                "actual_preview_network_failure_preserves_inputs", "table_retry_after_actual_network_failure",
                "context_guard_cancellation_preserves_grade_draft", "context_confirm_discards_only_with_consent",
                "sticky_grid_targets_remain_visible_and_clickable_after_keyboard_scroll"
            ],
            "console":console,
            "limits":"Fictitious draft/context journey. Does not claim results from the independent HTTP/SSR/layout or response-time groups, a professional-user trial, assistive-technology session, physical mobile device or commercial validation."
        }),
    )?;
    println!(
        "Forms acceptance passed; evidence: {}",
        h.evidence.display()
    );
    Ok(())
}

fn assert_console(console: &Value, allow_expected_network_failures: bool) -> TestResult {
    for entry in console.as_array().ok_or("Browser logs must be an array")? {
        // Only the functional journey intentionally exercises these failures.
        let expected_negative_response = allow_expected_network_failures
            && entry["message"].as_str().is_some_and(|message| {
                message.contains("/api/prototipo/")
                    && (message.contains("ERR_BLOCKED_BY_CLIENT")
                        || (message.contains("/medicoes/preview")
                            && message.contains("status of 422")))
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
    browser.wait("return document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '26';",Duration::from_secs(10)).await?;
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
        let first_row_geometry = initial_grid_controls(h, browser, width).await?;
        let vertical_extent = document_ends_at_footer(h, browser, width).await?;
        browser
            .full_screenshot(&h.evidence.join(format!("forms-table-{width}.png")))
            .await?;
        observations.push(json!({"width":width,"metrics":metrics,"firstRowGeometry":first_row_geometry,"verticalExtent":vertical_extent}));
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
    clear_field_by_keyboard(browser, "#table-search").await?;
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
    scroll_first_edit_into_pointer_view(h, browser).await?;
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
    clear_field_by_keyboard(browser, "#form-notes").await?;
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
    browser.wait(PAGE_READY, Duration::from_secs(10)).await?;
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

async fn clear_field_by_keyboard(browser: &Browser, selector: &str) -> TestResult {
    browser.fill(selector, "").await?;
    assert_eq!(browser.property(selector, "value").await?, "");
    Ok(())
}

async fn document_ends_at_footer(h: &Harness, browser: &Browser, width: u32) -> TestResult<Value> {
    let mut metrics = browser.execute(r#"
const footer=document.querySelector('footer.site-footer');
return {documentScrollHeight:document.documentElement.scrollHeight,bodyScrollHeight:document.body.scrollHeight,footerBottom:footer?footer.getBoundingClientRect().bottom+scrollY:null,innerHeight};
"#).await?;
    let layout = browser.cdp("Page.getLayoutMetrics", json!({})).await?;
    metrics["cdpCssContentHeight"] = layout["cssContentSize"]["height"].clone();
    let path = h
        .evidence
        .join(format!("forms-document-height-{width}.json"));
    write_json(&path, &metrics)?;
    let number = |key: &str| -> TestResult<f64> {
        let value = metrics[key]
            .as_f64()
            .ok_or_else(|| format!("Missing vertical extent {key}: {metrics}"))?;
        assert!(value.is_finite() && value > 0.0, "Invalid {key}: {metrics}");
        Ok(value)
    };
    let largest_extent = number("documentScrollHeight")?
        .max(number("bodyScrollHeight")?)
        .max(number("cdpCssContentHeight")?);
    // The footer rectangle already includes its 24 px vertical padding; body
    // has no margin. Permit only a small trailing spacing band, not grid rows
    // escaping their scroll container and creating thousands of blank pixels.
    let maximum_extent = number("innerHeight")?.max(number("footerBottom")?) + 32.0;
    metrics["largestExtent"] = json!(largest_extent);
    metrics["maximumExtent"] = json!(maximum_extent);
    metrics["trailingAllowancePx"] = json!(32);
    write_json(&path, &metrics)?;
    assert!(
        largest_extent <= maximum_extent,
        "Content escapes beyond the footer at {width}px: {metrics}"
    );
    Ok(metrics)
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

async fn delayed_table_keeps_the_typing_focus(h: &Harness, browser: &Browser) -> TestResult {
    assert_eq!(
        browser
            .execute("return !!document.querySelector('#form-current');")
            .await?,
        true,
        "The row editor must remain open during pagination"
    );
    browser.cdp("Network.enable", json!({})).await?;
    browser.cdp("Network.emulateNetworkConditions",json!({"offline":false,"latency":1200,"downloadThroughput":500000,"uploadThroughput":125000,"connectionType":"cellular4g"})).await?;
    browser.click("#table-next").await?;
    browser.wait("return document.querySelector('#table-content')?.getAttribute('aria-busy') === 'true';",Duration::from_secs(2)).await?;
    browser.fill("#form-current", "38").await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "form-current"
    );
    assert_eq!(
        browser.attribute("#table-content", "aria-busy").await?,
        "true",
        "Typing must occur before the delayed GET completes"
    );
    browser.wait("return document.querySelector('#table-content')?.getAttribute('aria-busy') === 'false' && document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '26';",Duration::from_secs(12)).await?;
    // Observe after two frames so a scheduled focus restoration has had time
    // to run. An immediate assertion could miss the very focus theft at issue.
    let settled = browser.post("execute/async",json!({
        "script":"const done=arguments[arguments.length-1];requestAnimationFrame(()=>requestAnimationFrame(()=>done({focus:document.activeElement?.id,current:document.querySelector('#form-current')?.value,notes:document.querySelector('#form-notes')?.value,firstRow:document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId,busy:document.querySelector('#table-content')?.getAttribute('aria-busy')})));",
        "args":[]
    })).await?;
    assert_eq!(
        settled["focus"], "form-current",
        "The completed table request must not steal focus from an input being edited"
    );
    assert_eq!(settled["current"], "38");
    assert_eq!(settled["notes"], NOTE);
    assert_eq!(settled["firstRow"], "26");
    assert_eq!(settled["busy"], "false");
    let requests = browser.execute("return performance.getEntriesByType('resource').filter(e=>e.name.includes('/api/prototipo/aurora/medicoes?')&&new URL(e.name).searchParams.get('page')==='2').map(e=>({url:e.name,durationMs:e.duration}));").await?;
    let delayed = requests
        .as_array()
        .and_then(|requests| requests.last())
        .ok_or("Delayed table timing absent")?;
    assert!(
        delayed["durationMs"]
            .as_f64()
            .ok_or("Delayed table duration missing")?
            >= 900.0,
        "The GET must have actually been delayed: {delayed}"
    );
    browser
        .screenshot(&h.evidence.join("forms-delayed-table-keeps-focus.png"))
        .await?;
    write_json(
        &h.evidence.join("forms-delayed-table-focus.json"),
        &json!({
            "status":"passed","request":delayed,"settled":settled,
            "method":"Real page 2 GET delayed by CDP network latency of 1200 ms. After the request starts, WebDriver types 38 into the already open row editor. Focus, content and loaded page are observed after completion and two animation frames. No application state or response was mocked."
        }),
    )?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({"offline":false,"latency":0,"downloadThroughput":-1,"uploadThroughput":-1}),
        )
        .await?;
    browser.click("#table-prev").await?;
    browser.wait("return document.querySelector('#table-content')?.getAttribute('aria-busy') === 'false' && document.querySelector('#measurement-grid tbody tr[data-row-id]')?.dataset.rowId === '1';",Duration::from_secs(10)).await?;
    browser.fill("#form-current", "35").await?;
    Ok(())
}

async fn initial_grid_controls(h: &Harness, browser: &Browser, width: u32) -> TestResult<Value> {
    let geometry = browser.execute(r#"
const scroller = document.querySelector('.measurement-scroll');
const row = document.querySelector('#measurement-grid tbody tr[data-row-id="1"]');
if (!scroller || !row) throw new Error('First row or scroller missing');
const rect = element => {const r=element.getBoundingClientRect(); return {left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height};};
const checkbox = row.querySelector('#select-row-1');
const checkboxTarget = checkbox.labels?.[0] || checkbox;
const parts = [['checkbox',checkboxTarget],['identity',row.querySelector('.service-cell strong')],['description',row.querySelector('.service-cell > span:not(.unit-label):not(.status-badge)')],['unit',row.querySelector('.unit-label')],['edit',row.querySelector('#edit-row-1')]];
const clip = scroller.getBoundingClientRect();
const left = Math.max(0,clip.left + scroller.clientLeft);
const right = Math.min(innerWidth,clip.left + scroller.clientLeft + scroller.clientWidth);
return {viewport:innerWidth,scrollLeft:scroller.scrollLeft,clip:{left,right},parts:parts.map(([name,element])=>{
  if(!element) throw new Error('Missing control '+name);
  const bounds=rect(element);
  return {name,tag:element.tagName,id:element.id,bounds,horizontallyContained:bounds.left>=left-1&&bounds.right<=right+1,text:element.innerText,clientWidth:element.clientWidth,scrollWidth:element.scrollWidth};
})};
"#).await?;
    write_json(
        &h.evidence.join(format!("forms-grid-geometry-{width}.json")),
        &geometry,
    )?;
    assert_eq!(geometry["viewport"], width);
    assert_eq!(
        geometry["scrollLeft"], 0,
        "Initial identity and actions must be readable without first scrolling horizontally"
    );
    for part in geometry["parts"]
        .as_array()
        .ok_or("First-row geometry absent")?
    {
        assert_eq!(
            part["horizontallyContained"], true,
            "Essential row control is clipped at {width}px: {part}"
        );
        if matches!(part["name"].as_str(), Some("checkbox" | "edit")) {
            assert!(
                part["bounds"]["width"]
                    .as_f64()
                    .ok_or("Target width absent")?
                    >= 44.0,
                "The clickable target must be at least 44 px wide: {part}"
            );
            assert!(
                part["bounds"]["height"]
                    .as_f64()
                    .ok_or("Target height absent")?
                    >= 44.0,
                "The clickable target must be at least 44 px high: {part}"
            );
        } else {
            assert!(!part["text"].as_str().unwrap_or_default().trim().is_empty());
            assert!(
                part["scrollWidth"]
                    .as_u64()
                    .ok_or("Text scroll width absent")?
                    <= part["clientWidth"]
                        .as_u64()
                        .ok_or("Text client width absent")?
                        + 1,
                "Essential identity text must fit its cell: {part}"
            );
        }
    }
    Ok(geometry)
}

async fn scroll_first_edit_into_pointer_view(h: &Harness, browser: &Browser) -> TestResult {
    // The inspected ChromeDriver 156.0.8078.80 GET_LOCATION_IN_VIEW atom
    // writes scrollTop/scrollLeft directly and ignores CSS scroll margins.
    // Prepare this offscreen pointer target with actual wheel input. The
    // independent Tab/Shift+Tab regression below gets no such preparation.
    let path = h.evidence.join("forms-pointer-scroll.json");
    let mut states = vec![focused_grid_button_geometry(browser).await?];
    let mut wheels = Vec::new();
    let method = "Native WebDriver wheel input prepares the pointer target after clearing selection. DOM scripts only observe bounds, focus, scroll offsets and hit-testing. The inspected ChromeDriver 156.0.8078.80 GET_LOCATION_IN_VIEW atom scrolls by direct offsets without consuming CSS scroll-padding/scroll-margin; this describes the inspected driver atom only; the separate keyboard regression verifies the application behavior. The original WebDriver element click remains after the visibility assertion. The separate Tab/Shift+Tab regression has no wheel preparation.";
    for _ in 0..8 {
        let current = states.last().ok_or("Pointer geometry absent")?;
        write_json(
            &path,
            &json!({"status":"observed","method":method,"versions":browser.versions,"states":states,"wheelActions":wheels}),
        )?;
        if current["fullyVisible"] == true && current["hitIsButton"] == true {
            break;
        }
        let origin = &current["wheelOrigin"];
        let x = origin["x"]
            .as_f64()
            .ok_or("No visible grid area for native wheel input")?
            .round() as i64;
        let y = origin["y"]
            .as_f64()
            .ok_or("No visible grid area for native wheel input")?
            .round() as i64;
        let top = current["bounds"]["top"]
            .as_f64()
            .ok_or("Pointer target top absent")?;
        let bottom = current["bounds"]["bottom"]
            .as_f64()
            .ok_or("Pointer target bottom absent")?;
        let clip_top = current["clip"]["top"]
            .as_f64()
            .ok_or("Grid visible top absent")?;
        let clip_bottom = current["clip"]["bottom"]
            .as_f64()
            .ok_or("Grid visible bottom absent")?;
        let delta = if top < clip_top {
            (top - clip_top - 16.0).floor().max(-640.0)
        } else if bottom > clip_bottom {
            (bottom - clip_bottom + 16.0).ceil().min(640.0)
        } else {
            return Err(format!(
                "Pointer target is obstructed despite fitting the visible grid: {current}"
            )
            .into());
        };
        let wheel = json!({"type":"scroll","origin":"viewport","x":x,"y":y,"deltaX":0,"deltaY":delta as i64,"duration":200});
        wheels.push(wheel.clone());
        browser
            .post(
                "actions",
                json!({"actions":[{"type":"wheel","id":"forms-grid-wheel","actions":[wheel]}]}),
            )
            .await?;
        states.push(focused_grid_button_geometry(browser).await?);
    }
    let after = states.last().ok_or("Final pointer geometry absent")?;
    write_json(
        &path,
        &json!({"status":"observed","method":method,"versions":browser.versions,"states":states,"wheelActions":wheels}),
    )?;
    assert_eq!(
        after["fullyVisible"], true,
        "Native wheel input must reveal the whole button below the sticky header before the original click: {after}"
    );
    assert_eq!(
        after["hitIsButton"], true,
        "The button center must receive the pointer before the original click: {after}"
    );
    write_json(
        &path,
        &json!({"status":"passed","method":method,"versions":browser.versions,"states":states,"wheelActions":wheels}),
    )
}

async fn sticky_grid_controls_remain_reachable(h: &Harness, browser: &Browser) -> TestResult {
    let mut cases = Vec::new();
    for width in [1280, 360] {
        browser.viewport(width, 900, false).await?;
        open(h, browser).await?;
        tab_to(browser, "#edit-row-25").await?;
        let scrolled=browser.execute("return {scrollTop:document.querySelector('.measurement-scroll')?.scrollTop,focus:document.activeElement?.id};").await?;
        assert_eq!(scrolled["focus"], "edit-row-25");
        assert!(
            scrolled["scrollTop"]
                .as_f64()
                .ok_or("Grid scroll offset absent")?
                > 0.0,
            "This regression must actually scroll the grid"
        );
        let mut returned = false;
        for _ in 0..100 {
            browser.chord(&[SHIFT], TAB).await?;
            if browser
                .execute("return document.activeElement?.id === 'edit-row-1';")
                .await?
                == true
            {
                returned = true;
                break;
            }
        }
        assert!(
            returned,
            "The first edit action must remain reachable by reverse Tab"
        );
        let geometry = focused_grid_button_geometry(browser).await?;
        write_json(
            &h.evidence.join(format!("forms-sticky-edit-{width}.json")),
            &json!({"status":"observed","scrolled":scrolled,"returned":geometry}),
        )?;
        assert_focused_grid_button(&geometry, true)?;
        browser
            .screenshot(&h.evidence.join(format!("forms-sticky-edit-{width}.png")))
            .await?;
        // Keep the actual click. A keyboard-only activation would not detect
        // the sticky header intercepting a pointer click at the button center.
        browser.click("#edit-row-1").await?;
        browser
            .wait(
                "return !!document.querySelector('#form-current');",
                Duration::from_secs(5),
            )
            .await?;
        browser.click("#form-cancel").await?;
        browser.wait("return !document.querySelector('#form-current') && document.activeElement?.id === 'edit-row-1';",Duration::from_secs(5)).await?;
        let restored = focused_grid_button_geometry(browser).await?;
        assert_focused_grid_button(&restored, false)?;
        let case = json!({"status":"passed","width":width,"scrolled":scrolled,"returned":geometry,"afterEditorReturn":restored});
        write_json(
            &h.evidence.join(format!("forms-sticky-edit-{width}.json")),
            &case,
        )?;
        cases.push(case);
    }
    write_json(
        &h.evidence.join("forms-sticky-controls.json"),
        &json!({"status":"passed","cases":cases,"method":"WebDriver Tab to row 25, reverse Tab to row 1, DOM hit-testing without mutation, real WebDriver click, then return from editor. This keyboard regression uses no wheel preparation. The earlier pointer journey separately records its native wheel preparation and preserves the original WebDriver element click."}),
    )
}

async fn focused_grid_button_geometry(browser: &Browser) -> TestResult<Value> {
    browser.post("execute/async",json!({"script":r#"
const done=arguments[arguments.length-1];
requestAnimationFrame(()=>requestAnimationFrame(()=>{
  const button=document.querySelector('#edit-row-1');
  const scroller=document.querySelector('.measurement-scroll');
  const heading=document.querySelector('.service-heading');
  const r=button.getBoundingClientRect(), s=scroller.getBoundingClientRect(), h=heading.getBoundingClientRect();
  const x=r.left+r.width/2, y=r.top+r.height/2, hit=document.elementFromPoint(x,y);
  const clip={left:Math.max(0,s.left+scroller.clientLeft),right:Math.min(innerWidth,s.left+scroller.clientLeft+scroller.clientWidth),top:Math.max(0,s.top+scroller.clientTop,h.bottom),bottom:Math.min(innerHeight,s.top+scroller.clientTop+scroller.clientHeight)};
  const wheelOrigin=clip.right-clip.left>=24&&clip.bottom-clip.top>=24?{x:(clip.left+clip.right)/2,y:(clip.top+clip.bottom)/2}:null;
  done({focus:document.activeElement?.id,outlineWidth:parseFloat(getComputedStyle(button).outlineWidth),bounds:{left:r.left,right:r.right,top:r.top,bottom:r.bottom,width:r.width,height:r.height},clip,wheelOrigin,viewport:{width:innerWidth,height:innerHeight},scroller:{left:s.left,right:s.right,top:s.top,bottom:s.bottom},headerBottom:h.bottom,scrollTop:scroller.scrollTop,scrollLeft:scroller.scrollLeft,hit:{tag:hit?.tagName,id:hit?.id,classes:hit?.className},hitIsButton:hit===button||button.contains(hit),fullyVisible:r.left>=clip.left-1&&r.right<=clip.right+1&&r.top>=clip.top-1&&r.bottom<=clip.bottom+1});
}));
"#,"args":[]})).await
}

fn assert_focused_grid_button(geometry: &Value, keyboard_focus: bool) -> TestResult {
    assert_eq!(
        geometry["focus"], "edit-row-1",
        "Focus must identify the reached edit action: {geometry}"
    );
    assert_eq!(
        geometry["fullyVisible"], true,
        "The focused edit action must not be clipped by the viewport, scroller or sticky heading: {geometry}"
    );
    assert_eq!(
        geometry["hitIsButton"], true,
        "The sticky heading must not intercept the button center: {geometry}"
    );
    if keyboard_focus {
        assert!(
            geometry["outlineWidth"]
                .as_f64()
                .ok_or("Focus outline absent")?
                >= 3.0,
            "Keyboard focus needs its visible outline: {geometry}"
        );
    }
    Ok(())
}
