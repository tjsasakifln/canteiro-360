//! Browser acceptance uses the W3C WebDriver protocol and Chrome's documented
//! diagnostic endpoint. JavaScript strings below only observe the test page;
//! they are never compiled into, served by, or copied to the product.

use base64::Engine;
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
static BROWSER_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const TAB: &str = "\u{e004}";
const ENTER: &str = "\u{e007}";
const TOGGLE: &str = "#foundation-details-toggle";
const READY: &str =
    "return document.querySelector('.interaction-status')?.dataset.ready === 'true';";

#[tokio::test]
#[ignore = "Requires cargo xtask build, Chromium and matching ChromeDriver"]
async fn rendered_page_hydrates_and_recovers_with_keyboard() -> TestResult {
    let _serial = BROWSER_TEST_LOCK.lock().await;
    let harness = Harness::start("browser-functional").await?;
    let browser = Browser::start(&harness).await?;
    let result = functional_cases(&harness, &browser).await;
    let closing = browser.close().await;
    result?;
    closing?;
    Ok(())
}

#[tokio::test]
#[ignore = "Runs 80 measured navigations under CPU/network throttling"]
async fn initial_route_performance_desktop_and_mobile() -> TestResult {
    let _serial = BROWSER_TEST_LOCK.lock().await;
    let harness = Harness::start("browser-performance").await?;
    let browser = Browser::start(&harness).await?;
    let result = performance_cases(&harness, &browser).await;
    let closing = browser.close().await;
    result?;
    closing?;
    Ok(())
}

