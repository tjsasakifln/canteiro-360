//! Transporte exclusivo das capturas e medições das jornadas sintéticas.
//! Nenhum log, configuração, cabeçalho ou valor de formulário é selecionado.
use crate::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
};

const FILE_LIMIT: u64 = 4 * 1024 * 1024;
const TOTAL_LIMIT: u64 = 8 * 1024 * 1024;
const CHUNK_LENGTH: usize = 2048;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const NAMES: [&str; 12] = [
    "registry-cadastros-360.png",
    "registry-cadastros-1280.png",
    "registry-carteira-360.png",
    "registry-carteira-1280.png",
    "registry-projetos-360.png",
    "registry-projetos-1280.png",
    "registry-documentos-360.png",
    "registry-documentos-1280.png",
    "registry-obrigacoes-360.png",
    "registry-obrigacoes-1280.png",
    "registry-failure.png",
    "registry-interface.json",
];
const ROUTES: [&str; 5] = [
    "cadastros",
    "carteira",
    "projetos",
    "documentos",
    "obrigacoes",
];

struct Prepared {
    name: &'static str,
    bytes: Vec<u8>,
    sha256: String,
}
struct Inventory {
    files: Vec<Prepared>,
    missing: Vec<&'static str>,
    source_bytes: u64,
}

fn authorization(
    synthetic: Option<&str>,
    environment: Option<&str>,
    source: Option<&str>,
) -> Result<String> {
    if synthetic != Some("1") || environment != Some("development") {
        return Err(
            "evidências exigem CANTEIRO_SYNTHETIC_EVIDENCE=1 e CANTEIRO_ENV=development".into(),
        );
    }
    let source =
        source.ok_or("informe CANTEIRO_EVIDENCE_SOURCE com o SHA de 40 caracteres da revisão")?;
    if source.len() != 40 || !source.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(
            "CANTEIRO_EVIDENCE_SOURCE precisa ser um SHA hexadecimal de 40 caracteres".into(),
        );
    }
    Ok(source.to_ascii_lowercase())
}

pub fn emit(root: &Path) -> Result<()> {
    let source = authorization(
        env::var("CANTEIRO_SYNTHETIC_EVIDENCE").ok().as_deref(),
        env::var("CANTEIRO_ENV").ok().as_deref(),
        env::var("CANTEIRO_EVIDENCE_SOURCE").ok().as_deref(),
    )?;
    // Validate the entire selection before printing any file content.
    let inventory = prepare(root)?;
    output(&inventory, &source, &mut io::stdout().lock())
}

fn directory(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err("não foi possível conferir o diretório de evidências".into()),
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => Ok(true),
        Ok(_) => Err(
            "target e target/evidence precisam ser diretórios reais, sem links simbólicos".into(),
        ),
    }
}

fn prepare(root: &Path) -> Result<Inventory> {
    let mut result = Inventory {
        files: vec![],
        missing: vec![],
        source_bytes: 0,
    };
    let root = root.canonicalize()?;
    let target = root.join("target");
    let evidence = target.join("evidence");
    if !directory(&target)? || !directory(&evidence)? {
        result.missing.extend(NAMES);
        return Ok(result);
    }
    if evidence.canonicalize()? != evidence {
        return Err(
            "o diretório de evidências precisa permanecer dentro de target/evidence".into(),
        );
    }
    let mut emitted_bytes = 0_u64;
    for name in NAMES {
        let Some(bytes) = read_regular(&evidence, name)? else {
            result.missing.push(name);
            continue;
        };
        result.source_bytes += bytes.len() as u64;
        if result.source_bytes > TOTAL_LIMIT {
            return Err("a seleção de evidências excede o limite total de 8 MiB".into());
        }
        let bytes = if name.ends_with(".png") {
            if !bytes.starts_with(PNG_SIGNATURE) {
                return Err(format!("assinatura PNG inválida: {name}").into());
            }
            bytes
        } else {
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|_| "registry-interface.json não contém JSON válido")?;
            serde_json::to_vec(&project_report(&value)?)?
        };
        emitted_bytes += bytes.len() as u64;
        if bytes.len() as u64 > FILE_LIMIT || emitted_bytes > TOTAL_LIMIT {
            return Err("o conteúdo projetado excede o limite de evidências".into());
        }
        result.files.push(Prepared {
            name,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            bytes,
        });
    }
    Ok(result)
}

