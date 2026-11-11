use crate::Result;
use regex::Regex;
use serde_json::Value;
use std::{fs, path::Path, process::Command};

pub fn validate(root: &Path) -> Result<()> {
    let tracked = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()?;
    if !tracked.status.success() {
        return Err("não foi possível conferir os arquivos versionados".into());
    }
    for path in tracked
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = std::str::from_utf8(path)?;
        if path.starts_with("target/") {
            return Err("artefatos gerados em target não podem ser versionados".into());
        }
    }
    let patterns = Patterns::new()?;
    let mut scanned = 0;
    walk(root, root, &patterns, &mut scanned)?;
    println!(
        "Política aprovada: {scanned} arquivos, sem lógica JS/TS própria ou credenciais embutidas detectadas."
    );
    Ok(())
}

fn walk(root: &Path, directory: &Path, patterns: &Patterns, scanned: &mut usize) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root)?;
        if relative == Path::new(".git") {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(format!(
                "link simbólico não permitido na base de código: {}",
                relative.display()
            )
            .into());
        }
        if relative == Path::new("target") {
            continue;
        }
        if file_type.is_dir() {
            walk(root, &path, patterns, scanned)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path)?;
            inspect(relative, &bytes, patterns)?;
            *scanned += 1;
        }
    }
    Ok(())
}

struct Patterns {
    executable_markup: Regex,
    javascript_api: Regex,
    known_credential: Regex,
    config_assignment: Regex,
    rust_literal: Regex,
}

