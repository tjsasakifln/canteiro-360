//! Ferramentas locais de compilação e verificação do Canteiro 360.

mod assets;
mod plan;
mod policy;

use serde::Serialize;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Verificação interrompida: {error}");
            ExitCode::FAILURE
        }
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask deve estar em tools/xtask")
        .to_owned()
}

fn run() -> Result<()> {
    let root = workspace();
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() > 1 {
        return Err("use um único comando: check, plan, policy, setup, build, budget ou ci".into());
    }
    match arguments.first().map(String::as_str).unwrap_or("help") {
        "plan" => plan::validate(&root),
        "policy" => policy::validate(&root),
        "check" => check(&root),
        "setup" => setup(&root),
        "build" => build(&root),
        "budget" => {
            assets::verify(&root, true)?;
            Ok(())
        }
        "ci" => {
            check(&root)?;
            command(&root, "cargo", &["fmt", "--all", "--", "--check"])?;
            command(
                &root,
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            command(&root, "cargo", &["test", "--locked", "--workspace"])?;
            command(
                &root,
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "canteiro-persistence",
                    "--",
                    "--ignored",
                    "--nocapture",
                ],
            )?;
            command(
                &root,
                "cargo",
                &[
                    "clippy",
                    "--locked",
                    "-p",
                    "canteiro-web",
                    "--no-default-features",
                    "--features",
                    "hydrate",
                    "--target",
                    "wasm32-unknown-unknown",
                    "--",
                    "-D",
                    "warnings",
                ],
            )?;
            command(&root, "cargo", &["deny", "--locked", "check"])?;
            build(&root)?;
            command(
                &root,
                "cargo",
                &[
                    "test",
                    "--locked",
                    "-p",
                    "canteiro-e2e",
                    "--",
                    "--ignored",
                    "--nocapture",
                ],
            )
        }
        "help" | "--help" | "-h" => {
            println!(
                "cargo xtask check   valida plano e política de código/segredos\ncargo xtask plan    valida referências do plano aprovado\ncargo xtask policy  rejeita lógica JS/TS e segredos no repositório\ncargo xtask setup   instala ferramentas nas versões fixadas\ncargo xtask build   compila SSR/WASM, gera Brotli e mede o pacote inicial\ncargo xtask budget  verifica limites e integridade dos arquivos Brotli\ncargo xtask ci      executa as verificações locais da integração contínua"
            );
            Ok(())
        }
        other => Err(format!("comando desconhecido: {other}").into()),
    }
}

fn check(root: &Path) -> Result<()> {
    plan::validate(root)?;
    policy::validate(root)
}

fn command(root: &Path, program: &str, arguments: &[&str]) -> Result<()> {
    println!("Executando {program} {}", arguments.join(" "));
    let status = Command::new(program)
        .args(arguments)
        .env_remove("LEPTOS_OUTPUT_NAME")
        .current_dir(root)
        .status()?;
    if !status.success() {
        return Err(format!("{program} terminou com {status}").into());
    }
    Ok(())
}

fn capture(root: &Path, program: &str, arguments: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(arguments)
        .env_remove("LEPTOS_OUTPUT_NAME")
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        // Não imprimir stdout/stderr de processos que podem carregar configuração externa.
        return Err(format!("{program} terminou com {}", output.status).into());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn tool_version(root: &Path, key: &str) -> Result<String> {
    let manifest: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.toml"))?)?;
    let version = manifest
        .get("workspace")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("canteiro"))
        .and_then(|v| v.get(key))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("versão de {key} ausente em workspace.metadata.canteiro"))?;
    if !RegexVersion::valid(version) {
        return Err(format!("versão de {key} precisa ser exata e estável").into());
    }
    Ok(version.to_owned())
}

struct RegexVersion;

impl RegexVersion {
    fn valid(value: &str) -> bool {
        let parts = value.split('.').collect::<Vec<_>>();
        parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
    }
}