fn same_file(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        first.dev() == second.dev()
            && first.ino() == second.ino()
            && first.len() == second.len()
            && first.mtime() == second.mtime()
            && first.mtime_nsec() == second.mtime_nsec()
    }
    #[cfg(not(unix))]
    {
        let _ = (first, second);
        // Fail closed where the descriptor identity cannot be checked this way.
        false
    }
}

fn read_regular(directory: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let path = directory.join(name);
    let before = match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(format!("não foi possível conferir o arquivo: {name}").into()),
        Ok(m) => m,
    };
    if !before.is_file() || before.file_type().is_symlink() {
        return Err(
            format!("a evidência precisa ser arquivo regular, sem link simbólico: {name}").into(),
        );
    }
    if before.len() > FILE_LIMIT {
        return Err(format!("a evidência excede 4 MiB: {name}").into());
    }
    if path.canonicalize()?.parent() != Some(directory) {
        return Err(format!("a evidência está fora de target/evidence: {name}").into());
    }
    let file = fs::File::open(&path)?;
    if !same_file(&before, &file.metadata()?) {
        return Err(format!("a identidade do arquivo mudou durante a leitura: {name}").into());
    }
    let mut bytes = Vec::new();
    (&file).take(FILE_LIMIT + 1).read_to_end(&mut bytes)?;
    let after = fs::symlink_metadata(&path)?;
    if bytes.len() as u64 > FILE_LIMIT
        || bytes.len() as u64 != before.len()
        || !after.is_file()
        || after.file_type().is_symlink()
        || !same_file(&before, &file.metadata()?)
        || !same_file(&before, &after)
        || path.canonicalize()?.parent() != Some(directory)
    {
        return Err(format!("a evidência mudou durante a leitura: {name}").into());
    }
    Ok(Some(bytes))
}

fn unsigned_fields(value: &Value, fields: &[&str]) -> Map<String, Value> {
    fields
        .iter()
        .filter_map(|key| value.get(*key)?.as_u64().map(|n| ((*key).into(), json!(n))))
        .collect()
}
fn allowed_text(value: &Value, key: &str, allowed: &[&str], output: &mut Map<String, Value>) {
    if let Some(text) = value.get(key).and_then(Value::as_str)
        && allowed.contains(&text)
    {
        output.insert(key.into(), json!(text));
    }
}
fn project_report(value: &Value) -> Result<Value> {
    if !value.is_object() {
        return Err("registry-interface.json precisa conter um objeto de medições".into());
    }
    let status = value["status"]
        .as_str()
        .filter(|s| ["passed", "failed", "running"].contains(s))
        .unwrap_or("unknown");
    let mut browser = Map::new();
    for key in ["browser", "driver"] {
        if let Some(version) = value["browser"][key]
            .as_str()
            .and_then(|s| s.split_whitespace().next())
        {
            let parts: Vec<_> = version.split('.').collect();
            if parts.len() == 4
                && parts.iter().all(|p| {
                    !p.is_empty() && p.len() <= 10 && p.bytes().all(|b| b.is_ascii_digit())
                })
            {
                browser.insert(key.into(), json!(version));
            }
        }
    }
    allowed_text(
        &value["browser"],
        "platform",
        &["linux", "mac", "windows"],
        &mut browser,
    );
    let environment_input = &value["environment"];
    let mut environment = unsigned_fields(
        environment_input,
        &[
            "startedAtUnixMs",
            "availableParallelism",
            "apiBinaryBytes",
            "wasmBinaryBytes",
        ],
    );
    allowed_text(
        environment_input,
        "os",
        &["linux", "macos", "windows", "freebsd"],
        &mut environment,
    );
    allowed_text(
        environment_input,
        "architecture",
        &["x86_64", "aarch64", "x86", "arm"],
        &mut environment,
    );
    let layouts = value["layouts"]
        .as_array()
        .into_iter()
        .flatten()
        .take(10)
        .filter_map(|row| {
            let route = row["route"].as_str().filter(|r| ROUTES.contains(r))?;
            let width = row["width"].as_u64().filter(|w| [360, 1280].contains(w))?;
            let geometry = unsigned_fields(
                &row["geometry"],
                &[
                    "width",
                    "scroll",
                    "headings",
                    "controlCount",
                    "minimumControlWidth",
                    "minimumControlHeight",
                ],
            );
            Some(json!({"route":route,"width":width,"geometry":geometry}))
        })
        .collect::<Vec<_>>();
    Ok(json!({"status":status,"browser":browser,"environment":environment,"layouts":layouts}))
}

