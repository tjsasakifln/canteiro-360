#[allow(dead_code)]
mod support;

use canteiro_calculation_verification::run_case;
use serde_json::{Value, json};
use std::{fs, time::Duration};
use support::{Browser, Harness, TestResult, write_json};

const REFERENCE: &str = include_str!("../../../docs/validacao/issue-005/reference-vectors.json");

#[tokio::test]
#[ignore = "requires built server, PostgreSQL, generated verifier WASM and WebDriver"]
async fn independent_vectors_match_native_and_real_browser_wasm() -> TestResult {
    let h = Harness::start("calculation-parity").await?;
    let browser = Browser::start(&h).await?;
    browser
        .navigate(&format!("{}/conferencia", h.app_url))
        .await?;
    browser.wait("return document.querySelector('[data-calculation-ready]')?.dataset.calculationReady === 'true'", Duration::from_secs(30)).await?;
    let reference: Value = serde_json::from_str(REFERENCE)?;
    let cases: Vec<_> = reference["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .chain(
            reference["property_samples"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|group| group["samples"].as_array().unwrap()),
        )
        .collect();
    // The expected answers and reference method are deliberately never passed
    // to the WASM module. JavaScript only transports inputs and return values.
    assert_eq!(cases.len(), 74);
    let inputs: Vec<_> = cases
        .iter()
        .map(
            |case| json!({"id":case["id"], "operation":case["operation"], "inputs":case["inputs"]}),
        )
        .collect();
    let wasm = browser.post("execute/async", json!({
        "script": "const done=arguments[arguments.length-1]; const cases=arguments[0]; import('/pkg/verification/calculation-verification.js').then(async m=>{await m.default(); done(cases.map(c=>({id:c.id,result:JSON.parse(m.verify_calculation_vector(JSON.stringify(c)))})));}).catch(e=>done({transport_error:String(e)}));",
        "args":[inputs]
    })).await?;
    assert!(wasm.is_array(), "WASM transport failed: {wasm}");
    let wasm = wasm.as_array().unwrap();
    assert_eq!(wasm.len(), cases.len());
    let mut results = Vec::new();
    for (index, (case, wasm)) in cases.iter().zip(wasm).enumerate() {
        let native = run_case(&inputs[index]);
        assert_eq!(native, case["expected"], "native {}", case["id"]);
        assert_eq!(wasm["id"], case["id"]);
        assert_eq!(wasm["result"], case["expected"], "WASM {}", case["id"]);
        assert_eq!(
            native.to_string(),
            wasm["result"].to_string(),
            "canonical {}",
            case["id"]
        );
        results.push(json!({"id":case["id"],"expected":case["expected"],"native":native,"wasm":wasm["result"],"passed":true}));
    }
    let logs = browser.logs().await?;
    assert_eq!(logs, json!([]), "Browser console: {logs}");
    write_json(
        &h.evidence.join("calculation-parity.json"),
        &json!({
            "status":"passed","policy":"C360-DECIMAL-1","count":results.len(),"results":results,
            "versions":browser.versions,"environment":h.environment,"console":logs,
            "reference":"docs/validacao/issue-005/reference-vectors.json",
            "note":"Golden expectations are external to the executed WASM; generated bridge transports JSON only."
        }),
    )?;
    fs::write(
        h.evidence.join("calculation-parity-reference.json"),
        REFERENCE,
    )?;
    browser.close().await?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires built server, PostgreSQL and WebDriver"]
async fn previews_are_readable_keyboard_operable_and_preserve_invalid_input() -> TestResult {
    let h = Harness::start("calculation-interface").await?;
    let browser = Browser::start(&h).await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp(
            "Network.setBlockedURLs",
            json!({"urls":["*canteiro-web_bg.wasm*"]}),
        )
        .await?;
    browser
        .navigate(&format!("{}/conferencia", h.app_url))
        .await?;
    assert!(browser.text("#allocation-output").await?.contains("33,34"));
    assert!(!browser.enabled("#allocation-total").await?);
    for (selector, value) in [
        ("#allocation-total", "100,00"),
        ("#allocation-count", "3"),
        ("#calendar-start", "31/01/2027"),
        ("#calendar-end", "31/03/2027"),
    ] {
        assert_eq!(browser.property(selector, "value").await?, value);
    }
    browser.wait("return getComputedStyle(document.querySelector('#calculation-retry').parentElement).visibility === 'visible'", Duration::from_secs(20)).await?;
    assert_eq!(
        browser.attribute("#calculation-retry", "href").await?,
        "/conferencia"
    );
    browser
        .full_screenshot(&h.evidence.join("calculation-wasm-unavailable.png"))
        .await?;
    let blocked_logs = browser.logs().await?;
    browser
        .cdp("Network.setBlockedURLs", json!({"urls":[]}))
        .await?;
    let mut layouts = Vec::new();
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/conferencia", h.app_url))
            .await?;
        browser.wait("return document.querySelector('[data-calculation-ready]')?.dataset.calculationReady === 'true'", Duration::from_secs(30)).await?;
        assert!(browser.text("#allocation-output").await?.contains("33,34"));
        assert!(
            browser
                .text("#calendar-output")
                .await?
                .contains("28/02/2027")
        );
        let dimensions = browser.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,height:document.documentElement.scrollHeight,inputs:[...document.querySelectorAll('input,select')].map(e=>({id:e.id,label:!!document.querySelector('label[for=\"'+e.id+'\"]'),height:e.getBoundingClientRect().height}))};").await?;
        assert!(dimensions["scroll"].as_u64().unwrap() <= dimensions["width"].as_u64().unwrap());
        assert!(
            dimensions["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["label"] == true && row["height"].as_f64().unwrap() >= 44.0)
        );
        browser
            .full_screenshot(&h.evidence.join(format!("calculation-{width}.png")))
            .await?;
        // A capture beyond the viewport can omit a composited scroll surface
        // that has never been on screen. Bring each result into the viewport
        // with real keyboard navigation and wheel input, then preserve what a
        // reader sees. Focusing a region alone need not center its entire box.
        for (name, selector) in [
            ("allocation", "#allocation-output .calculation-table"),
            ("calendar", "#calendar-output .calculation-table"),
        ] {
            browser.tab_to(selector).await?;
            let before = browser.execute(&format!(
                "const e=document.querySelector({});const r=e.getBoundingClientRect();return {{top:r.top,bottom:r.bottom,height:innerHeight,text:e.innerText,scrollLeft:e.scrollLeft}};",
                serde_json::to_string(selector)?
            )).await?;
            let delta = ((before["top"].as_f64().unwrap() + before["bottom"].as_f64().unwrap()
                - before["height"].as_f64().unwrap())
                / 2.0)
                .round() as i64;
            browser.post("actions", json!({"actions":[{"type":"wheel","id":"calculation-page-wheel","actions":[{"type":"scroll","origin":"viewport","x":1,"y":450,"deltaX":0,"deltaY":delta,"duration":200}]}]})).await?;
            browser.wait(&format!("const r=document.querySelector({}).getBoundingClientRect();return r.top >= 0 && r.bottom <= innerHeight + 1;", serde_json::to_string(selector)?), Duration::from_secs(5)).await?;
            let bounds = browser.execute(&format!(
                "const e=document.querySelector({});const r=e.getBoundingClientRect();return {{top:r.top,bottom:r.bottom,height:innerHeight,text:e.innerText,scrollLeft:e.scrollLeft}};",
                serde_json::to_string(selector)?
            )).await?;
            assert!(bounds["top"].as_f64().unwrap() >= 0.0);
            assert!(bounds["bottom"].as_f64().unwrap() <= bounds["height"].as_f64().unwrap() + 1.0);
            assert!(
                bounds["text"]
                    .as_str()
                    .unwrap()
                    .contains(if name == "calendar" {
                        "28/02/2027"
                    } else {
                        "33,34"
                    })
            );
            browser
                .screenshot(
                    &h.evidence
                        .join(format!("calculation-{name}-visible-{width}.png")),
                )
                .await?;
            if width == 360 {
                browser.key("\u{e014}").await?;
                browser
                    .wait(
                        &format!(
                            "return document.querySelector({}).scrollLeft > 0;",
                            serde_json::to_string(selector)?
                        ),
                        Duration::from_secs(5),
                    )
                    .await?;
                browser
                    .screenshot(
                        &h.evidence
                            .join(format!("calculation-{name}-scrolled-{width}.png")),
                    )
                    .await?;
            }
        }
        layouts.push(dimensions);
    }
    browser.tab_to("#allocation-total").await?;
    browser.fill("#allocation-total", "100,01").await?;
    browser
        .wait(
            "return document.querySelector('#allocation-output').textContent.includes('100,01')",
            Duration::from_secs(10),
        )
        .await?;
    assert_eq!(browser.execute("return [...document.querySelectorAll('#allocation-output tbody tr')].map(r=>r.children[1].textContent);").await?, json!(["33,34","33,34","33,33"]));
    browser.fill("#allocation-total", "1.234").await?;
    assert_eq!(browser.property("#allocation-count", "value").await?, "3");
    assert_eq!(
        browser.property("#allocation-total", "value").await?,
        "1.234"
    );
    assert_eq!(
        browser
            .attribute("#allocation-total", "aria-invalid")
            .await?,
        "true"
    );
    assert!(
        browser
            .text("#allocation-total-error")
            .await?
            .contains("ambíguo")
    );
    assert!(
        !browser
            .text("#allocation-output")
            .await?
            .contains("Total preservado")
    );
    browser
        .screenshot(&h.evidence.join("calculation-invalid-preserved.png"))
        .await?;
    browser.fill("#allocation-total", "100,00").await?;
    browser.fill("#allocation-count", "0").await?;
    assert_eq!(
        browser
            .attribute("#allocation-count", "aria-invalid")
            .await?,
        "true"
    );
    browser.fill("#allocation-count", "3").await?;
    browser.fill("#calendar-start", "31/01/2028").await?;
    browser.fill("#calendar-end", "31/03/2028").await?;
    assert!(
        browser
            .text("#calendar-output")
            .await?
            .contains("29/02/2028")
    );
    browser.choose("#calendar-policy", 1, "fixed").await?;
    assert!(
        browser
            .text("#calendar-output")
            .await?
            .contains("01/03/2028")
    );
    browser.choose("#calendar-policy", 0, "civil").await?;
    browser.fill("#calendar-start", "31/02/2028").await?;
    assert_eq!(
        browser.attribute("#calendar-start", "aria-invalid").await?,
        "true"
    );
    assert_eq!(
        browser.property("#calendar-end", "value").await?,
        "31/03/2028"
    );
    assert!(
        browser
            .text("#calendar-start-error")
            .await?
            .contains("não existe")
    );
    browser.fill("#calendar-start", "31/01/2028").await?;
    assert!(
        browser
            .text("#calendar-output")
            .await?
            .contains("29/02/2028")
    );
    let logs = browser.logs().await?;
    assert_eq!(logs, json!([]), "Browser console: {logs}");
    write_json(
        &h.evidence.join("calculation-interface.json"),
        &json!({"status":"passed","layouts":layouts,"versions":browser.versions,"console":logs,"deliberately_blocked_wasm_console":blocked_logs,"environment":h.environment,"checks":["SSR defaults remain in all four inputs without WASM","WASM failure keeps readable preview and offers reload","360 and 1280 CSS pixels","keyboard entry","result tables visible after native Tab and wheel input","horizontal table scrolling by arrow key at 360 CSS pixels","ambiguous decimal keeps other fields","invalid date keeps term","leap year","explicit fixed cadence","recovery"]}),
    )?;
    // Save the exact golden file used with the browser observations.
    fs::write(
        h.evidence.join("calculation-reference-vectors.json"),
        REFERENCE,
    )?;
    browser.close().await?;
    Ok(())
}