async fn functional_cases(h: &Harness, browser: &Browser) -> TestResult {
    let response = h.http.get(&h.app_url).send().await?;
    assert_eq!(response.status(), 200, "SSR route must return HTTP 200");
    let html = response.text().await?;
    assert!(html.contains("Cada etapa da obra, com clareza."));
    assert!(html.contains("Ainda não está liberado para operação comercial"));
    assert!(!html.contains("type=\"password\""));
    fs::write(h.evidence.join("ssr.html"), &html)?;

    browser.viewport(1366, 768, false).await?;
    browser.cdp("Network.enable", json!({})).await?;
    browser
        .cdp("Network.setCacheDisabled", json!({"cacheDisabled": true}))
        .await?;
    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value": true}),
        )
        .await?;
    browser.navigate(&h.app_url).await?;
    assert_eq!(
        browser.text("h1").await?,
        "Cada etapa da obra, com clareza."
    );
    assert!(
        !browser.enabled(TOGGLE).await?,
        "SSR button must be disabled"
    );
    browser
        .screenshot(&h.evidence.join("ssr-without-scripts.png"))
        .await?;

    browser
        .cdp(
            "Emulation.setScriptExecutionDisabled",
            json!({"value": false}),
        )
        .await?;
    let hydration_started = Instant::now();
    browser.navigate(&h.app_url).await?;
    browser.wait(READY, Duration::from_secs(20)).await?;
    let hydration_wall_ms = hydration_started.elapsed().as_secs_f64() * 1000.0;
    assert!(browser.enabled(TOGGLE).await?);
    assert_eq!(
        browser.attribute(TOGGLE, "aria-expanded").await?,
        json!("false")
    );

    browser.tab_to(TOGGLE).await?;
    let focus = browser.execute(
        "const e=document.activeElement;const r=e.getBoundingClientRect();const s=getComputedStyle(e);return {id:e.id,outline:s.outlineWidth,width:r.width,height:r.height,top:r.top,bottom:r.bottom,viewport:innerHeight};",
    ).await?;
    assert_eq!(focus["outline"], "3px", "Keyboard focus must be visible");
    assert!(number(&focus, "height")? >= 44.0);
    assert!(number(&focus, "width")? >= 44.0);
    assert!(number(&focus, "top")? >= 0.0);
    assert!(number(&focus, "bottom")? <= number(&focus, "viewport")?);

    browser.key(ENTER).await?;
    browser.wait("return document.querySelector('#foundation-details-toggle').getAttribute('aria-expanded') === 'true';", Duration::from_secs(3)).await?;
    assert_eq!(
        browser.attribute("#foundation-details", "hidden").await?,
        Value::Null
    );
    browser
        .screenshot(&h.evidence.join("desktop-expanded.png"))
        .await?;
    browser.key(" ").await?;
    browser.wait("return document.querySelector('#foundation-details-toggle').getAttribute('aria-expanded') === 'false';", Duration::from_secs(3)).await?;
    browser.key(ENTER).await?;

    let mut viewports = Vec::new();
    for width in [360, 320] {
        browser.viewport(width, 800, false).await?;
        let dimensions = browser.execute(
            "return {viewport:innerWidth,document:document.documentElement.scrollWidth,body:document.body.scrollWidth,buttonHeight:document.querySelector('#foundation-details-toggle').getBoundingClientRect().height};",
        ).await?;
        assert!(number(&dimensions, "document")? <= number(&dimensions, "viewport")?);
        assert!(number(&dimensions, "body")? <= number(&dimensions, "viewport")?);
        assert!(number(&dimensions, "buttonHeight")? >= 44.0);
        browser
            .screenshot(&h.evidence.join(format!("mobile-{width}-expanded.png")))
            .await?;
        viewports.push(dimensions);
    }
    let normal_logs = browser.logs().await?;
    assert_clean_console(&normal_logs)?;

    let missing = h
        .http
        .get(format!("{}/endereco-inexistente", h.app_url))
        .send()
        .await?;
    assert_eq!(missing.status(), 404);
    assert!(
        missing
            .text()
            .await?
            .contains("Este endereço não está disponível.")
    );

    // Only the disposable copy is changed; the original release artifacts stay intact.
    let mut removed = Vec::new();
    for name in ["canteiro-web_bg.wasm", "canteiro-web_bg.wasm.br"] {
        let path = h.site.join("pkg").join(name);
        if path.is_file() {
            removed.push((path.clone(), fs::read(&path)?));
            fs::remove_file(path)?;
        }
    }
    assert!(
        !removed.is_empty(),
        "The negative case must really remove WASM"
    );
    browser.cdp("Network.clearBrowserCache", json!({})).await?;
    browser.navigate(&h.app_url).await?;
    assert_eq!(
        browser.text("h1").await?,
        "Cada etapa da obra, com clareza."
    );
    assert!(!browser.enabled(TOGGLE).await?);
    tokio::time::sleep(Duration::from_millis(10_300)).await;
    assert_eq!(
        browser
            .execute("return getComputedStyle(document.querySelector('.delayed-help')).visibility;")
            .await?,
        "visible"
    );
    browser.tab_to(".retry-link").await?;
    assert_eq!(
        browser
            .execute("return getComputedStyle(document.activeElement).outlineWidth;")
            .await?,
        "3px"
    );
    browser
        .screenshot(&h.evidence.join("wasm-unavailable.png"))
        .await?;
    let failure_logs = browser.logs().await?;
    assert!(
        failure_logs
            .as_array()
            .is_some_and(|entries| entries.iter().any(|entry| {
                let message = entry["message"].as_str().unwrap_or_default();
                message.contains("wasm") || message.contains("WebAssembly")
            })),
        "The failed WASM load must be visible in browser diagnostics"
    );
    assert_no_hydration_mismatch(&failure_logs)?;

    for (path, bytes) in removed {
        fs::write(path, bytes)?;
    }
    browser.key(ENTER).await?;
    browser.wait(READY, Duration::from_secs(20)).await?;
    assert!(browser.enabled(TOGGLE).await?);
    assert_eq!(browser.attribute(TOGGLE, "aria-expanded").await?, "false");
    browser
        .screenshot(&h.evidence.join("keyboard-recovery.png"))
        .await?;
    let recovery_logs = browser.logs().await?;
    assert_clean_console(&recovery_logs)?;

    write_json(
        &h.evidence.join("browser-functional.json"),
        &json!({
            "status": "passed",
            "versions": browser.versions,
            "environment": h.environment,
            "checks": ["useful_ssr_without_scripts", "hydration_without_mismatch", "keyboard_enter_space", "focus_3px", "touch_target_44px", "reflow_320_360", "http_404", "missing_wasm_delayed_guidance", "keyboard_retry_recovers"],
            "hydration_navigation_wall_ms": hydration_wall_ms,
            "focus": focus,
            "viewports": viewports,
            "normal_console": normal_logs,
            "expected_missing_wasm_console": failure_logs,
            "recovery_console": recovery_logs,
            "screen_reader": "Not executed; semantic DOM and keyboard only."
        }),
    )?;
    println!(
        "Functional browser acceptance passed; evidence: {}",
        h.evidence.display()
    );
    Ok(())
}

