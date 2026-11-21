//! Transporte CSV oficial versionado. Nenhum dado é gravado durante análise de lote.
use crate::{
    ActorId, CivilDate, EntityId,
    budgets::{
        BudgetLine, BudgetRevision, BudgetSchedule, BudgetState, BudgetVariation, PeriodBasis,
        SchedulePeriod,
    },
    decimal::{Percent, Quantity, UnitPrice},
    units::MeasureUnit,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, str::FromStr};

pub const FORMAT_VERSION: &str = "CANTEIRO-ORCAMENTO-CSV-1";
const HEADER: &[&str] = &[
    "tipo",
    "orcamento",
    "revisao",
    "estado",
    "responsavel",
    "variacao",
    "obra",
    "escopo",
    "grupo",
    "etapa",
    "subetapa",
    "id",
    "codigo",
    "descricao",
    "unidade",
    "quantidade",
    "preco",
    "catalogo",
    "edicao",
];
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_SERVICES: usize = 100_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumberLocale {
    Canonical,
    PtBr,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellError {
    pub row: usize,
    pub column: usize,
    pub cause: String,
}
impl std::fmt::Display for CellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Linha {}, coluna {}: {}",
            self.row, self.column, self.cause
        )
    }
}
impl std::error::Error for CellError {}
fn err(row: usize, col: usize, cause: &str) -> CellError {
    CellError {
        row,
        column: col,
        cause: cause.into(),
    }
}
pub(crate) fn safe(s: &str) -> bool {
    // Planilhas podem ignorar espaços e caracteres invisíveis antes de uma
    // fórmula. A verificação vale para entrada e saída, sem executar conteúdo.
    // Controles de direção e caracteres de largura zero podem esconder um
    // marcador de fórmula do revisor sem alterar sua aparência na planilha.
    // Rejeitamos esses caracteres em qualquer posição, não apenas no início.
    if s.chars().any(|c| {
        matches!(
            c,
            '\u{feff}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                | '\u{2060}' | '\u{2066}'..='\u{2069}' | '\0'
        )
    }) {
        return false;
    }
    let visible = s.trim_start_matches(char::is_whitespace);
    !visible
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '=' | '+' | '-' | '@'))
        && !s.chars().any(|c| c == '\0')
}
fn csv_row(cells: &[String], out: &mut String) {
    for (idx, cell) in cells.iter().enumerate() {
        if idx > 0 {
            out.push(';');
        }
        if cell.chars().any(|c| matches!(c, ';' | '"' | '\n' | '\r')) {
            out.push('"');
            out.push_str(&cell.replace('"', "\"\""));
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push_str("\r\n");
}
fn export_row(cells: &[String], out: &mut String, row_number: usize) -> Result<(), CellError> {
    for (col, value) in cells.iter().enumerate() {
        if !safe(value) {
            return Err(err(
                row_number,
                col + 1,
                "Campo pode ser interpretado como fórmula ao abrir a planilha.",
            ));
        }
    }
    csv_row(cells, out);
    Ok(())
}
fn canonical_to_locale(value: String, locale: NumberLocale) -> String {
    if locale == NumberLocale::PtBr {
        value.replace('.', ",")
    } else {
        value
    }
}
pub(crate) fn locale_number(
    value: &str,
    locale: NumberLocale,
    row: usize,
    col: usize,
) -> Result<String, CellError> {
    if value.is_empty() || !safe(value) {
        return Err(err(row, col, "Número inválido."));
    }
    if locale == NumberLocale::Canonical {
        if value.contains(',') {
            return Err(err(row, col, "Use ponto como separador decimal."));
        }
        return Ok(value.to_owned());
    }
    let (integer, fraction) = value.split_once(',').unwrap_or((value, ""));
    if !integer.chars().all(|c| c.is_ascii_digit() || c == '.')
        || !fraction.bytes().all(|c| c.is_ascii_digit())
        || value.matches(',').count() > 1
        || integer.is_empty()
        || (value.contains(',') && fraction.is_empty())
    {
        return Err(err(row, col, "Confira milhar e separador decimal pt-BR."));
    }
    let chunks: Vec<_> = integer.split('.').collect();
    if chunks.len() > 1
        && (chunks[0].is_empty()
            || chunks[0].len() > 3
            || !chunks.iter().skip(1).all(|g| g.len() == 3))
    {
        return Err(err(row, col, "Agrupamento de milhares inválido."));
    }
    let normalized = integer.replace('.', "");
    let value = if fraction.is_empty() {
        normalized
    } else {
        format!("{normalized}.{fraction}")
    };
    Ok(value)
}
pub(crate) fn parse_csv(
    input: &str,
    columns: usize,
    max_rows: usize,
) -> Result<Vec<Vec<String>>, CellError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    if input.len() > MAX_BYTES {
        return Err(err(1, 1, "Arquivo acima do limite de 8 MiB."));
    }
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;
    let mut closed = false;
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cell.push('"');
                } else {
                    quoted = false;
                    closed = true;
                }
            } else {
                cell.push(ch);
            }
            continue;
        }
        match ch {
            '"' if cell.is_empty() && !closed => quoted = true,
            ';' => {
                row.push(std::mem::take(&mut cell));
                closed = false;
            }
            '\n' => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                closed = false;
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                closed = false;
            }
            _ if closed => {
                return Err(err(
                    rows.len() + 1,
                    row.len() + 1,
                    "Texto após campo entre aspas.",
                ));
            }
            '"' => return Err(err(rows.len() + 1, row.len() + 1, "Aspas sem abertura.")),
            _ => cell.push(ch),
        }
        if rows.len() > max_rows {
            return Err(err(rows.len(), 1, "Limite de linhas excedido."));
        }
    }
    if quoted {
        return Err(err(
            rows.len() + 1,
            row.len() + 1,
            "Campo entre aspas não foi fechado.",
        ));
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    for (i, row) in rows.iter().enumerate() {
        if row.len() != columns {
            return Err(err(
                i + 1,
                row.len(),
                "Quantidade de colunas incompatível com o formato oficial.",
            ));
        }
        for (j, value) in row.iter().enumerate() {
            if value.len() > 10_000 {
                return Err(err(i + 1, j + 1, "Célula excede o limite."));
            }
            if i > 0 && !safe(value) {
                return Err(err(
                    i + 1,
                    j + 1,
                    "Conteúdo executável ou fórmula não permitido.",
                ));
            }
        }
    }
    Ok(rows)
}
fn fields() -> Vec<String> {
    HEADER.iter().map(|s| (*s).into()).collect()
}
fn row() -> Vec<String> {
    vec![String::new(); HEADER.len()]
}
pub fn export_csv(value: &BudgetRevision, locale: NumberLocale) -> Result<String, CellError> {
    value.validate().map_err(|e| err(1, 1, &e.to_string()))?;
    let mut out = String::from(FORMAT_VERSION);
    out.push_str("\r\n");
    csv_row(&fields(), &mut out);
    let mut meta = row();
    meta[0] = "META".into();
    meta[1] = value.budget_id.clone();
    meta[2] = value.revision.to_string();
    meta[3] = match value.status {
        BudgetState::Draft => "draft",
        BudgetState::Review => "review",
        BudgetState::Approved => "approved",
    }
    .into();
    meta[4] = value.responsible_id.to_string();
    export_row(&meta, &mut out, 3)?;
    for (index, v) in value.variations.iter().enumerate() {
        let mut r = row();
        r[0] = "VAR".into();
        r[5] = v.id.clone();
        r[6] = v.work_id.to_string();
        r[7] = v.scope.clone();
        export_row(&r, &mut out, 4 + index)?;
    }
    for (index, line) in value.lines.iter().enumerate() {
        let mut r = row();
        r[0] = "ITEM".into();
        r[5] = line.variation_id.clone();
        r[8] = line.group.clone();
        r[9] = line.stage.clone();
        r[10] = line.substage.clone();
        r[11] = line.id.clone();
        r[12] = line.code.clone();
        r[13] = line.description.clone();
        r[14] = line.unit.to_string();
        r[15] = canonical_to_locale(line.quantity.to_string(), locale);
        r[16] = canonical_to_locale(line.price.to_string(), locale);
        r[17] = line.catalog_id.clone().unwrap_or_default();
        r[18] = line
            .catalog_edition
            .map(|v| v.to_string())
            .unwrap_or_default();
        export_row(&r, &mut out, 4 + value.variations.len() + index)?;
    }
    if let Some(schedule) = &value.schedule {
        let mut entry = row();
        entry[0] = "SCHEDULE".into();
        entry[1] = schedule.starts_on.to_string();
        entry[2] = schedule.deadline_days.to_string();
        entry[3] = match schedule.basis {
            PeriodBasis::Fixed30Days => "fixed_30_days",
            PeriodBasis::CivilMonths => "civil_months",
        }
        .into();
        export_row(
            &entry,
            &mut out,
            4 + value.variations.len() + value.lines.len(),
        )?;
        for (index, period) in schedule.periods.iter().enumerate() {
            let mut entry = row();
            entry[0] = "PERIOD".into();
            entry[5] = period.id.clone();
            entry[6] = period.position.to_string();
            entry[15] = canonical_to_locale(period.percentage.to_string(), locale);
            export_row(
                &entry,
                &mut out,
                5 + value.variations.len() + value.lines.len() + index,
            )?;
        }
    }
    if out.len() > MAX_BYTES {
        return Err(err(1, 1, "Exportação acima do limite de 8 MiB."));
    }
    Ok(out)
}
pub fn preview_import(input: &str, locale: NumberLocale) -> Result<BudgetRevision, CellError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let rest = input
        .strip_prefix(FORMAT_VERSION)
        .and_then(|s| s.strip_prefix("\r\n").or_else(|| s.strip_prefix('\n')))
        .ok_or_else(|| err(1, 1, "Versão de formato não reconhecida."))?;
    let data = parse_csv(rest, HEADER.len(), MAX_SERVICES + 1000)?;
    if data.first() != Some(&fields()) {
        return Err(err(2, 1, "Cabeçalhos diferentes do formato oficial."));
    }
    let mut budget: Option<BudgetRevision> = None;
    let mut seen = BTreeSet::new();
    for (index, r) in data.iter().enumerate().skip(1) {
        let num = index + 2;
        match r[0].as_str() {
            "META" => {
                if budget.is_some() {
                    return Err(err(num, 1, "Metadados duplicados."));
                }
                let revision = r[2].parse().map_err(|_| err(num, 3, "Revisão inválida."))?;
                let status = match r[3].as_str() {
                    "draft" => BudgetState::Draft,
                    "review" => BudgetState::Review,
                    "approved" => BudgetState::Approved,
                    _ => return Err(err(num, 4, "Estado inválido.")),
                };
                budget = Some(BudgetRevision {
                    budget_id: r[1].clone(),
                    revision,
                    status,
                    responsible_id: ActorId::from_str(&r[4])
                        .map_err(|_| err(num, 5, "Responsável inválido."))?,
                    variations: vec![],
                    lines: vec![],
                    schedule: None,
                });
            }
            "VAR" => {
                let b = budget
                    .as_mut()
                    .ok_or_else(|| err(num, 1, "Metadados precisam vir primeiro."))?;
                if !seen.insert(r[5].clone()) {
                    return Err(err(num, 6, "Variação duplicada."));
                }
                b.variations.push(BudgetVariation {
                    id: r[5].clone(),
                    work_id: EntityId::from_str(&r[6])
                        .map_err(|_| err(num, 7, "Obra inválida."))?,
                    scope: r[7].clone(),
                });
            }
            "ITEM" => {
                let b = budget
                    .as_mut()
                    .ok_or_else(|| err(num, 1, "Metadados precisam vir primeiro."))?;
                if b.lines.len() >= MAX_SERVICES {
                    return Err(err(num, 1, "Limite de serviços excedido."));
                }
                let quantity: Quantity = locale_number(&r[15], locale, num, 16)?
                    .parse()
                    .map_err(|_| err(num, 16, "Quantidade fora da escala admitida."))?;
                let price: UnitPrice = locale_number(&r[16], locale, num, 17)?
                    .parse()
                    .map_err(|_| err(num, 17, "Preço fora da escala admitida."))?;
                b.lines.push(BudgetLine {
                    id: r[11].clone(),
                    variation_id: r[5].clone(),
                    group: r[8].clone(),
                    stage: r[9].clone(),
                    substage: r[10].clone(),
                    code: r[12].clone(),
                    description: r[13].clone(),
                    unit: MeasureUnit::from_str(&r[14])
                        .map_err(|_| err(num, 15, "Unidade desconhecida."))?,
                    quantity,
                    price,
                    catalog_id: (!r[17].is_empty()).then(|| r[17].clone()),
                    catalog_edition: if r[18].is_empty() {
                        None
                    } else {
                        Some(
                            r[18]
                                .parse()
                                .map_err(|_| err(num, 19, "Edição inválida."))?,
                        )
                    },
                });
            }
            "SCHEDULE" => {
                let b = budget
                    .as_mut()
                    .ok_or_else(|| err(num, 1, "Metadados precisam vir primeiro."))?;
                if b.schedule.is_some() {
                    return Err(err(num, 1, "O cronograma foi declarado mais de uma vez."));
                }
                let basis = match r[3].as_str() {
                    "fixed_30_days" => PeriodBasis::Fixed30Days,
                    "civil_months" => PeriodBasis::CivilMonths,
                    _ => return Err(err(num, 4, "Base de calendário desconhecida.")),
                };
                b.schedule = Some(BudgetSchedule {
                    starts_on: CivilDate::from_str(&r[1])
                        .map_err(|_| err(num, 2, "Data de início inválida."))?,
                    deadline_days: r[2]
                        .parse()
                        .map_err(|_| err(num, 3, "Prazo em dias inválido."))?,
                    basis,
                    periods: vec![],
                });
            }
            "PERIOD" => {
                let b = budget
                    .as_mut()
                    .ok_or_else(|| err(num, 1, "Metadados precisam vir primeiro."))?;
                let schedule = b
                    .schedule
                    .as_mut()
                    .ok_or_else(|| err(num, 1, "Declare o cronograma antes de seus períodos."))?;
                let percentage: Percent = locale_number(&r[15], locale, num, 16)?
                    .parse()
                    .map_err(|_| err(num, 16, "Percentual de período inválido."))?;
                schedule.periods.push(SchedulePeriod {
                    id: r[5].clone(),
                    position: r[6]
                        .parse()
                        .map_err(|_| err(num, 7, "Posição de período inválida."))?,
                    percentage,
                });
            }
            _ => return Err(err(num, 1, "Tipo de registro não reconhecido.")),
        }
    }
    let result = budget.ok_or_else(|| err(3, 1, "Metadados ausentes."))?;
    result.validate().map_err(|e| err(3, 1, &e.to_string()))?;
    result.summarize().map_err(|e| err(3, 1, &e.to_string()))?;
    Ok(result)
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImportDifference {
    pub included: usize,
    pub changed: usize,
    pub removed: usize,
}
pub fn compare(existing: &BudgetRevision, incoming: &BudgetRevision) -> ImportDifference {
    let old: std::collections::BTreeMap<_, _> = existing.lines.iter().map(|l| (&l.id, l)).collect();
    let new: std::collections::BTreeMap<_, _> = incoming.lines.iter().map(|l| (&l.id, l)).collect();
    ImportDifference {
        included: new.keys().filter(|id| !old.contains_key(*id)).count(),
        changed: new
            .iter()
            .filter(|(id, line)| old.get(*id).is_some_and(|other| *other != **line))
            .count(),
        removed: old.keys().filter(|id| !new.contains_key(*id)).count(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::budgets::{BudgetLine, BudgetState, BudgetVariation};
    fn sample(n: usize) -> BudgetRevision {
        let mut v = BudgetRevision {
            budget_id: "Teste CSV".into(),
            revision: 3,
            status: BudgetState::Review,
            responsible_id: format!("{:032x}", 1_u8).parse().unwrap(),
            variations: vec![BudgetVariation {
                id: "v1".into(),
                work_id: format!("{:032x}", 2_u8).parse().unwrap(),
                scope: "Obra fictícia".into(),
            }],
            lines: vec![],
            schedule: None,
        };
        for i in 0..n {
            v.lines.push(BudgetLine {
                id: format!("item-{i}"),
                variation_id: "v1".into(),
                group: "Pavimento".into(),
                stage: "Revestimento".into(),
                substage: "Preparação".into(),
                code: format!("C-{i}"),
                description: "Descrição técnica".into(),
                unit: MeasureUnit::SquareMetre,
                quantity: "1234.56".parse().unwrap(),
                price: "78.90".parse().unwrap(),
                catalog_id: Some("ref-2021".into()),
                catalog_edition: Some(3),
            });
        }
        v
    }
    #[test]
    fn schedule_survives_csv_roundtrip() {
        let mut original = sample(3);
        original.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                90,
                PeriodBasis::CivilMonths,
                3,
            )
            .unwrap(),
        );
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        assert!(csv.contains("SCHEDULE;2026-11-01;90;civil_months"));
        assert_eq!(preview_import(&csv, NumberLocale::PtBr).unwrap(), original);
    }
    #[test]
    fn formula_in_export_is_rejected_before_worksheet_open() {
        let mut original = sample(1);
        original.lines[0].description = "=HYPERLINK(1)".into();
        let error = export_csv(&original, NumberLocale::PtBr).unwrap_err();
        assert_eq!(error.column, 14);
    }
    #[test]
    fn brazilian_number_and_10000_services_roundtrip() {
        let original = sample(10_000);
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        assert!(csv.contains("1234,56") && csv.contains("78,900000"));
        let imported = preview_import(&csv, NumberLocale::PtBr).unwrap();
        assert_eq!(imported, original);
        assert_eq!(compare(&original, &imported).changed, 0);
    }
    #[test]
    fn a_single_invalid_cell_rejects_entire_batch() {
        let csv = export_csv(&sample(2), NumberLocale::PtBr).unwrap();
        let invalid = csv.replacen("1234,56", "1.23,456", 1);
        let error = preview_import(&invalid, NumberLocale::PtBr).unwrap_err();
        assert_eq!(error.column, 16);
        assert!(error.row >= 5);
        let formula = csv.replacen(
            "Descrição técnica",
            "=HYPERLINK(\\\"https://example.org\\\")",
            1,
        );
        assert!(preview_import(&formula, NumberLocale::PtBr).is_err());
    }
    #[test]
    fn hidden_formula_prefixes_are_rejected_in_both_directions() {
        let original = sample(1);
        let clean = export_csv(&original, NumberLocale::PtBr).unwrap();
        for prefix in [
            " ", "\t", "\u{feff}", "\u{200b}", "\u{200c}", "\u{200d}", "\u{200e}", "\u{202e}",
            "\u{2060}", "\u{2066}", "\u{00a0}",
        ] {
            let attack = format!("{prefix}=HYPERLINK(1)");
            let mut outgoing = original.clone();
            outgoing.lines[0].description = attack.clone();
            // A validação do orçamento pode recusar o texto antes de
            // materializar a linha CSV, portanto a coluna ainda não existe.
            assert!(export_csv(&outgoing, NumberLocale::PtBr).is_err());
            let incoming = clean.replacen("Descrição técnica", &attack, 1);
            assert_eq!(
                preview_import(&incoming, NumberLocale::PtBr)
                    .unwrap_err()
                    .column,
                14
            );
        }
        let mut benign = original.clone();
        benign.lines[0].description = "Materiais   conferidos".into();
        assert_eq!(
            preview_import(
                &export_csv(&benign, NumberLocale::PtBr).unwrap(),
                NumberLocale::PtBr
            )
            .unwrap(),
            benign
        );
    }
    #[test]
    fn formula_masked_inside_description_is_rejected() {
        let mut original = sample(1);
        for hidden in ['\u{200c}', '\u{200d}', '\u{200f}', '\u{202a}', '\u{2069}'] {
            original.lines[0].description = format!("Mobilização{hidden} administrativa");
            assert!(export_csv(&original, NumberLocale::Canonical).is_err());
        }
    }
    #[test]
    fn malformed_last_service_rejects_ten_thousand_line_batch() {
        let original = sample(10_000);
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        let corrupted = csv.replacen("C-9999", "=1+1", 1);
        let error = preview_import(&corrupted, NumberLocale::PtBr).unwrap_err();
        assert_eq!(error.column, 13);
        assert_eq!(error.row, 10_004);
        // A prévia não expõe nenhum subconjunto como importação válida.
        assert!(preview_import(&corrupted, NumberLocale::PtBr).is_err());
    }
    #[test]
    fn quote_and_delimiter_are_not_executable() {
        let mut original = sample(1);
        original.lines[0].description = "Materiais; \"revisados\"".into();
        let csv = export_csv(&original, NumberLocale::Canonical).unwrap();
        assert_eq!(
            preview_import(&csv, NumberLocale::Canonical).unwrap(),
            original
        );
    }
}
