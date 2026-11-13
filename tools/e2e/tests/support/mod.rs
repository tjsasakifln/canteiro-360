//! Standalone WebDriver support for prototype acceptance. Product interaction is
//! performed through WebDriver; script execution below is read-only observation.

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

pub type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;
pub const TAB: &str = "\u{e004}";
pub const ENTER: &str = "\u{e007}";
pub const ESCAPE: &str = "\u{e00c}";
pub const HOME: &str = "\u{e011}";
pub const DOWN: &str = "\u{e015}";
pub const SHIFT: &str = "\u{e008}";
pub const ALT: &str = "\u{e00a}";

pub struct Harness {
    _api: ChildGuard,
    _driver: ChildGuard,
    _temporary: TempDirectory,
    pub evidence: PathBuf,
    pub app_url: String,
    driver_url: String,
    browser_bin: PathBuf,
    pub http: Client,
    pub environment: Value,
    #[allow(dead_code)] // Shared harness: only the authentication suite consumes its mail fixture.
    pub auth: canteiro_application::auth::AuthService,
}

impl Harness {
    pub async fn start(name: &str) -> TestResult<Self> {
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
        let auth_key: [u8; 32] = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(std::env::var("CANTEIRO_AUTH_KEY")?)?
            .try_into()
            .map_err(|_| "Inject a 32-byte ephemeral key for the fixture database")?;
        let settings = canteiro_persistence::DatabaseSettings::from_external_url(&std::env::var(
            "CANTEIRO_DATABASE_URL",
        )?)?;
        let database = canteiro_persistence::Database::connect(
            &settings,
            canteiro_persistence::DatabaseEnvironment::Development,
            canteiro_persistence::TransactionLimits::default(),
        )
        .await?;
        let auth = canteiro_application::auth::AuthService::new(database, auth_key).await?;
        api_command
            .current_dir(&root)
            .env("CANTEIRO_ENV", "development")
            .env("CANTEIRO_SITE_ROOT", &site)
            .env(
                "CANTEIRO_AUTH_KEY",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(auth_key),
            )
            .env(
                "CANTEIRO_PUBLIC_ORIGIN",
                format!("http://127.0.0.1:{app_port}"),
            )
            .env_remove("CANTEIRO_AUTH_MAIL_ENDPOINT")
            .env_remove("CANTEIRO_AUTH_MAIL_CREDENTIAL")
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
            evidence,
            app_url,
            driver_url,
            browser_bin,
            http,
            environment,
            auth,
        })
    }
}

pub struct Browser {
    http: Client,
    url: String,
    pub versions: Value,
}

impl Browser {
    pub async fn start(h: &Harness) -> TestResult<Self> {
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

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> TestResult<Value> {
        let mut request = self.http.request(method, format!("{}/{}", self.url, path));
        if let Some(body) = body {
            request = request.json(&body);
        }
        decode_response(request.send().await?).await
    }

    pub async fn post(&self, path: &str, body: Value) -> TestResult<Value> {
        self.request(Method::POST, path, Some(body)).await
    }

    pub async fn cdp(&self, command: &str, params: Value) -> TestResult<Value> {
        self.post("goog/cdp/execute", json!({"cmd":command,"params":params}))
            .await
    }

    pub async fn execute(&self, script: &str) -> TestResult<Value> {
        self.post("execute/sync", json!({"script":script,"args":[]}))
            .await
    }

    pub async fn navigate(&self, url: &str) -> TestResult {
        self.post("url", json!({"url":url})).await?;
        Ok(())
    }

    pub async fn viewport(&self, width: u32, height: u32, mobile: bool) -> TestResult {
        self.cdp(
            "Emulation.setDeviceMetricsOverride",
            json!({"width":width,"height":height,"deviceScaleFactor":1,"mobile":mobile}),
        )
        .await?;
        Ok(())
    }

    pub async fn element(&self, selector: &str) -> TestResult<String> {
        let value = self
            .post("element", json!({"using":"css selector","value":selector}))
            .await?;
        value["element-6066-11e4-a52e-4f735466cecf"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("Element not found: {selector}").into())
    }

    pub async fn text(&self, selector: &str) -> TestResult<String> {
        let id = self.element(selector).await?;
        let value = self
            .request(Method::GET, &format!("element/{id}/text"), None)
            .await?;
        value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| "Text response is not a string".into())
    }

    pub async fn attribute(&self, selector: &str, attribute: &str) -> TestResult<Value> {
        let id = self.element(selector).await?;
        self.request(
            Method::GET,
            &format!("element/{id}/attribute/{attribute}"),
            None,
        )
        .await
    }

    /// W3C attributes reflect markup; live field state must be read as a property.
    pub async fn property(&self, selector: &str, property: &str) -> TestResult<Value> {
        let id = self.element(selector).await?;
        self.request(
            Method::GET,
            &format!("element/{id}/property/{property}"),
            None,
        )
        .await
    }

    pub async fn enabled(&self, selector: &str) -> TestResult<bool> {
        let id = self.element(selector).await?;
        self.request(Method::GET, &format!("element/{id}/enabled"), None)
            .await?
            .as_bool()
            .ok_or_else(|| "Enabled response is not a bool".into())
    }

    pub async fn click(&self, selector: &str) -> TestResult {
        let id = self.element(selector).await?;
        self.post(&format!("element/{id}/click"), json!({})).await?;
        Ok(())
    }

