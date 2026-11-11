use crate::Result;
use regex::Regex;
use serde::Serialize;
use std::{fs, io::Read, path::Path};

const WASM_LIMIT: u64 = 1024 * 1024;
const JS_LIMIT: u64 = 80 * 1024;
const CSS_LIMIT: u64 = 60 * 1024;
const MAX_RAW_ASSET: u64 = 128 * 1024 * 1024;

#[derive(Serialize)]
pub struct Asset {
    path: String,
    kind: String,
    raw_bytes: u64,
    brotli_bytes: Option<u64>,
    budget_bytes: u64,
}

#[derive(Default, Serialize)]
pub struct Totals {
    pub wasm_brotli_bytes: u64,
    pub javascript_brotli_bytes: u64,
    pub css_brotli_bytes: u64,
    pub other_brotli_bytes: u64,
    pub inline_javascript_raw_bytes: u64,
    pub inline_css_raw_bytes: u64,
}

#[derive(Serialize)]
pub struct BudgetReport {
    schema_version: u8,
    route: &'static str,
    scope: &'static str,
    compression: &'static str,
    pub assets: Vec<Asset>,
    pub totals: Totals,
    wasm_limit_bytes: u64,
    javascript_limit_bytes: u64,
    css_limit_bytes: u64,
    javascript_budget_bytes: u64,
    css_budget_bytes: u64,
    html_raw_bytes: u64,
    html_reference_brotli_bytes: u64,
}

fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoded = Vec::new();
    brotli::CompressorReader::new(bytes, 4096, 11, 22).read_to_end(&mut encoded)?;
    Ok(encoded)
}

fn files(directory: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut output = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("artefatos servidos não podem usar links simbólicos".into());
        }
        if kind.is_dir() {
            output.extend(files(&entry.path())?);
        } else if kind.is_file() {
            output.push(entry.path());
        }
    }
    output.sort();
    Ok(output)
}

pub fn compress_site(site: &Path) -> Result<()> {
    for path in files(site)? {
        if path.extension().is_some_and(|extension| extension == "br") {
            continue;
        }
        if fs::metadata(&path)?.len() > MAX_RAW_ASSET {
            return Err("artefato maior que o limite de leitura da ferramenta".into());
        }
        let bytes = fs::read(&path)?;
        let br = path.with_file_name(format!(
            "{}.br",
            path.file_name()
                .ok_or("arquivo sem nome")?
                .to_string_lossy()
        ));
        fs::write(br, compress(&bytes)?)?;
    }
    Ok(())
}