// Observers record lab measurements only. They do not implement product behavior.
const OBSERVE_PERFORMANCE: &str = r#"(() => {
  const m = globalThis.__canteiroLab = {lcpMs:0, cls:0, hydratedAtMs:null, interactionPaintMs:null, eventDurations:[], observerErrors:[]};
  let shiftStart=0, shiftLast=0, shiftValue=0;
  const observe = (type, callback, extra={}) => {
    try { new PerformanceObserver(list => list.getEntries().forEach(callback)).observe({type, buffered:true, ...extra}); }
    catch(error) {m.observerErrors.push(type + ': ' + error.message);}
  };
  observe('largest-contentful-paint', entry => {m.lcpMs=entry.startTime;});
  observe('layout-shift', entry => {
    if(entry.hadRecentInput) return;
    if(entry.startTime-shiftLast > 1000 || entry.startTime-shiftStart > 5000) {shiftStart=entry.startTime;shiftValue=0;}
    shiftLast=entry.startTime;shiftValue+=entry.value;m.cls=Math.max(m.cls,shiftValue);
  });
  observe('event', entry => {m.eventDurations.push({name:entry.name,duration:entry.duration,interactionId:entry.interactionId});}, {durationThreshold:16});
  const ready = () => {if(m.hydratedAtMs === null && document.querySelector('.interaction-status')?.dataset.ready === 'true') m.hydratedAtMs=performance.now();};
  new MutationObserver(ready).observe(document, {subtree:true,childList:true,attributes:true,attributeFilter:['data-ready']});
  document.addEventListener('click', event => {
    if(event.target.closest?.('#foundation-details-toggle')) {
      const start=event.timeStamp;
      requestAnimationFrame(() => requestAnimationFrame(() => {m.interactionPaintMs=performance.now()-start;}));
    }
  }, true);
})();"#;

