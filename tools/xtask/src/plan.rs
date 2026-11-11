use crate::Result;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, collections::BTreeSet, fs, path::Path};

const APPROVED_ISSUES: usize = 72;
const APPROVED_RULES: usize = 192;

#[derive(Clone, Deserialize)]
struct Backlog {
    schema_version: u32,
    product: String,
    repository: String,
    total_issues: usize,
    last_issue: usize,
    phases: Vec<Phase>,
    issues: Vec<Issue>,
}

#[derive(Clone, Deserialize)]
struct Phase {
    id: String,
    start: usize,
    end: usize,
}

#[derive(Clone, Deserialize)]
struct Issue {
    sequence: usize,
    title: String,
    name: String,
    goal: String,
    phase: String,
    dependencies: Vec<usize>,
    file: String,
    rule_ids: Vec<String>,
    github_number: usize,
    github_url: String,
}

#[derive(Clone, Deserialize)]
struct Matrix {
    schema_version: u32,
    rules: Vec<Value>,
}

#[derive(Clone, Deserialize)]
struct Source {
    id: String,
    url: String,
    retrieval_status: String,
    sha256: Option<String>,
}

fn json<T: serde::de::DeserializeOwned>(root: &Path, path: &str) -> Result<T> {
    Ok(serde_json::from_str(&fs::read_to_string(root.join(path))?)?)
}

pub fn validate(root: &Path) -> Result<()> {
    let backlog: Backlog = json(root, "docs/backlog.json")?;
    let matrix: Matrix = json(root, "docs/matriz.json")?;
    let sources: Vec<Source> = json(root, "docs/fontes/sicop.json")?;
    validate_documents(root, &backlog, &matrix, &sources)?;
    println!(
        "Plano aprovado: {} issues, {} regras, dependências, fontes e documentos coerentes.",
        backlog.issues.len(),
        matrix.rules.len()
    );
    Ok(())
}

fn validate_documents(
    root: &Path,
    backlog: &Backlog,
    matrix: &Matrix,
    sources: &[Source],
) -> Result<()> {
    if backlog.schema_version != 1 || matrix.schema_version != 1 {
        return Err("versão de esquema do plano desconhecida".into());
    }
    if backlog.product != "canteiro-360" || backlog.repository != "tjsasakifln/canteiro-360" {
        return Err("o plano aponta para outro produto ou repositório".into());
    }
    if backlog.total_issues != APPROVED_ISSUES
        || backlog.issues.len() != APPROVED_ISSUES
        || backlog.last_issue != APPROVED_ISSUES
        || matrix.rules.len() != APPROVED_RULES
    {
        return Err("quantidade diverge do plano aprovado: 72 issues e 192 regras".into());
    }
    let mut previous_end = 0;
    let mut phases = BTreeMap::new();
    for phase in &backlog.phases {
        if phase.start != previous_end + 1
            || phase.end < phase.start
            || phase.end > APPROVED_ISSUES
            || phases.insert(&phase.id, phase).is_some()
        {
            return Err(
                format!("fase inválida, duplicada ou sem continuidade: {}", phase.id).into(),
            );
        }
        previous_end = phase.end;
    }
    if previous_end != APPROVED_ISSUES {
        return Err("as fases não cobrem todo o plano".into());
    }
    let mut source_map = BTreeMap::new();
    for source in sources {
        if source_map.insert(source.id.as_str(), source).is_some()
            || !source.url.starts_with("https://")
            || !["downloaded", "failed"].contains(&source.retrieval_status.as_str())
            || source.retrieval_status == "downloaded"
                && !source.sha256.as_ref().is_some_and(|sha| {
                    sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit())
                })
        {
            return Err(format!("fonte inválida ou duplicada: {}", source.id).into());
        }
    }
    let mut rule_map = BTreeMap::new();
    let matrix_markdown = fs::read_to_string(root.join("docs/MATRIZ-RASTREABILIDADE.md"))?;
    for rule in &matrix.rules {
        let id = string(rule, "id")?;
        if rule_map.insert(id, rule).is_some() {
            return Err(format!("regra duplicada: {id}").into());
        }
        for field in ["rule", "private_rule", "acceptance", "locator", "source"] {
            if string(rule, field)?.trim().is_empty() {
                return Err(format!("regra {id} sem {field}").into());
            }
        }
        if !["Documentada", "Adaptada", "Decisão"].contains(&string(rule, "origin")?) {
            return Err(format!("regra {id} com origem desconhecida").into());
        }
        let issue_number = rule["issue"].as_u64().ok_or("regra sem issue primária")? as usize;
        if issue_number == 0 || issue_number > APPROVED_ISSUES {
            return Err(format!("regra {id} aponta para issue inexistente").into());
        }
        let source = string(rule, "source")?;
        if source.starts_with("https://") {
            if let Some(source_id) = rule.get("source_id").and_then(Value::as_str)
                && source_map.get(source_id).map(|s| s.url.as_str()) != Some(source)
            {
                return Err(format!("fonte primária divergente na regra {id}").into());
            }
        } else if source.starts_with("http:")
            || source.starts_with('/')
            || source.split('/').any(|part| part == "..")
            || !root.join(source).is_file()
        {
            return Err(format!("documento local inválido na regra {id}").into());
        }
        if let Some(additional) = rule.get("sources") {
            for source in additional
                .as_array()
                .ok_or("sources precisa ser uma lista")?
            {
                let reference = string(source, "id")?;
                let url = string(source, "url")?;
                if source_map.get(reference).map(|s| s.url.as_str()) != Some(url)
                    || string(source, "locator")?.trim().is_empty()
                {
                    return Err(format!("fonte complementar inválida na regra {id}").into());
                }
            }
        }
        let matrix_row = matrix_markdown.lines().find(|line| {
            line.starts_with(&format!("| [{id}]")) || line.starts_with(&format!("| {id} |"))
        });
        if !matrix_row.is_some_and(|line| line.contains(&format!("/issues/{issue_number})"))) {
            return Err(format!("matriz Markdown não liga {id} à issue {issue_number}").into());
        }
    }
    let mut primary_rules = BTreeMap::new();
    for file in ["contratos", "orcamentos-medicoes", "operacao", "produto"] {
        let rules: Vec<Value> = json(root, &format!("docs/regras/{file}.json"))?;
        for rule in rules {
            let id = string(&rule, "id")?.to_owned();
            if primary_rules.insert(id.clone(), rule).is_some() {
                return Err(format!("regra duplicada nos arquivos de origem: {id}").into());
            }
        }
    }
    if primary_rules.len() != rule_map.len()
        || primary_rules
            .iter()
            .any(|(id, rule)| rule_map.get(id.as_str()).copied() != Some(rule))
    {
        return Err("matriz JSON diverge das regras completas de origem".into());
    }
    let backlog_markdown = fs::read_to_string(root.join("docs/BACKLOG.md"))?;
    let mut covered = BTreeSet::new();
    for (index, issue) in backlog.issues.iter().enumerate() {
        let number = index + 1;
        let expected_dependencies = if number == 1 {
            vec![]
        } else {
            vec![number - 1]
        };
        let phase = phases
            .get(&issue.phase)
            .ok_or("issue aponta para fase inexistente")?;
        if issue.sequence != number
            || issue.github_number != number
            || issue.dependencies != expected_dependencies
            || issue.file != format!("docs/issues/{number:03}.md")
            || issue.title != format!("{number:02}. {}", issue.name)
            || issue.goal.trim().is_empty()
            || !(phase.start..=phase.end).contains(&number)
            || issue.github_url
                != format!("https://github.com/{}/issues/{number}", backlog.repository)
        {
            return Err(format!(
                "issue {number:02} com sequência, vínculo ou dependência divergente"
            )
            .into());
        }
        let body = fs::read_to_string(root.join(&issue.file))?;
        if body.lines().next() != Some(format!("# {}", issue.title).as_str())
            || !body.contains(&issue.goal)
            || !body.contains("## Critérios de aceite verificáveis")
            || !body.contains("## Evidências para fechar")
        {
            return Err(format!("documento da issue {number:02} diverge do manifesto").into());
        }
        if !backlog_markdown.lines().any(|line| {
            line.starts_with(&format!("| {number:02} |"))
                && line.contains(&issue.github_url)
                && line.contains(&format!("issues/{number:03}.md"))
        }) {
            return Err(
                format!("plano Markdown não liga a issue {number:02} à especificação").into(),
            );
        }
        for id in &issue.rule_ids {
            if !covered.insert(id.clone())
                || rule_map
                    .get(id.as_str())
                    .and_then(|rule| rule["issue"].as_u64())
                    != Some(number as u64)
                || !body.contains(&format!("| {id} |"))
            {
                return Err(
                    format!("regra {id} ausente, repetida ou ligada à issue incorreta").into(),
                );
            }
        }
    }
    if covered.len() != rule_map.len() {
        return Err("existem regras da matriz sem vínculo no plano de implementação".into());
    }
    Ok(())
}

fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("campo obrigatório ausente: {field}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (std::path::PathBuf, Backlog, Matrix, Vec<Source>) {
        let root = crate::workspace();
        let backlog = json(&root, "docs/backlog.json").expect("manifesto versionado");
        let matrix = json(&root, "docs/matriz.json").expect("matriz versionada");
        let sources = json(&root, "docs/fontes/sicop.json").expect("fontes versionadas");
        (root, backlog, matrix, sources)
    }

    #[test]
    fn approved_documents_cross_reference_each_other() {
        let (root, backlog, matrix, sources) = fixture();
        validate_documents(&root, &backlog, &matrix, &sources)
            .expect("plano aprovado precisa ser coerente");
    }

    #[test]
    fn same_count_with_cyclic_dependency_is_rejected() {
        let (root, mut backlog, matrix, sources) = fixture();
        backlog.issues[1].dependencies = vec![3];
        assert!(validate_documents(&root, &backlog, &matrix, &sources).is_err());
    }

    #[test]
    fn same_count_with_orphan_or_reassigned_rule_is_rejected() {
        let (root, mut backlog, matrix, sources) = fixture();
        backlog.issues[0].rule_ids = vec!["BR38".to_owned()];
        assert!(validate_documents(&root, &backlog, &matrix, &sources).is_err());
    }

    #[test]
    fn changed_acceptance_cannot_silently_diverge_from_source() {
        let (root, backlog, mut matrix, sources) = fixture();
        matrix.rules[0]["acceptance"] = Value::String("aprovar sem verificar".to_owned());
        assert!(validate_documents(&root, &backlog, &matrix, &sources).is_err());
    }

    #[test]
    fn unknown_primary_source_is_rejected() {
        let (root, backlog, mut matrix, sources) = fixture();
        matrix.rules[0]["source_id"] = Value::String("SIC-999".to_owned());
        assert!(validate_documents(&root, &backlog, &matrix, &sources).is_err());
    }
}