impl Patterns {
    fn new() -> Result<Self> {
        Ok(Self {
            executable_markup: Regex::new(
                r#"(?i)<\s*script\b|\bon(?:click|load|error|submit|focus|mouseover)\s*=|javascript\s*:"#,
            )?,
            javascript_api: Regex::new(r#"\binline_js\s*=|\bjs_sys\s*::\s*(?:eval|Function)\b"#)?,
            known_credential: Regex::new(
                r#"-----BEGIN (?:[A-Z ]+ )?PRIVATE KEY-----|\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[0-9A-Z]{16})\b|(?:postgres(?:ql)?|mysql|redis)://[^\s/:"']+:[^\s/@"']+@"#,
            )?,
            config_assignment: Regex::new(
                r#"(?im)(?:^|[{,])\s*(?:export\s+)?["']?([A-Z0-9_-]*(?:PASSWORD|SECRET|TOKEN|API[_-]?KEY|PRIVATE[_-]?KEY|DATABASE[_-]?URL))["']?\s*[:=]\s*("[^"\r\n]*"|'[^'\r\n]*'|[^\r\n,]*)"#,
            )?,
            rust_literal: Regex::new(
                r#"(?im)\b(?:let\s+(?:mut\s+)?|const\s+|static\s+)([A-Z0-9_]*(?:PASSWORD|SECRET|TOKEN|API_KEY|PRIVATE_KEY|DATABASE_URL))\s*(?::\s*&(?:'static\s+)?str)?\s*=\s*[br]*#*"([^"]+)"#,
            )?,
        })
    }
}

fn inspect(path: &Path, bytes: &[u8], patterns: &Patterns) -> Result<()> {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let filename = path.file_name().and_then(|v| v.to_str()).unwrap_or("");
    if ["js", "jsx", "ts", "tsx", "mjs", "cjs", "mts", "cts"].contains(&extension.as_str())
        || filename == "package.json"
        || filename == "package-lock.json"
        || filename == "yarn.lock"
        || filename == "pnpm-lock.yaml"
    {
        return Err(format!(
            "lógica JS/TS ou servidor Node não permitido: {}",
            path.display()
        )
        .into());
    }
    if filename == ".env"
        || filename.starts_with(".env.")
            && !filename.ends_with(".example")
            && !filename.ends_with(".template")
    {
        return Err(format!(
            "configuração local secreta deve ficar fora do repositório: {}",
            path.display()
        )
        .into());
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Ok(());
    };
    if patterns.known_credential.is_match(text) {
        return Err(format!(
            "padrão de credencial embutida detectado: {}",
            path.display()
        )
        .into());
    }
    if extension == "json" {
        let data: Value =
            serde_json::from_str(text).map_err(|_| format!("JSON inválido: {}", path.display()))?;
        inspect_values(path, &data)?;
    }
    if extension == "toml" {
        let data: toml::Value =
            toml::from_str(text).map_err(|_| format!("TOML inválido: {}", path.display()))?;
        inspect_values(path, &serde_json::to_value(data)?)?;
    }
    if ["rs", "html", "htm", "svg", "xml"].contains(&extension.as_str())
        && (patterns.executable_markup.is_match(text) || patterns.javascript_api.is_match(text))
    {
        return Err(format!(
            "script próprio, evento inline ou execução JS detectada: {}",
            path.display()
        )
        .into());
    }
    if [
        "yaml",
        "yml",
        "ini",
        "conf",
        "properties",
        "example",
        "template",
    ]
    .contains(&extension.as_str())
        || filename.starts_with(".env")
    {
        for capture in patterns.config_assignment.captures_iter(text) {
            if capture[2].trim_start().starts_with('#') {
                continue;
            }
            let value = capture[2]
                .trim()
                .trim_end_matches(',')
                .trim()
                .trim_matches(['"', '\'']);
            if !external_or_empty(value) {
                return Err(format!(
                    "configuração contém valor secreto literal: {} (campo {})",
                    path.display(),
                    &capture[1]
                )
                .into());
            }
        }
    }
    if extension == "rs" {
        for capture in patterns.rust_literal.captures_iter(text) {
            if !external_or_empty(&capture[2]) {
                return Err(format!(
                    "literal secreto em código: {} (campo {})",
                    path.display(),
                    &capture[1]
                )
                .into());
            }
        }
    }
    Ok(())
}

fn inspect_values(path: &Path, value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                if secret_key(key)
                    && !value.is_null()
                    && !value.as_str().is_some_and(external_or_empty)
                {
                    return Err(format!(
                        "configuração contém valor secreto literal: {} (campo {key})",
                        path.display()
                    )
                    .into());
                }
                inspect_values(path, value)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                inspect_values(path, value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "password",
        "secret",
        "secretkey",
        "token",
        "apikey",
        "privatekey",
        "signingkey",
        "accesskey",
        "databaseurl",
    ]
    .iter()
    .any(|suffix| normalized.ends_with(suffix))
}

fn external_or_empty(value: &str) -> bool {
    value.is_empty() || Regex::new(r#"^\$\{[A-Za-z_][A-Za-z0-9_]*\}$|^\$\{\{\s*(?:secrets|env)\.[A-Za-z_][A-Za-z0-9_]*\s*\}\}$"#).expect("referências externas válidas").is_match(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(path: &str, text: &str) -> bool {
        inspect(
            Path::new(path),
            text.as_bytes(),
            &Patterns::new().expect("expressões válidas"),
        )
        .is_ok()
    }

    #[test]
    fn every_javascript_module_extension_is_rejected() {
        for extension in ["js", "jsx", "ts", "tsx", "mjs", "cjs", "mts", "cts", "JS"] {
            assert!(!allowed(&format!("crates/web/custom.{extension}"), ""));
        }
        assert!(!allowed("package.json", "{}"));
    }

    #[test]
    fn inline_script_cannot_hide_in_html_or_rust() {
        let script = ["<scr", "ipt>window.example = true;</scr", "ipt>"].concat();
        assert!(!allowed("crates/web/src/lib.rs", &script));
        assert!(!allowed("public/index.html", &script));
        let event = ["<button on", "click=\"doWork()\">"].concat();
        assert!(!allowed("public/index.html", &event));
        let bridge = ["#[wasm_bindgen(inline", "_js = \"export function x(){}\")]"].concat();
        assert!(!allowed("crates/web/src/bridge.rs", &bridge));
    }

    #[test]
    fn example_password_is_not_an_exception() {
        for (path, assignment) in [
            (
                ".env.example",
                ["DATABASE_", "PASSWORD=example-password"].concat(),
            ),
            (
                "config/homologacao.toml",
                ["password", " = \"example-password\""].concat(),
            ),
            (
                "config/test.json",
                ["{\"api_", "key\": \"example-value\"}"].concat(),
            ),
        ] {
            assert!(!allowed(path, &assignment));
        }
    }

    #[test]
    fn secrets_in_nested_maps_and_inline_toml_are_rejected() {
        let json = [
            "{\"connection\":[{\"pass",
            "word\":\"synthetic-example\"}]}",
        ]
        .concat();
        assert!(!allowed("config/nested.json", &json));
        let toml = ["credentials = { to", "ken = \"synthetic-example\" }"].concat();
        assert!(!allowed("config/inline.toml", &toml));
        let camel_case = ["{\"api", "Key\":\"synthetic-example\"}"].concat();
        assert!(!allowed("config/camel.json", &camel_case));
        assert!(allowed(
            "config/external.json",
            "{\"database_url\":\"${CANTEIRO_DATABASE_URL}\"}"
        ));
    }

    #[test]
    fn external_environment_reference_and_parser_are_allowed() {
        assert!(allowed(
            "config/dev.toml",
            "database_url_env = \"CANTEIRO_DATABASE_URL\""
        ));
        assert!(allowed(".env.example", "DATABASE_URL=\n"));
        assert!(allowed(
            "config/runtime.toml",
            "DATABASE_URL = \"${CANTEIRO_DATABASE_URL}\""
        ));
        assert!(allowed(
            "crates/api/src/config.rs",
            "let database_url = std::env::var(\"CANTEIRO_DATABASE_URL\");"
        ));
        assert!(allowed(
            "crates/web/src/lib.rs",
            "view! { <HydrationScripts options=options/> <button on:click=move |_| {}> }"
        ));
    }

    #[test]
    fn hardcoded_rust_secret_and_connection_password_are_rejected() {
        let literal = [
            "const APP_",
            "SECRET: &str = \"synthetic-sensitive-value\";",
        ]
        .concat();
        assert!(!allowed("crates/api/src/config.rs", &literal));
        let connection = ["postgre", "sql://demo:synthetic-value@localhost/test"].concat();
        assert!(!allowed("README.md", &connection));
    }

    #[test]
    fn dotenv_file_is_rejected_even_when_empty() {
        assert!(!allowed(".env", ""));
        assert!(!allowed(".env.production", ""));
    }

    #[test]
    fn prefixed_database_url_and_comment_like_password_are_rejected() {
        let assignment = ["CANTEIRO_DATABASE_", "URL=synthetic-credential"].concat();
        assert!(!allowed(".env.example", &assignment));
        let json = ["{\"pass", "word\":\"#synthetic-credential\"}"].concat();
        assert!(!allowed("config/example.json", &json));
        let disguised = ["{\"to", "ken\":\"${{ 'synthetic-credential' }}\"}"].concat();
        assert!(!allowed("config/disguised.json", &disguised));
        let dotenv = ["PASS", "WORD=\"#synthetic-credential\""].concat();
        assert!(!allowed(".env.example", &dotenv));
        let yaml = ["credentials: { to", "ken: synthetic-credential }"].concat();
        assert!(!allowed("config/inline.yaml", &yaml));
    }
}