    pub async fn key(&self, key: &str) -> TestResult {
        self.post("actions",json!({"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":key},{"type":"keyUp","value":key}]}]})).await?;
        Ok(())
    }

    pub async fn tab_to(&self, selector: &str) -> TestResult {
        let expression = format!(
            "return document.activeElement?.matches({});",
            serde_json::to_string(selector)?
        );
        if self.execute(&expression).await? == true {
            return Ok(());
        }
        // A supported page of 100 services exposes up to 200 row controls,
        // in addition to the editor, navigation and prototype options.
        // Keep real keyboard traversal and a finite failure limit.
        for _ in 0..320 {
            self.key(TAB).await?;
            if self.execute(&expression).await? == true {
                return Ok(());
            }
        }
        Err(format!("Element not reachable by Tab: {selector}").into())
    }

    pub async fn wait(&self, expression: &str, timeout: Duration) -> TestResult {
        let started = Instant::now();
        while started.elapsed() < timeout {
            if self.execute(expression).await? == true {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Err(format!("Browser condition timed out after {timeout:?}: {expression}").into())
    }

    pub async fn screenshot(&self, path: &Path) -> TestResult {
        let value = self.request(Method::GET, "screenshot", None).await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.as_str().ok_or("Screenshot is not base64")?)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub async fn full_screenshot(&self, path: &Path) -> TestResult {
        let layout = self.cdp("Page.getLayoutMetrics", json!({})).await?;
        let size = &layout["cssContentSize"];
        let width = size["width"].as_f64().ok_or("Missing page width")?;
        let height = size["height"].as_f64().ok_or("Missing page height")?;
        let capture = self
            .cdp(
                "Page.captureScreenshot",
                json!({
                    "format":"png", "fromSurface":true, "captureBeyondViewport":true,
                    "clip":{"x":0,"y":0,"width":width,"height":height,"scale":1}
                }),
            )
            .await?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(capture["data"].as_str().ok_or("Screenshot data absent")?)?;
        fs::write(path, bytes)?;
        Ok(())
    }

    pub async fn logs(&self) -> TestResult<Value> {
        self.post("log", json!({"type":"browser"})).await
    }

    pub async fn fill(&self, selector: &str, text: &str) -> TestResult {
        // Use actual editing input. ChromeDriver's CLEAR atom assigns an empty
        // value, dispatches change without input, then blurs the control.
        // A controlled input can therefore restore its old value before typing.
        let before = self.property(selector, "value").await?;
        self.click(selector).await?;
        let focused = format!(
            "return document.activeElement?.matches({});",
            serde_json::to_string(selector)?
        );
        assert_eq!(
            self.execute(&focused).await?,
            true,
            "The field must hold focus before keyboard replacement: {selector}"
        );
        self.chord(&["\u{e009}"], "a").await?;
        self.key("\u{e003}").await?;
        assert_eq!(
            self.property(selector, "value").await?,
            "",
            "Native keyboard clearing failed for {selector}; previous value: {before}"
        );
        assert_eq!(
            self.execute(&focused).await?,
            true,
            "The field must keep focus after keyboard clearing: {selector}"
        );
        // Resolve again after the input event, so a legitimate reactive rebuild
        // cannot leave the typing command with an obsolete element reference.
        let id = self.element(selector).await?;
        if !text.is_empty() {
            self.post(&format!("element/{id}/value"), json!({"text":text}))
                .await?;
        }
        assert_eq!(
            self.property(selector, "value").await?,
            text,
            "Native keyboard replacement failed for {selector}; previous value: {before}"
        );
        Ok(())
    }

    pub async fn chord(&self, modifiers: &[&str], key: &str) -> TestResult {
        let mut actions: Vec<Value> = modifiers
            .iter()
            .map(|value| json!({"type":"keyDown","value":value}))
            .collect();
        actions.push(json!({"type":"keyDown","value":key}));
        actions.push(json!({"type":"keyUp","value":key}));
        actions.extend(
            modifiers
                .iter()
                .rev()
                .map(|value| json!({"type":"keyUp","value":value})),
        );
        self.post(
            "actions",
            json!({"actions":[{"type":"key","id":"keyboard","actions":actions}]}),
        )
        .await?;
        Ok(())
    }

    pub async fn ensure_options(&self) -> TestResult {
        if self
            .execute("return document.querySelector('.prototype-controls')?.open === false;")
            .await?
            == true
        {
            self.tab_to("#prototype-controls-toggle").await?;
            self.key(ENTER).await?;
            self.wait(
                "return document.querySelector('.prototype-controls')?.open === true;",
                Duration::from_secs(3),
            )
            .await?;
        }
        Ok(())
    }

    pub async fn history_back(&self) -> TestResult {
        self.post("back", json!({})).await?;
        Ok(())
    }

    pub async fn choose(&self, selector: &str, index: usize, expected: &str) -> TestResult {
        self.ensure_options().await?;
        self.tab_to(selector).await?;
        self.key(" ").await?;
        self.key(HOME).await?;
        for _ in 0..index {
            self.key(DOWN).await?;
        }
        self.key(ENTER).await?;
        let expression = format!(
            "return document.querySelector({})?.value === {};",
            serde_json::to_string(selector)?,
            serde_json::to_string(expected)?
        );
        self.wait(&expression, Duration::from_secs(3)).await?;
        Ok(())
    }

    pub async fn close(&self) -> TestResult {
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

pub fn write_json(path: &Path, value: &Value) -> TestResult {
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