fn output(inventory: &Inventory, source: &str, writer: &mut impl Write) -> Result<()> {
    let files = NAMES
        .iter()
        .map(
            |name| match inventory.files.iter().find(|f| f.name == *name) {
                Some(file) => json!({"name":name,"status":"ready","bytes":file.bytes.len()}),
                None => json!({"name":name,"status":"missing"}),
            },
        )
        .collect::<Vec<_>>();
    writeln!(
        writer,
        "C360_EVIDENCE_INVENTORY {}",
        json!({"source":source,"files":files,"source_bytes":inventory.source_bytes,"total_bytes":inventory.files.iter().map(|f| f.bytes.len()).sum::<usize>(),"missing":inventory.missing.len()})
    )?;
    for file in &inventory.files {
        let encoded = STANDARD.encode(&file.bytes);
        writeln!(
            writer,
            "C360_EVIDENCE_BEGIN {}",
            json!({"name":file.name,"bytes":file.bytes.len(),"sha256":file.sha256,"encoding":"base64","source":source})
        )?;
        for (index, chunk) in encoded.as_bytes().chunks(CHUNK_LENGTH).enumerate() {
            writeln!(
                writer,
                "C360_EVIDENCE_DATA {} {index} {}",
                file.name,
                std::str::from_utf8(chunk)?
            )?;
        }
        writeln!(
            writer,
            "C360_EVIDENCE_END {}",
            json!({"name":file.name,"sha256":file.sha256,"chunks":encoded.len().div_ceil(CHUNK_LENGTH)})
        )?;
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
    const SOURCE: &str = "0123456789abcdef0123456789abcdef01234567";
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = env::temp_dir().join(format!(
                "canteiro-evidence-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir_all(root.join("target/evidence")).unwrap();
            Self(root)
        }
        fn file(&self, name: &str) -> PathBuf {
            self.0.join("target/evidence").join(name)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn evidence_requires_both_gates_and_a_complete_source_sha() {
        assert!(authorization(None, Some("development"), Some(SOURCE)).is_err());
        assert!(authorization(Some("1"), Some("production"), Some(SOURCE)).is_err());
        for source in [
            None,
            Some("abc"),
            Some("../private"),
            Some("0123456789abcdef0123456789abcdef0123456\n"),
        ] {
            assert!(authorization(Some("1"), Some("development"), source).is_err());
        }
        assert_eq!(
            authorization(Some("1"), Some("development"), Some(SOURCE)).unwrap(),
            SOURCE
        );
    }

    #[test]
    fn evidence_projection_discards_secrets_and_all_free_text() {
        let input = json!({"status":"passed","token":"SECRET_VALUE","console":["SECRET_VALUE"],
            "browser":{"browser":"156.0.8078.80","driver":"156.0.8078.80 (SECRET_VALUE)","platform":"linux","cookie":"SECRET_VALUE"},
            "environment":{"startedAtUnixMs":123,"availableParallelism":2,"apiBinaryBytes":100,"wasmBinaryBytes":20,"os":"linux","architecture":"x86_64","cpu":"SECRET_VALUE","headers":{"authorization":"SECRET_VALUE"},"CANTEIRO_AUTH_KEY":"SECRET_VALUE"},
            "layouts":[{"route":"cadastros","width":360,"geometry":{"width":360,"scroll":360,"headings":1,"input":"SECRET_VALUE"},"form":{"name":"SECRET_VALUE"}}, {"route":"SECRET_VALUE","width":360}],
            "scenarios":["SECRET_VALUE"]});
        let projected = project_report(&input).unwrap();
        assert!(!projected.to_string().contains("SECRET_VALUE"));
        assert_eq!(projected["browser"]["driver"], "156.0.8078.80");
        assert_eq!(projected["environment"]["availableParallelism"], 2);
        assert_eq!(projected["layouts"].as_array().unwrap().len(), 1);
        assert_eq!(
            projected["layouts"][0]["geometry"],
            json!({"width":360,"scroll":360,"headings":1})
        );
        assert_eq!(
            project_report(&json!({"status":"SECRET_VALUE","environment":{"os":"SECRET_VALUE"}}))
                .unwrap()["status"],
            "unknown"
        );
        assert!(project_report(&json!(["SECRET_VALUE"])).is_err());
    }

    #[test]
    fn evidence_allowlist_frames_are_recoverable_and_missing_files_are_reported() {
        let fixture = Fixture::new();
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend(vec![42; 5000]);
        fs::write(fixture.file(NAMES[0]), &png).unwrap();
        fs::write(fixture.file("private-server.log"), "SECRET_VALUE").unwrap();
        fs::write(fixture.file("registry-unlisted-360.png"), "SECRET_VALUE").unwrap();
        let prepared = prepare(&fixture.0).unwrap();
        assert_eq!(prepared.files.len(), 1);
        assert_eq!(prepared.missing.len(), 11);
        let mut log = vec![];
        output(&prepared, SOURCE, &mut log).unwrap();
        let log = String::from_utf8(log).unwrap();
        assert!(!log.contains("SECRET_VALUE"));
        assert!(!log.contains("private-server.log"));
        let mut encoded = String::new();
        let mut chunks = 0;
        for line in log
            .lines()
            .filter(|line| line.starts_with("C360_EVIDENCE_DATA "))
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields[1], NAMES[0]);
            assert_eq!(fields[2].parse::<usize>().unwrap(), chunks);
            assert!(fields[3].len() <= CHUNK_LENGTH);
            encoded.push_str(fields[3]);
            chunks += 1;
        }
        assert_eq!(STANDARD.decode(encoded).unwrap(), png);
        let end: Value = serde_json::from_str(
            log.lines()
                .find_map(|l| l.strip_prefix("C360_EVIDENCE_END "))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(end["chunks"], chunks);
        assert_eq!(end["sha256"], format!("{:x}", Sha256::digest(&png)));
        fs::remove_file(fixture.file(NAMES[0])).unwrap();
        let missing = prepare(&fixture.0).unwrap();
        assert!(missing.files.is_empty());
        assert_eq!(missing.missing.len(), NAMES.len());
    }

    #[cfg(unix)]
    #[test]
    fn evidence_rejects_file_and_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = fixture.0.join("outside.png");
        fs::write(&outside, PNG_SIGNATURE).unwrap();
        symlink(&outside, fixture.file(NAMES[0])).unwrap();
        assert!(prepare(&fixture.0).is_err());
        fs::remove_file(fixture.file(NAMES[0])).unwrap();
        fs::remove_dir(fixture.0.join("target/evidence")).unwrap();
        symlink(&fixture.0, fixture.0.join("target/evidence")).unwrap();
        assert!(prepare(&fixture.0).is_err());
    }

    #[test]
    fn evidence_rejects_wrong_magic_regular_file_and_size_limits() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.file(NAMES[0])).unwrap();
        assert!(prepare(&fixture.0).is_err());
        fs::remove_dir(fixture.file(NAMES[0])).unwrap();
        fs::write(fixture.file(NAMES[0]), "not a PNG").unwrap();
        assert!(prepare(&fixture.0).is_err());
        let file = fs::File::create(fixture.file(NAMES[0])).unwrap();
        file.set_len(FILE_LIMIT + 1).unwrap();
        assert!(prepare(&fixture.0).is_err());
        for name in &NAMES[..2] {
            fs::write(fixture.file(name), PNG_SIGNATURE).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(fixture.file(name))
                .unwrap()
                .set_len(FILE_LIMIT)
                .unwrap();
        }
        fs::write(fixture.file(NAMES[2]), PNG_SIGNATURE).unwrap();
        assert!(prepare(&fixture.0).is_err());
    }
}