async fn performance_cases(h: &Harness, browser: &Browser) -> TestResult {
    browser.cdp("Network.enable", json!({})).await?;
    browser.cdp("Performance.enable", json!({})).await?;
    browser
        .cdp("Emulation.setCPUThrottlingRate", json!({"rate": 4}))
        .await?;
    browser
        .cdp(
            "Network.emulateNetworkConditions",
            json!({
                "offline": false, "latency": 150, "downloadThroughput": 500_000,
                "uploadThroughput": 125_000, "connectionType": "cellular4g"
            }),
        )
        .await?;
    browser
        .cdp(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({"source": OBSERVE_PERFORMANCE}),
        )
        .await?;

    let mut samples = Vec::new();
    let mut summaries = Vec::new();
    for (profile, width, height, mobile) in
        [("desktop", 1366, 768, false), ("mobile", 360, 800, true)]
    {
        browser.viewport(width, height, mobile).await?;
        for cache in ["cold", "warm"] {
            let cold = cache == "cold";
            browser
                .cdp("Network.setCacheDisabled", json!({"cacheDisabled": cold}))
                .await?;
            if !cold {
                browser.navigate(&h.app_url).await?;
                browser.wait(READY, Duration::from_secs(25)).await?;
                assert_clean_console(&browser.logs().await?)?;
            }
            let mut group = Vec::new();
            for run in 1..=20 {
                if cold {
                    browser.cdp("Network.clearBrowserCache", json!({})).await?;
                }
                let started = Instant::now();
                browser.navigate(&h.app_url).await?;
                browser.wait(READY, Duration::from_secs(25)).await?;
                let navigation_wall_ms = started.elapsed().as_secs_f64() * 1000.0;
                tokio::time::sleep(Duration::from_millis(250)).await;
                browser.click(TOGGLE).await?;
                browser
                    .wait(
                        "return Number.isFinite(globalThis.__canteiroLab?.interactionPaintMs);",
                        Duration::from_secs(3),
                    )
                    .await?;
                let metrics = browser.execute(r#"
                    const n=performance.getEntriesByType('navigation')[0];
                    return {...globalThis.__canteiroLab,
                      navigation:{ttfbMs:n.responseStart,domContentLoadedMs:n.domContentLoadedEventEnd,loadMs:n.loadEventEnd},
                      resources:performance.getEntriesByType('resource').map(r=>({name:new URL(r.name).pathname,transferBytes:r.transferSize,encodedBytes:r.encodedBodySize,decodedBytes:r.decodedBodySize,durationMs:r.duration})),
                      javascriptHeap:performance.memory ? {usedBytes:performance.memory.usedJSHeapSize,totalBytes:performance.memory.totalJSHeapSize,limitBytes:performance.memory.jsHeapSizeLimit} : null,
                      viewport:{width:innerWidth,height:innerHeight},
                      expanded:document.querySelector('#foundation-details-toggle').getAttribute('aria-expanded')
                    };
                "#).await?;
                assert_eq!(metrics["expanded"], "true");
                assert!(number(&metrics, "hydratedAtMs")? > 0.0);
                assert!(
                    number(&metrics, "lcpMs")? > 0.0,
                    "LCP must be measured, not default to zero"
                );
                assert!(
                    metrics["observerErrors"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
                );
                let runtime = browser.cdp("Performance.getMetrics", json!({})).await?;
                assert_clean_console(&browser.logs().await?)?;
                let sample = json!({"profile":profile,"cache":cache,"run":run,"navigationWallMs":navigation_wall_ms,"metrics":metrics,"runtimeMetrics":runtime});
                group.push(sample.clone());
                samples.push(sample);
                // Preserve partial evidence if a later sample or acceptance threshold fails.
                write_json(
                    &h.evidence.join("performance-samples.json"),
                    &json!({"versions":browser.versions,"samples":samples}),
                )?;
            }
            let summary = json!({
                "profile": profile, "cache": cache, "n": group.len(),
                "lcpP75Ms": percentile(&group, "lcpMs", 0.75)?,
                "clsP75": percentile(&group, "cls", 0.75)?,
                "hydrationP75Ms": percentile(&group, "hydratedAtMs", 0.75)?,
                "controlledInteractionP75Ms": percentile(&group, "interactionPaintMs", 0.75)?
            });
            println!("Lab result: {summary}");
            summaries.push(summary);
        }
    }
    let mut report = json!({
        "status": "measured",
        "versions": browser.versions,
        "environment": h.environment,
        "scenario": {"cpuSlowdown":4,"downloadMbps":4,"uploadMbps":1,"rttMs":150,"navigationsPerProfileAndCache":20,"totalNavigations":samples.len(),"warmupNavigations":2},
        "cacheNote": "Cold disables and clears the HTTP cache before every navigation. Warm enables cache after one unmeasured navigation. The response cache policy still applies; inspect resource transfer sizes before claiming cache hits.",
        "memoryNote": "JavaScript heap and CDP runtime counters, not whole-process RSS or total WASM memory.",
        "interactionNote": "Controlled click to two animation frames is a conservative lab proxy, not real-user INP. Event Timing entries are included when above the browser threshold.",
        "physicalDevice": "Not executed; desktop and mobile are Chromium viewport emulations.",
        "summaries": summaries,
        "samplesFile": "performance-samples.json"
    });
    write_json(&h.evidence.join("browser-performance.json"), &report)?;
    for summary in &summaries {
        assert!(
            number(summary, "lcpP75Ms")? <= 2500.0,
            "LCP budget exceeded: {summary}"
        );
        assert!(
            number(summary, "clsP75")? <= 0.1,
            "CLS budget exceeded: {summary}"
        );
        assert!(
            number(summary, "controlledInteractionP75Ms")? <= 100.0,
            "Local interaction budget exceeded: {summary}"
        );
    }
    report["status"] = json!("passed");
    write_json(&h.evidence.join("browser-performance.json"), &report)?;
    println!(
        "Performance acceptance passed; evidence: {}",
        h.evidence.display()
    );
    Ok(())
}

fn percentile(samples: &[Value], metric: &str, fraction: f64) -> TestResult<f64> {
    let mut values: Vec<_> = samples
        .iter()
        .map(|sample| number(&sample["metrics"], metric))
        .collect::<TestResult<_>>()?;
    values.sort_by(f64::total_cmp);
    let index = ((values.len() as f64 * fraction).ceil() as usize).saturating_sub(1);
    values
        .get(index)
        .copied()
        .ok_or_else(|| "No measurements for percentile".into())
}

fn number(value: &Value, name: &str) -> TestResult<f64> {
    value[name]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("Missing numeric measurement {name}: {value}").into())
}

fn assert_no_hydration_mismatch(logs: &Value) -> TestResult {
    for entry in logs
        .as_array()
        .ok_or("Browser log did not return an array")?
    {
        let text = entry["message"].as_str().unwrap_or_default().to_lowercase();
        if text.contains("hydration")
            && (text.contains("mismatch") || text.contains("failed") || text.contains("error"))
        {
            return Err(format!("Hydration divergence: {entry}").into());
        }
    }
    Ok(())
}

fn assert_clean_console(logs: &Value) -> TestResult {
    assert_no_hydration_mismatch(logs)?;
    for entry in logs
        .as_array()
        .ok_or("Browser log did not return an array")?
    {
        if entry["level"] == "SEVERE" {
            return Err(format!("Unexpected browser error: {entry}").into());
        }
    }
    Ok(())
}

struct Harness {
    _api: ChildGuard,
    _driver: ChildGuard,
    _temporary: TempDirectory,
    site: PathBuf,
    evidence: PathBuf,
    app_url: String,
    driver_url: String,
    browser_bin: PathBuf,
    http: Client,
    environment: Value,
}

impl Harness {
    async fn start(name: &str) -> TestResult<Self> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()?;
        let evidence = root.join("target/evidence");
        fs::create_dir_all(&evidence)?;
        let started_at_unix_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
        // Never leave a previous successful report looking like the current run.
        write_json(
            &evidence.join(format!("{name}.json")),
            &json!({"status":"running","startedAtUnixMs":started_at_unix_ms}),
        )?;
        if name == "browser-performance" {
            write_json(
                &evidence.join("performance-samples.json"),
                &json!({"status":"running","startedAtUnixMs":started_at_unix_ms,"samples":[]}),
            )?;
        }
        let api_bin = std::env::var_os("CANTEIRO_API_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("target/release/canteiro-api"));
        if !api_bin.is_file() {
            return Err(format!(
                "Missing {}. Run cargo xtask build; this test never skips missing prerequisites.",
                api_bin.display()
            )
            .into());
        }
        let driver_bin = find_executable("CANTEIRO_WEBDRIVER_BIN", "chromedriver")?;
        let browser_bin = find_executable("CANTEIRO_BROWSER_BIN", "chromium")?;
        let temporary = TempDirectory::new(name)?;
        let site = temporary.0.join("site");
        copy_directory(&root.join("target/site"), &site)?;
        if !site.join("pkg/canteiro-web_bg.wasm").is_file() {
            return Err("Missing release WASM in target/site; run cargo xtask build".into());
        }
        let app_port = free_port()?;
        let driver_port = free_port()?;
        let mut api_command = Command::new(&api_bin);
        api_command
            .current_dir(&root)
            .env("CANTEIRO_ENV", "development")
            .env("CANTEIRO_SITE_ROOT", &site)
            .env("CANTEIRO_BIND_ADDR", format!("127.0.0.1:{app_port}"));
        let api = ChildGuard::start(api_command, &evidence.join(format!("{name}-server.log")))?;
        let mut driver_command = Command::new(&driver_bin);
        driver_command.args([
            format!("--port={driver_port}"),
            "--allowed-ips=127.0.0.1".into(),
        ]);
        let driver =
            ChildGuard::start(driver_command, &evidence.join(format!("{name}-driver.log")))?;
        let http = Client::builder()
            .timeout(Duration::from_secs(45))
            .no_proxy()
            .build()?;
        let app_url = format!("http://127.0.0.1:{app_port}");
        let driver_url = format!("http://127.0.0.1:{driver_port}");
        wait_http(&http, &format!("{app_url}/healthz")).await?;
        wait_http(&http, &format!("{driver_url}/status")).await?;
        let environment = json!({
            "startedAtUnixMs":started_at_unix_ms,
            "os":std::env::consts::OS,"architecture":std::env::consts::ARCH,
            "availableParallelism":std::thread::available_parallelism()?.get(),
            "cpu":fs::read_to_string("/proc/cpuinfo").ok().and_then(|info| info.lines().find(|line| line.starts_with("model name")).map(str::to_string)),
            "memory":fs::read_to_string("/proc/meminfo").ok().and_then(|info| info.lines().find(|line| line.starts_with("MemTotal")).map(str::to_string)),
            "browserCommandVersion":command_version(&browser_bin)?,
            "driverCommandVersion":command_version(&driver_bin)?,
            "apiBinaryBytes":fs::metadata(&api_bin)?.len(),
            "wasmBinaryBytes":fs::metadata(site.join("pkg/canteiro-web_bg.wasm"))?.len(),
            "executorNote":"Shared execution environment; this record does not assert exclusive CPU allocation or physical mobile testing."
        });
        Ok(Self {
            _api: api,
            _driver: driver,
            _temporary: temporary,
            site,
            evidence,
            app_url,
            driver_url,
            browser_bin,
            http,
            environment,
        })
    }
}

struct Browser {
    http: Client,
    url: String,
    versions: Value,
}

impl Browser {
    async fn start(h: &Harness) -> TestResult<Self> {
        let payload = json!({"capabilities":{"alwaysMatch":{
            "browserName":"chrome", "pageLoadStrategy":"normal",
            "goog:chromeOptions":{"binary":h.browser_bin,"args":["--headless=new","--no-sandbox","--disable-dev-shm-usage","--no-first-run","--no-default-browser-check","--window-size=1366,768"]},
            "goog:loggingPrefs":{"browser":"ALL"}
        }}});
        let value = decode_response(
            h.http
                .post(format!("{}/session", h.driver_url))
                .json(&payload)
                .send()
                .await?,
        )
        .await?;
        let id = value["sessionId"]
            .as_str()
            .ok_or("WebDriver did not create a session")?;
        let capabilities = &value["capabilities"];
        let browser_version = capabilities["browserVersion"]
            .as_str()
            .ok_or("Browser version absent")?;
        let driver_version = capabilities["chrome"]["chromedriverVersion"]
            .as_str()
            .ok_or("ChromeDriver version absent")?;
        let browser_build = browser_version.split('.').take(3).collect::<Vec<_>>();
        let driver_build = driver_version.split('.').take(3).collect::<Vec<_>>();
        if browser_build != driver_build {
            return Err(format!(
                "Incompatible browser {browser_version} and ChromeDriver {driver_version}"
            )
            .into());
        }
        if let Ok(expected) = std::env::var("CANTEIRO_EXPECT_BROWSER_VERSION") {
            assert_eq!(browser_version, expected, "Unexpected browser version");
        }
        let browser = Self {
            http: h.http.clone(),
            url: format!("{}/session/{id}", h.driver_url),
            versions: json!({"browser":browser_version,"driver":driver_version,"platform":capabilities["platformName"]}),
        };
        browser
            .post(
                "timeouts",
                json!({"script":30_000,"pageLoad":30_000,"implicit":0}),
            )
            .await?;
        Ok(browser)
    }

    async fn request(&self, method: Method, path: &str, body: Option<Value>) -> TestResult<Value> {
        let mut request = self.http.request(method, format!("{}/{}", self.url, path));
        if let Some(body) = body {
            request = request.json(&body);
        }
        decode_response(request.send().await?).await
    }

    async fn post(&self, path: &str, body: Value) -> TestResult<Value> {
        self.request(Method::POST, path, Some(body)).await
    }

    async fn cdp(&self, command: &str, params: Value) -> TestResult<Value> {
        self.post("goog/cdp/execute", json!({"cmd":command,"params":params}))
            .await
    }

    async fn execute(&self, script: &str) -> TestResult<Value> {
        self.post("execute/sync", json!({"script":script,"args":[]}))
            .await
    }

    async fn navigate(&self, url: &str) -> TestResult {
        self.post("url", json!({"url":url})).await?;
        Ok(())
    }

    async fn viewport(&self, width: u32, height: u32, mobile: bool) -> TestResult {
        self.cdp(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":height,"deviceScaleFactor":1,"mobile":mobile}),
        )
        .await?;
        Ok(())
    }

    async fn element(&self, selector: &str) -> TestResult<String> {
        let value = self
            .post("element", json!({"using":"css selector","value":selector}))
            .await?;
        value["element-6066-11e4-a52e-4f735466cecf"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("Element not found: {selector}").into())
    }

    async fn text(&self, selector: &str) -> TestResult<String> {
        let id = self.element(selector).await?;
        let value = self
            .request(Method::GET, &format!("element/{id}/text"), None)
            .await?;
        value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "Text response is not a string".into())
    }

    async fn attribute(&self, selector: &str, attribute: &str) -> TestResult<Value> {
        let id = self.element(selector).await?;
        self.request(
            Method::GET,
            &format!("element/{id}/attribute/{attribute}"),
            None,
        )
        .await
    }

    async fn enabled(&self, selector: &str) -> TestResult<bool> {
        let id = self.element(selector).await?;
        self.request(Method::GET, &format!("element/{id}/enabled"), None)
            .await?
            .as_bool()
            .ok_or_else(|| "Enabled response is not a bool".into())
    }

    async fn click(&self, selector: &str) -> TestResult {
        let id = self.element(selector).await?;
        self.post(&format!("element/{id}/click"), json!({})).await?;
        Ok(())
    }

    async fn key(&self, key: &str) -> TestResult {
        self.post("actions",json!({"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":key},{"type":"keyUp","value":key}]}]})).await?;
        Ok(())
    }

    async fn tab_to(&self, selector: &str) -> TestResult {
        let expression = format!(
            "return document.activeElement?.matches({});",
            serde_json::to_string(selector)?
        );
        for _ in 0..12 {
            self.key(TAB).await?;
            if self.execute(&expression).await? == true {
                return Ok(());
            }
        }
        Err(format!("Element not reachable by Tab: {selector}").into())
    }

    async fn wait(&self, expression: &str, timeout: Duration) -> TestResult {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self.execute(expression).await? == true {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(format!("Browser condition timed out after {timeout:?}: {expression}").into())
    }

    async fn screenshot(&self, path: &Path) -> TestResult {
        let value = self.request(Method::GET, "screenshot", None).await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.as_str().ok_or("Screenshot is not base64")?)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    async fn logs(&self) -> TestResult<Value> {
        self.post("log", json!({"type":"browser"})).await
    }

    async fn close(&self) -> TestResult {
        decode_response(self.http.delete(&self.url).send().await?).await?;
        Ok(())
    }
}