fn setup(root: &Path) -> Result<()> {
    for (package, executable) in [
        ("wasm-bindgen-cli", "wasm-bindgen"),
        ("cargo-deny", "cargo-deny"),
    ] {
        let version = tool_version(root, package)?;
        let existing = capture(root, executable, &["--version"]).unwrap_or_default();
        if existing.split_whitespace().nth(1) != Some(version.as_str()) {
            command(
                root,
                "cargo",
                &["install", package, "--locked", "--version", &version],
            )?;
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct BuildReport {
    schema_version: u8,
    rustc: String,
    cargo: String,
    wasm_bindgen: String,
    host: String,
    server_build_ms: u128,
    wasm_build_ms: u128,
    binding_generation_ms: u128,
    total_ms: u128,
    server_bytes: u64,
    incremental: bool,
    package: assets::BudgetReport,
}

fn build(root: &Path) -> Result<()> {
    check(root)?;
    let cli_version = tool_version(root, "wasm-bindgen-cli")?;
    let cli = capture(root, "wasm-bindgen", &["--version"])?;
    if cli.split_whitespace().nth(1) != Some(cli_version.as_str()) {
        return Err("wasm-bindgen-cli diverge da versão fixada; execute cargo xtask setup".into());
    }
    let lock: toml::Value = toml::from_str(&fs::read_to_string(root.join("Cargo.lock"))?)?;
    let matches = lock["package"]
        .as_array()
        .ok_or("Cargo.lock sem pacotes")?
        .iter()
        .filter(|p| p["name"].as_str() == Some("wasm-bindgen"))
        .collect::<Vec<_>>();
    if matches.len() != 1 || matches[0]["version"].as_str() != Some(cli_version.as_str()) {
        return Err("wasm-bindgen-cli precisa coincidir com wasm-bindgen no Cargo.lock".into());
    }
    let total = Instant::now();
    let incremental = root.join("target/release/canteiro-api").exists();
    let start = Instant::now();
    command(
        root,
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "canteiro-api",
            "-p",
            "canteiro-jobs",
            "-p",
            "canteiro-persistence",
            "--release",
        ],
    )?;
    let server_build_ms = start.elapsed().as_millis();
    let start = Instant::now();
    command(
        root,
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "canteiro-web",
            "--no-default-features",
            "--features",
            "hydrate",
            "--target",
            "wasm32-unknown-unknown",
            "--profile",
            "wasm-release",
        ],
    )?;
    let wasm_build_ms = start.elapsed().as_millis();
    let package = root.join("target/site/pkg");
    if package.exists() {
        fs::remove_dir_all(&package)?;
    }
    fs::create_dir_all(&package)?;
    let start = Instant::now();
    command(
        root,
        "wasm-bindgen",
        &[
            "--target",
            "web",
            "--no-typescript",
            "--out-name",
            "canteiro-web",
            "--out-dir",
            "target/site/pkg",
            "target/wasm32-unknown-unknown/wasm-release/canteiro_web.wasm",
        ],
    )?;
    let binding_generation_ms = start.elapsed().as_millis();
    fs::copy(
        root.join("style/main.css"),
        package.join("canteiro-web.css"),
    )?;
    fs::copy(
        root.join("style/prototype.css"),
        package.join("prototype.css"),
    )?;
    assets::compress_site(&root.join("target/site"))?;
    fs::create_dir_all(root.join("target/evidence"))?;
    let html = capture(root, "target/release/canteiro-api", &["--render-shell"])?;
    if !html.contains("<html") || !html.contains("Canteiro 360") {
        return Err("render-shell não devolveu o HTML da rota inicial".into());
    }
    fs::write(root.join("target/evidence/index.html"), html)?;
    let report = BuildReport {
        schema_version: 1,
        rustc: capture(root, "rustc", &["--version"])?.trim().to_owned(),
        cargo: capture(root, "cargo", &["--version"])?.trim().to_owned(),
        wasm_bindgen: cli.trim().to_owned(),
        host: format!("{}-{}", env::consts::ARCH, env::consts::OS),
        server_build_ms,
        wasm_build_ms,
        binding_generation_ms,
        total_ms: total.elapsed().as_millis(),
        server_bytes: fs::metadata(root.join("target/release/canteiro-api"))?.len(),
        incremental,
        package: assets::verify(root, true)?,
    };
    fs::write(
        root.join("target/evidence/build.json"),
        serde_json::to_string_pretty(&report)?,
    )?;
    println!("Compilação e orçamento aprovados. Evidências: target/evidence/build.json");
    Ok(())
}