pub fn verify(root: &Path, write_report: bool) -> Result<BudgetReport> {
    let site = root.join("target/site");
    for required in [
        "pkg/canteiro-web.js",
        "pkg/canteiro-web_bg.wasm",
        "pkg/canteiro-web.css",
    ] {
        if !site.join(required).is_file() {
            return Err(format!(
                "artefato obrigatório ausente: {required}; execute cargo xtask build"
            )
            .into());
        }
    }
    let mut assets = Vec::new();
    let mut totals = Totals::default();
    for path in files(&site)? {
        let relative = path
            .strip_prefix(&site)?
            .to_string_lossy()
            .replace('\\', "/");
        if path.extension().is_some_and(|extension| extension == "br") {
            let original = path.with_extension("");
            if !original.is_file() {
                return Err(format!("arquivo Brotli órfão: {relative}").into());
            }
            continue;
        }
        let raw_len = fs::metadata(&path)?.len();
        if raw_len > MAX_RAW_ASSET {
            return Err(format!("artefato excede o limite de leitura: {relative}").into());
        }
        let original = fs::read(&path)?;
        let compressed_path = path.with_file_name(format!(
            "{}.br",
            path.file_name()
                .ok_or("arquivo sem nome")?
                .to_string_lossy()
        ));
        let compressed =
            fs::read(&compressed_path).map_err(|_| format!("Brotli ausente para {relative}"))?;
        let mut decoded = Vec::new();
        brotli::Decompressor::new(compressed.as_slice(), 4096)
            .take(raw_len + 1)
            .read_to_end(&mut decoded)
            .map_err(|_| format!("Brotli inválido para {relative}"))?;
        if decoded != original {
            return Err(format!(
                "Brotli desatualizado ou diferente do arquivo servido: {relative}"
            )
            .into());
        }
        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let kind = match extension.as_str() {
            "wasm" => "wasm",
            "js" | "mjs" | "cjs" => "javascript",
            "css" => "css",
            "svg" | "png" | "jpg" | "jpeg" | "webp" | "avif" | "ico" | "woff" | "woff2" | "txt" => {
                "other"
            }
            _ => {
                return Err(format!(
                    "tipo de arquivo servido sem classificação de orçamento: {relative}"
                )
                .into());
            }
        };
        if kind == "css" {
            verify_css_references(&site, &path, std::str::from_utf8(&original)?)?;
        }
        add(&mut totals, kind, compressed.len() as u64);
        assets.push(Asset {
            path: relative,
            kind: kind.to_owned(),
            raw_bytes: raw_len,
            brotli_bytes: Some(compressed.len() as u64),
            budget_bytes: compressed.len() as u64,
        });
    }
    let html = fs::read_to_string(root.join("target/evidence/index.html"))?;
    if !html.contains("<html") || !html.contains("Canteiro 360") {
        return Err("evidência HTML da entrada ausente ou inválida".into());
    }
    // O bootstrap oficial também ocupa a resposta inicial. Contar seu tamanho
    // sem compressão evita tratar Brotli isolado como se fosse transferência real
    // de uma parte do HTML comprimido. Nenhum script inline fica fora da soma.
    let scripts = Regex::new(r#"(?is)<scrip[t]\b([^>]*)>(.*?)</scrip[t]\s*>"#)?;
    for (index, script) in scripts.captures_iter(&html).enumerate() {
        if let Some(source) = attribute(&script[1], "src")? {
            let source = source.split('?').next().ok_or("script sem caminho")?;
            if !source.starts_with("/pkg/")
                || source.contains("..")
                || !site.join(source.trim_start_matches('/')).is_file()
            {
                return Err("script externo não incluído no orçamento da entrada".into());
            }
        }
        let payload = script[2].as_bytes();
        if !payload.is_empty() {
            let size = payload.len() as u64;
            totals.inline_javascript_raw_bytes += size;
            assets.push(Asset {
                path: format!("HTML:inline-script-{index}"),
                kind: "javascript".to_owned(),
                raw_bytes: payload.len() as u64,
                brotli_bytes: None,
                budget_bytes: size,
            });
        }
    }
    verify_html_stylesheets(&site, &html)?;
    let styles = Regex::new(r#"(?is)<style\b[^>]*>(.*?)</style\s*>"#)?;
    for (index, style) in styles.captures_iter(&html).enumerate() {
        let payload = style[1].as_bytes();
        verify_css_references(&site, &site.join("index.html"), &style[1])?;
        let size = payload.len() as u64;
        totals.inline_css_raw_bytes += size;
        assets.push(Asset {
            path: format!("HTML:inline-style-{index}"),
            kind: "css".to_owned(),
            raw_bytes: payload.len() as u64,
            brotli_bytes: None,
            budget_bytes: size,
        });
    }
    enforce(&totals)?;
    let report = BudgetReport {
        schema_version: 1,
        route: "/",
        scope: "todos os arquivos servidos em target/site e scripts/estilos inline da rota; soma conservadora",
        compression: "arquivos .br conferidos byte a byte; inline contabilizado sem compressão; HTML reference usa Brotli quality=11 lgwin=22 e não representa medida de rede",
        javascript_budget_bytes: totals.javascript_brotli_bytes
            + totals.inline_javascript_raw_bytes,
        css_budget_bytes: totals.css_brotli_bytes + totals.inline_css_raw_bytes,
        assets,
        totals,
        wasm_limit_bytes: WASM_LIMIT,
        javascript_limit_bytes: JS_LIMIT,
        css_limit_bytes: CSS_LIMIT,
        html_raw_bytes: html.len() as u64,
        html_reference_brotli_bytes: compress(html.as_bytes())?.len() as u64,
    };
    if write_report {
        fs::create_dir_all(root.join("target/evidence"))?;
        fs::write(
            root.join("target/evidence/budget.json"),
            serde_json::to_string_pretty(&report)?,
        )?;
    }
    println!(
        "Orçamento inicial (.br servido + inline bruto): WASM {}/{WASM_LIMIT} B, JS {}/{JS_LIMIT} B, CSS {}/{CSS_LIMIT} B.",
        report.totals.wasm_brotli_bytes, report.javascript_budget_bytes, report.css_budget_bytes
    );
    Ok(report)
}

fn add(totals: &mut Totals, kind: &str, bytes: u64) {
    match kind {
        "wasm" => totals.wasm_brotli_bytes += bytes,
        "javascript" => totals.javascript_brotli_bytes += bytes,
        "css" => totals.css_brotli_bytes += bytes,
        _ => totals.other_brotli_bytes += bytes,
    }
}

fn enforce(totals: &Totals) -> Result<()> {
    for (label, actual, limit) in [
        ("WASM", totals.wasm_brotli_bytes, WASM_LIMIT),
        (
            "JS",
            totals.javascript_brotli_bytes + totals.inline_javascript_raw_bytes,
            JS_LIMIT,
        ),
        (
            "CSS",
            totals.css_brotli_bytes + totals.inline_css_raw_bytes,
            CSS_LIMIT,
        ),
    ] {
        if actual > limit {
            return Err(format!(
                "orçamento {label} excedido: {actual} B > {limit} B (.br servido + inline bruto)"
            )
            .into());
        }
    }
    Ok(())
}

fn verify_html_stylesheets(site: &Path, html: &str) -> Result<()> {
    let links = Regex::new(r#"(?is)<link\b([^>]*)>"#)?;
    if Regex::new(r#"(?i)<base\b"#)?.is_match(html) {
        return Err("base HTML pode alterar a origem dos recursos contabilizados".into());
    }
    for link in links.captures_iter(html) {
        if attribute(&link[1], "rel")?.is_some_and(|value| {
            value.split_ascii_whitespace().any(|name| {
                ["stylesheet", "preload", "modulepreload"]
                    .iter()
                    .any(|expected| name.eq_ignore_ascii_case(expected))
            })
        }) {
            let reference =
                attribute(&link[1], "href")?.ok_or("folha de estilo sem href contabilizado")?;
            local_reference(site, &site.join("index.html"), reference)?;
        }
    }
    Ok(())
}

fn attribute<'a>(tag: &'a str, name: &str) -> Result<Option<&'a str>> {
    let attributes =
        Regex::new(r#"(?is)\b([a-z][a-z0-9:-]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#)?;
    for captures in attributes.captures_iter(tag) {
        if captures[1].eq_ignore_ascii_case(name) {
            return Ok(captures
                .get(2)
                .or_else(|| captures.get(3))
                .or_else(|| captures.get(4))
                .map(|value| value.as_str()));
        }
    }
    Ok(None)
}

fn verify_css_references(site: &Path, file: &Path, css: &str) -> Result<()> {
    // Escapes podem esconder url/@import dos verificadores textuais. A base
    // atual não precisa desse formato; sua introdução requer parser dedicado.
    if css.contains('\\') {
        return Err(
            "CSS com escapes exige análise de dependências antes de entrar no pacote inicial"
                .into(),
        );
    }
    let urls = Regex::new(r#"(?is)\burl\(\s*([^)]*)\)"#)?;
    for capture in urls.captures_iter(css) {
        local_reference(site, file, capture[1].trim().trim_matches(['"', '\'']))?;
    }
    let imports = Regex::new(r#"(?is)@import\s+(["'])([^"']+)["']"#)?;
    for capture in imports.captures_iter(css) {
        local_reference(site, file, &capture[2])?;
    }
    Ok(())
}

fn local_reference(site: &Path, source: &Path, reference: &str) -> Result<()> {
    let reference = reference.trim();
    if reference.starts_with('#') {
        return Ok(());
    }
    if reference.is_empty()
        || reference.starts_with("//")
        || reference.contains(':')
        || reference.contains('\\')
        || reference.split('/').any(|part| part == "..")
    {
        return Err("recurso externo ou não contabilizado na entrada".into());
    }
    let reference = reference
        .split(['?', '#'])
        .next()
        .ok_or("recurso sem caminho")?;
    let target = if reference.starts_with('/') {
        site.join(reference.trim_start_matches('/'))
    } else {
        source
            .parent()
            .ok_or("referência sem diretório")?
            .join(reference)
    };
    if !target.is_file() {
        return Err("recurso local referenciado não existe no conjunto contabilizado".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "canteiro-budget-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("target/site/pkg")).expect("diretório temporário");
            fs::create_dir_all(path.join("target/evidence")).expect("diretório de evidência");
            for name in [
                "canteiro-web.js",
                "canteiro-web_bg.wasm",
                "canteiro-web.css",
            ] {
                fs::write(
                    path.join("target/site/pkg").join(name),
                    b"synthetic build fixture",
                )
                .expect("artefato fictício");
            }
            fs::write(
                path.join("target/evidence/index.html"),
                "<!doctype html><html lang=\"pt-BR\"><body>Canteiro 360</body></html>",
            )
            .expect("HTML de teste");
            compress_site(&path.join("target/site")).expect("comprimir fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn extra_files_are_aggregated_instead_of_looking_only_at_main_wasm() {
        let fixture = Fixture::new();
        let first = verify(&fixture.0, false).expect("fixture válida");
        fs::write(
            fixture.0.join("target/site/pkg/second.wasm"),
            b"another required module",
        )
        .expect("módulo extra");
        compress_site(&fixture.0.join("target/site")).expect("comprimir");
        let second = verify(&fixture.0, false).expect("pacote continua pequeno");
        assert!(second.totals.wasm_brotli_bytes > first.totals.wasm_brotli_bytes);
    }

    #[test]
    fn stale_or_missing_brotli_is_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("target/site/pkg/canteiro-web.css"),
            b"changed after compression",
        )
        .expect("mutação");
        assert!(verify(&fixture.0, false).is_err());
        fs::remove_file(fixture.0.join("target/site/pkg/canteiro-web.css.br"))
            .expect("remover Brotli");
        assert!(verify(&fixture.0, false).is_err());
    }

    #[test]
    fn exact_limit_passes_but_one_byte_over_fails_for_each_kind() {
        for (kind, limit) in [
            ("wasm", WASM_LIMIT),
            ("javascript", JS_LIMIT),
            ("css", CSS_LIMIT),
        ] {
            let mut totals = Totals::default();
            add(&mut totals, kind, limit);
            assert!(enforce(&totals).is_ok());
            add(&mut totals, kind, 1);
            assert!(enforce(&totals).is_err());
        }
    }

    #[test]
    fn inline_bootstrap_is_counted() {
        let fixture = Fixture::new();
        let before = verify(&fixture.0, false).expect("fixture");
        let html = [
            "<html>Canteiro 360<scr",
            "ipt type=\"module\">import('/pkg/canteiro-web.js');</scr",
            "ipt></html>",
        ]
        .concat();
        fs::write(fixture.0.join("target/evidence/index.html"), html)
            .expect("script gerado fictício");
        let after = verify(&fixture.0, false).expect("bootstrap pequeno");
        assert!(
            after.totals.inline_javascript_raw_bytes > before.totals.inline_javascript_raw_bytes
        );
    }

    #[test]
    fn additional_large_javascript_file_breaks_real_compressed_budget() {
        let fixture = Fixture::new();
        let mut state = 0x1234_5678_u32;
        let payload = (0..200_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                b'!' + (state % 90) as u8
            })
            .collect::<Vec<_>>();
        fs::write(fixture.0.join("target/site/pkg/extra.js"), payload).expect("arquivo sintético");
        compress_site(&fixture.0.join("target/site")).expect("comprimir");
        let error = verify(&fixture.0, false)
            .err()
            .expect("pacote acima do limite");
        assert!(error.to_string().contains("orçamento JS excedido"));
    }

    #[test]
    fn remote_stylesheets_and_imports_cannot_escape_the_budget() {
        let fixture = Fixture::new();
        let html = "<html>Canteiro 360<link rel=\"stylesheet\" href=\"https://example.org/extra.css\"></html>";
        fs::write(fixture.0.join("target/evidence/index.html"), html).expect("CSS externo");
        assert!(verify(&fixture.0, false).is_err());
        fs::write(
            fixture.0.join("target/evidence/index.html"),
            "<html>Canteiro 360<link rel=stylesheet href=https://example.org/extra.css></html>",
        )
        .expect("atributos sem aspas");
        assert!(verify(&fixture.0, false).is_err());
        fs::write(
            fixture.0.join("target/evidence/index.html"),
            "<html>Canteiro 360</html>",
        )
        .expect("HTML local");
        for css in [
            "@import url(https://example.org/extra.css);",
            "@import 'https://example.org/extra.css';",
            "body{background:url(//example.org/image.png)}",
            "@import 'missing.css';",
        ] {
            fs::write(fixture.0.join("target/site/pkg/canteiro-web.css"), css)
                .expect("mutação CSS");
            compress_site(&fixture.0.join("target/site")).expect("comprimir");
            assert!(verify(&fixture.0, false).is_err(), "precisa rejeitar {css}");
        }
    }

    #[test]
    fn local_stylesheet_import_is_validated_and_counted() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("target/site/pkg/additional.css"),
            "body{color:navy}",
        )
        .expect("folha adicional");
        fs::write(
            fixture.0.join("target/site/pkg/canteiro-web.css"),
            "@import 'additional.css';",
        )
        .expect("import local");
        compress_site(&fixture.0.join("target/site")).expect("comprimir");
        let report = verify(&fixture.0, false).expect("recurso local incluído");
        assert!(
            report
                .assets
                .iter()
                .any(|asset| asset.path == "pkg/additional.css")
        );
    }
}