async fn decode_response(response: reqwest::Response) -> TestResult<Value> {
    let status = response.status();
    let decoded: Value = response.json().await?;
    let value = decoded.get("value").ok_or("Malformed WebDriver response")?;
    if !status.is_success() || value.get("error").is_some() {
        return Err(format!("WebDriver {status}: {value}").into());
    }
    Ok(value.clone())
}

async fn wait_http(client: &Client, url: &str) -> TestResult {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(15) {
        if let Ok(response) = client.get(url).timeout(Duration::from_secs(1)).send().await
            && response.status().is_success()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(format!("Service did not start: {url}; inspect target/evidence logs").into())
}

fn free_port() -> TestResult<u16> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

fn find_executable(variable: &str, default: &str) -> TestResult<PathBuf> {
    let requested = std::env::var_os(variable).unwrap_or_else(|| default.into());
    let requested = PathBuf::from(requested);
    if requested.components().count() > 1 {
        if requested.is_file() {
            return Ok(requested.canonicalize()?);
        }
    } else if let Some(path) = std::env::var_os("PATH") {
        for folder in std::env::split_paths(&path) {
            let candidate = folder.join(&requested);
            if candidate.is_file() {
                return Ok(candidate.canonicalize()?);
            }
        }
    }
    Err(format!(
        "Required executable {} not found. Set {variable}; this test never skips.",
        requested.display()
    )
    .into())
}

fn command_version(path: &Path) -> TestResult<String> {
    let output = Command::new(path).arg("--version").output()?;
    if !output.status.success() {
        return Err(format!("Cannot query version of {}", path.display()).into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn copy_directory(source: &Path, destination: &Path) -> TestResult {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err("Unexpected non-regular site artifact".into());
        }
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> TestResult {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

struct TempDirectory(PathBuf);
impl TempDirectory {
    fn new(name: &str) -> TestResult<Self> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let path =
            std::env::temp_dir().join(format!("canteiro-{name}-{}-{unique}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct ChildGuard(Child);
impl ChildGuard {
    fn start(mut command: Command, log_path: &Path) -> TestResult<Self> {
        let log = fs::File::create(log_path)?;
        command
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log));
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        Ok(Self(command.spawn()?))
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // The child owns its group, so this also stops a browser left by a failed test.
            let _ = Command::new("kill")
                .args(["-TERM", "--", &format!("-{}", self.0.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
