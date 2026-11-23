//! Transporte CSV oficial versionado. Nenhum dado é gravado durante análise de lote.
use crate::{
    ActorId, CivilDate, EntityId,
    budgets::{
        BudgetLine, BudgetRevision, BudgetSchedule, BudgetState, BudgetVariation, PeriodBasis,
        SchedulePeriod, bounded,
    },
    decimal::{Percent, Quantity, UnitPrice},
    units::MeasureUnit,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    str::FromStr,
};

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
pub(crate) const MAX_BYTES: usize = 8 * 1024 * 1024;
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
        (c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
            || matches!(
                c,
                '\u{00ad}' | '\u{034f}' | '\u{061c}' | '\u{180e}' | '\u{feff}'
                    | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
                    | '\u{2060}'..='\u{206f}' | '\0'
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
) -> Result<(Vec<Vec<String>>, Vec<usize>), CellError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    if input.len() > MAX_BYTES {
        return Err(err(1, 1, "Arquivo acima do limite de 8 MiB."));
    }
    let mut rows = Vec::new();
    // Índice da linha física inicial, inclusive para campos com quebras de linha.
    let mut starts = Vec::new();
    let mut physical_line = 1_usize;
    let mut record_start = 1_usize;
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
                if ch == '\r' && chars.peek() != Some(&'\n') {
                    return Err(err(
                        physical_line,
                        row.len() + 1,
                        "Quebra de linha CR isolada não permitida; use LF ou CRLF.",
                    ));
                }
                if ch == '\n' {
                    physical_line += 1;
                }
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
                starts.push(record_start);
                physical_line += 1;
                record_start = physical_line;
                closed = false;
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
                starts.push(record_start);
                physical_line += 1;
                record_start = physical_line;
                closed = false;
            }
            '\r' => {
                return Err(err(
                    physical_line,
                    row.len() + 1,
                    "Quebra de linha CR isolada não permitida; use LF ou CRLF.",
                ));
            }
            _ if closed => {
                return Err(err(
                    physical_line,
                    row.len() + 1,
                    "Texto após campo entre aspas.",
                ));
            }
            '"' => return Err(err(physical_line, row.len() + 1, "Aspas sem abertura.")),
            _ => cell.push(ch),
        }
        if rows.len() > max_rows {
            return Err(err(physical_line, 1, "Limite de linhas excedido."));
        }
    }
    if quoted {
        return Err(err(
            physical_line,
            row.len() + 1,
            "Campo entre aspas não foi fechado.",
        ));
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
        starts.push(record_start);
    }
    // A última linha também conta, mesmo sem quebra de linha no fim.
    // O limite não pode depender de como o editor salvou o arquivo.
    if rows.len() > max_rows {
        return Err(err(record_start, 1, "Limite de linhas excedido."));
    }
    for (i, row) in rows.iter().enumerate() {
        if row.len() != columns {
            return Err(err(
                starts[i],
                row.len(),
                "Quantidade de colunas incompatível com o formato oficial.",
            ));
        }
        for (j, value) in row.iter().enumerate() {
            if value.len() > 10_000 {
                return Err(err(starts[i], j + 1, "Célula excede o limite."));
            }
            if i > 0 && !safe(value) {
                return Err(err(
                    starts[i],
                    j + 1,
                    "Conteúdo executável ou fórmula não permitido.",
                ));
            }
        }
    }
    Ok((rows, starts))
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
    // A declaração de versão também integra o limite do arquivo recebido.
    if input.len() > MAX_BYTES {
        return Err(err(1, 1, "Arquivo acima do limite de 8 MiB."));
    }
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let rest = input
        .strip_prefix(FORMAT_VERSION)
        .and_then(|s| s.strip_prefix("\r\n").or_else(|| s.strip_prefix('\n')))
        .ok_or_else(|| err(1, 1, "Versão de formato não reconhecida."))?;
    // A linha da versão foi removida antes de chamar o leitor de dados.
    // Ajuste o diagnóstico para corresponder à linha física do arquivo.
    let (data, starts) =
        parse_csv(rest, HEADER.len(), MAX_SERVICES + 1000).map_err(|mut error| {
            error.row += 1;
            error
        })?;
    if data.first() != Some(&fields()) {
        return Err(err(2, 1, "Cabeçalhos diferentes do formato oficial."));
    }
    let mut budget: Option<BudgetRevision> = None;
    let mut seen = BTreeSet::new();
    for (index, r) in data.iter().enumerate().skip(1) {
        // A versão fica na primeira linha física, antes dos registros CSV.
        let num = starts[index] + 1;
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
    // Os tipos numéricos e o formato já foram conferidos em todas as células.
    // Agora localizar a primeira falha estrutural na linha física do CSV, em vez
    // de atribuir toda falha de uma importação extensa aos metadados da linha 3.
    let mut service_ids = BTreeSet::new();
    let mut variation_scopes = BTreeSet::new();
    let mut period_ids = BTreeSet::new();
    let mut period_positions = BTreeMap::new();
    let mut service_rows = result.lines.iter();
    let variation_ids: BTreeSet<_> = result.variations.iter().map(|v| v.id.as_str()).collect();
    for (index, fields) in data.iter().enumerate().skip(1) {
        let row_number = starts[index] + 1;
        match fields[0].as_str() {
            "META" => {
                if !bounded(&fields[1], 120) {
                    return Err(err(row_number, 2, "Identificação do orçamento inválida."));
                }
                if result.revision == 0 {
                    return Err(err(row_number, 3, "A revisão precisa ser positiva."));
                }
            }
            "VAR" => {
                if !bounded(&fields[5], 120) {
                    return Err(err(row_number, 6, "Identificação da variação inválida."));
                }
                if !bounded(&fields[7], 500) {
                    return Err(err(row_number, 8, "Escopo da variação inválido."));
                }
                if !variation_scopes.insert((fields[6].as_str(), fields[7].as_str())) {
                    return Err(err(
                        row_number,
                        8,
                        "Obra e escopo repetidos em outra variação.",
                    ));
                }
            }
            "ITEM" => {
                // A avaliação monetária é identificada pela célula real, mesmo
                // quando há milhares de serviços antes da linha defeituosa.
                service_rows
                    .next()
                    .ok_or_else(|| err(row_number, 1, "Serviço ausente."))?
                    .total()
                    .map_err(|_| err(row_number, 17, "Total do serviço fora da escala admitida."))?;
                for (column, limit) in [
                    (9, 160),
                    (10, 160),
                    (11, 160),
                    (12, 120),
                    (13, 80),
                    (14, 800),
                ] {
                    if !bounded(&fields[column - 1], limit) {
                        return Err(err(
                            row_number,
                            column,
                            "Campo obrigatório ou tamanho inválido.",
                        ));
                    }
                }
                if !service_ids.insert(fields[11].as_str()) {
                    return Err(err(row_number, 12, "Identificador de serviço repetido."));
                }
                if !variation_ids.contains(fields[5].as_str()) {
                    return Err(err(row_number, 6, "Variação do serviço não encontrada."));
                }
                if fields[17].is_empty() != fields[18].is_empty() {
                    return Err(err(
                        row_number,
                        if fields[17].is_empty() { 18 } else { 19 },
                        "Informe catálogo e edição juntos, ou deixe ambos vazios.",
                    ));
                }
                if fields[18] == "0" {
                    return Err(err(
                        row_number,
                        19,
                        "A edição do catálogo precisa ser positiva.",
                    ));
                }
            }
            "SCHEDULE" => {
                if result
                    .schedule
                    .as_ref()
                    .is_some_and(|schedule| schedule.deadline_days == 0)
                {
                    return Err(err(row_number, 3, "O prazo deve ser positivo."));
                }
            }
            "PERIOD" => {
                if !bounded(&fields[5], 128) {
                    return Err(err(row_number, 6, "Identificação do período inválida."));
                }
                if !period_ids.insert(fields[5].as_str()) {
                    return Err(err(row_number, 6, "Identificador de período repetido."));
                }
                let position = fields[6]
                    .parse::<u32>()
                    .map_err(|_| err(row_number, 7, "Posição de período inválida."))?;
                if period_positions.insert(position, row_number).is_some() {
                    return Err(err(row_number, 7, "Posição de período repetida."));
                }
            }
            _ => {}
        }
    }
    for (expected, (position, row_number)) in period_positions.iter().enumerate() {
        if *position != expected as u32 {
            return Err(err(
                *row_number,
                7,
                "O cronograma deve usar posições consecutivas a partir de zero.",
            ));
        }
    }
    if let Some(schedule) = &result.schedule {
        let schedule_row = data
            .iter()
            .enumerate()
            .find(|(_, fields)| fields[0] == "SCHEDULE")
            .map(|(index, _)| starts[index] + 1)
            .unwrap_or(3);
        if schedule.periods.is_empty() {
            return Err(err(schedule_row, 1, "O cronograma precisa de períodos."));
        }
        // Datas, prazo, identificação e sequência já foram verificadas.
        // A inconsistência residual do rateio pertence ao percentual
        // do último período, não aos metadados da linha 3.
        if schedule.validate().is_err() {
            let last_period_row = data
                .iter()
                .enumerate()
                .rfind(|(_, fields)| fields[0] == "PERIOD")
                .map(|(index, _)| starts[index] + 1)
                .unwrap_or(schedule_row);
            return Err(err(last_period_row, 16, "A soma dos períodos deve ser 100%."));
        }
    }
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
/// Amostra limitada para comparação humana. A contagem inclui todas as linhas;
/// somente os detalhes são limitados para evitar respostas enormes com 10 mil itens.
pub const MAX_IMPORT_CHANGE_DETAILS: usize = 30;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImportLineChange {
    pub id: String,
    pub kind: &'static str,
    pub fields: Vec<&'static str>,
    pub before: Option<BudgetLine>,
    pub after: Option<BudgetLine>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ImportLineChanges {
    pub records: Vec<ImportLineChange>,
    pub omitted: usize,
    pub changed_metadata: Vec<&'static str>,
    pub variations_included: usize,
    pub variations_changed: usize,
    pub variations_removed: usize,
    pub previous_schedule: Option<BudgetSchedule>,
    pub imported_schedule: Option<BudgetSchedule>,
}

/// Compara uma importação com a revisão existente sem modificar nenhuma delas.
/// O conjunto é ordenado pelo identificador estável, não pela ordem no arquivo.
pub fn compare_details(
    existing: Option<&BudgetRevision>,
    incoming: &BudgetRevision,
) -> ImportLineChanges {
    let old: std::collections::BTreeMap<_, _> = existing
        .into_iter()
        .flat_map(|budget| budget.lines.iter())
        .map(|line| (&line.id, line))
        .collect();
    let new: std::collections::BTreeMap<_, _> =
        incoming.lines.iter().map(|line| (&line.id, line)).collect();
    let keys: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
    let mut records = Vec::new();
    let mut total = 0_usize;
    for key in keys {
        let before = old.get(key).copied();
        let after = new.get(key).copied();
        if before == after {
            continue;
        }
        total += 1;
        if records.len() >= MAX_IMPORT_CHANGE_DETAILS {
            continue;
        }
        let (kind, fields) = match (before, after) {
            (None, Some(_)) => ("included", Vec::new()),
            (Some(_), None) => ("removed", Vec::new()),
            (Some(a), Some(b)) => {
                let mut fields = Vec::new();
                if a.variation_id != b.variation_id {
                    fields.push("variação");
                }
                if a.group != b.group {
                    fields.push("grupo");
                }
                if a.stage != b.stage {
                    fields.push("etapa");
                }
                if a.substage != b.substage {
                    fields.push("subetapa");
                }
                if a.code != b.code {
                    fields.push("código");
                }
                if a.description != b.description {
                    fields.push("descrição");
                }
                if a.unit != b.unit {
                    fields.push("unidade");
                }
                if a.quantity != b.quantity {
                    fields.push("quantidade");
                }
                if a.price != b.price {
                    fields.push("preço");
                }
                if a.catalog_id != b.catalog_id {
                    fields.push("catálogo");
                }
                if a.catalog_edition != b.catalog_edition {
                    fields.push("edição");
                }
                ("changed", fields)
            }
            (None, None) => continue,
        };
        records.push(ImportLineChange {
            id: key.clone(),
            kind,
            fields,
            before: before.cloned(),
            after: after.cloned(),
        });
    }
    // As diferenças estruturais também afetam o aceite: não se limitam
    // aos serviços. Compare sem depender da ordem no arquivo importado.
    let previous_variations: std::collections::BTreeMap<_, _> = existing
        .into_iter()
        .flat_map(|budget| budget.variations.iter())
        .map(|variation| (&variation.id, variation))
        .collect();
    let imported_variations: std::collections::BTreeMap<_, _> = incoming
        .variations
        .iter()
        .map(|variation| (&variation.id, variation))
        .collect();
    let mut changed_metadata = Vec::new();
    if let Some(previous) = existing {
        if previous.budget_id != incoming.budget_id {
            changed_metadata.push("identificação do orçamento");
        }
        if previous.revision != incoming.revision {
            changed_metadata.push("revisão");
        }
        if previous.status != incoming.status {
            changed_metadata.push("estado");
        }
        if previous.responsible_id != incoming.responsible_id {
            changed_metadata.push("responsável");
        }
    }
    ImportLineChanges {
        omitted: total.saturating_sub(records.len()),
        records,
        changed_metadata,
        variations_included: imported_variations
            .keys()
            .filter(|id| !previous_variations.contains_key(*id))
            .count(),
        variations_changed: imported_variations
            .iter()
            .filter(|(id, variation)| {
                previous_variations
                    .get(*id)
                    .is_some_and(|previous| *previous != **variation)
            })
            .count(),
        variations_removed: previous_variations
            .keys()
            .filter(|id| !imported_variations.contains_key(*id))
            .count(),
        previous_schedule: existing.and_then(|budget| budget.schedule.clone()),
        imported_schedule: incoming.schedule.clone(),
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
    fn detailed_comparison_preserves_original_and_identifies_changed_fields() {
        let original = sample(3);
        let mut incoming = original.clone();
        incoming.lines[0].quantity = "2345.67".parse().unwrap();
        incoming.lines[0].description = "Nova descrição".into();
        incoming.lines.remove(1);
        incoming.lines.push(BudgetLine {
            id: "novo-servico".into(),
            ..original.lines[2].clone()
        });
        let comparison = compare_details(Some(&original), &incoming);
        assert_eq!(comparison.omitted, 0);
        assert_eq!(comparison.records.len(), 3);
        let changed = comparison
            .records
            .iter()
            .find(|row| row.id == "item-0")
            .unwrap();
        assert_eq!(changed.kind, "changed");
        assert_eq!(changed.fields, vec!["descrição", "quantidade"]);
        assert_eq!(changed.before.as_ref().unwrap(), &original.lines[0]);
        assert_eq!(changed.after.as_ref().unwrap(), &incoming.lines[0]);
        assert!(
            comparison
                .records
                .iter()
                .any(|row| row.id == "item-1" && row.kind == "removed")
        );
        assert!(
            comparison
                .records
                .iter()
                .any(|row| row.id == "novo-servico" && row.kind == "included")
        );
        assert_eq!(original.lines.len(), 3);
    }

    #[test]
    fn detailed_comparison_detects_variations_schedule_and_revision_metadata() {
        let mut previous = sample(2);
        previous.variations.push(BudgetVariation {
            id: "sem-servicos".into(),
            work_id: format!("{:032x}", 3_u8).parse().unwrap(),
            scope: "Frente sem serviços".into(),
        });
        previous.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                60,
                PeriodBasis::CivilMonths,
                2,
            )
            .unwrap(),
        );
        let mut imported = previous.clone();
        imported.revision += 1;
        imported.variations[0].scope = "Frente revisada".into();
        imported.variations.remove(1);
        imported.variations.push(BudgetVariation {
            id: "nova-frente".into(),
            work_id: format!("{:032x}", 4_u8).parse().unwrap(),
            scope: "Nova frente".into(),
        });
        imported.schedule.as_mut().unwrap().deadline_days = 90;

        let delta = compare_details(Some(&previous), &imported);
        assert_eq!(delta.records.len(), 0);
        assert_eq!(delta.changed_metadata, vec!["revisão"]);
        assert_eq!(
            (
                delta.variations_included,
                delta.variations_changed,
                delta.variations_removed
            ),
            (1, 1, 1)
        );
        assert_eq!(delta.previous_schedule, previous.schedule);
        assert_eq!(delta.imported_schedule, imported.schedule);
    }

    #[test]
    fn new_budget_preview_reports_structure_without_inventing_previous_schedule() {
        let mut imported = sample(10_000);
        imported.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                30,
                PeriodBasis::Fixed30Days,
                1,
            )
            .unwrap(),
        );
        let delta = compare_details(None, &imported);
        assert_eq!(delta.variations_included, 1);
        assert_eq!(delta.variations_changed, 0);
        assert_eq!(delta.variations_removed, 0);
        assert!(delta.changed_metadata.is_empty());
        assert!(delta.previous_schedule.is_none());
        assert_eq!(delta.imported_schedule, imported.schedule);
        assert_eq!(delta.omitted, 10_000 - MAX_IMPORT_CHANGE_DETAILS);
    }
    #[test]
    fn detailed_comparison_limits_display_but_counts_ten_thousand_services() {
        let incoming = sample(10_000);
        let comparison = compare_details(None, &incoming);
        assert_eq!(comparison.records.len(), MAX_IMPORT_CHANGE_DETAILS);
        assert_eq!(comparison.omitted, 10_000 - MAX_IMPORT_CHANGE_DETAILS);
        assert!(comparison.records.iter().all(|row| row.kind == "included"));
    }
    #[test]
    fn version_header_counts_toward_total_input_size() {
        let oversized = format!("{FORMAT_VERSION}\n{}", "x".repeat(MAX_BYTES));
        let error = preview_import(&oversized, NumberLocale::Canonical).unwrap_err();
        assert_eq!((error.row, error.column), (1, 1));
        assert_eq!(error.cause, "Arquivo acima do limite de 8 MiB.");
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
        for hidden in [
            '\u{00ad}', '\u{034f}', '\u{061c}', '\u{180e}', '\u{200c}', '\u{200d}', '\u{200f}',
            '\u{202a}', '\u{2061}', '\u{2064}', '\u{2069}',
        ] {
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
    fn structural_errors_report_actual_cells_in_ten_thousand_row_import() {
        let original = sample(10_000);
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        let duplicated_id = csv.replacen("item-9999;C-9999", "item-0;C-9999", 1);
        assert_ne!(duplicated_id, csv);
        let error = preview_import(&duplicated_id, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10_004, 12));
        assert!(error.cause.contains("repetido"));

        let without_edition = csv.replacen(";ref-2021;3\r\n", ";ref-2021;\r\n", 1);
        assert_ne!(without_edition, csv);
        let error = preview_import(&without_edition, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (5, 19));

        let missing_variation = csv.replacen(
            "ITEM;;;;;v1;;;Pavimento",
            "ITEM;;;;;v-inexistente;;;Pavimento",
            1,
        );
        assert_ne!(missing_variation, csv);
        let error = preview_import(&missing_variation, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (5, 6));
    }
    #[test]
    fn row_limit_applies_to_final_record_without_newline() {
        // Um registro final sem LF era aceito apesar de ultrapassar max_rows.
        let without_newline = "codigo;descricao\nA;regular";
        let error = parse_csv(without_newline, 2, 1).unwrap_err();
        assert_eq!(error.row, 2);
        assert_eq!(error.column, 1);
        assert!(error.cause.contains("Limite de linhas"));

        let with_newline = "codigo;descricao\nA;regular\n";
        assert!(parse_csv(with_newline, 2, 1).is_err());
        assert_eq!(parse_csv(without_newline, 2, 2).unwrap().0.len(), 2);
    }

    #[test]
    fn bare_carriage_return_cannot_hide_a_second_spreadsheet_record() {
        let malicious = "a;b\ncodigo;texto\r=HYPERLINK(1)\n";
        let error = parse_csv(malicious, 2, 10).unwrap_err();
        assert_eq!((error.row, error.column), (2, 2));
        assert!(error.cause.contains("CR isolada"));

        let old_mac_line_ending = "a;b\rc;d";
        assert!(parse_csv(old_mac_line_ending, 2, 10).is_err());

        let legitimate = "a;b\r\ncodigo;\"texto\r\nlegítimo\"\r\n";
        let (rows, starts) = parse_csv(legitimate, 2, 10).unwrap();
        assert_eq!(rows[1][1], "texto\r\nlegítimo");
        assert_eq!(starts, vec![1, 2]);
    }

    #[test]
    fn csv_reports_physical_lines_after_multiline_quoted_fields() {
        let wrong_columns = "a;b\r\n\"nota\nrevisada\";ok\r\nx;y;extra\r\n";
        let error = parse_csv(wrong_columns, 2, 10).unwrap_err();
        assert_eq!((error.row, error.column), (4, 3));

        let formula = "a;b\n\"nota\nrevisada\";ok\nx;=1+1\n";
        let error = parse_csv(formula, 2, 10).unwrap_err();
        assert_eq!((error.row, error.column), (4, 2));

        let unclosed = "a;b\n\"texto\nsem fechamento";
        let error = parse_csv(unclosed, 2, 10).unwrap_err();
        assert_eq!((error.row, error.column), (3, 1));
    }
    #[test]
    fn semantic_error_uses_physical_line_after_multiline_cell() {
        let csv = export_csv(&sample(2), NumberLocale::PtBr).unwrap();
        let mut changed = csv.replacen("Descrição técnica", "\"Descrição\n técnica\"", 1);
        let at = changed.rfind("78,900000").unwrap();
        changed.replace_range(at..at + "78,900000".len(), "preco-invalido");
        let error = preview_import(&changed, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (7, 17));
    }
    #[test]
    fn invalid_schedule_position_reports_real_csv_cell() {
        let mut original = sample(2);
        original.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                90,
                PeriodBasis::Fixed30Days,
                3,
            )
            .unwrap(),
        );
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        let missing_position = csv.replacen("periodo-3;2;", "periodo-3;99;", 1);
        assert_ne!(missing_position, csv);
        let error = preview_import(&missing_position, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10, 7));
        assert!(error.cause.contains("consecutivas"));

        let duplicate = csv.replacen("periodo-3;2;", "periodo-3;1;", 1);
        assert_ne!(duplicate, csv);
        let error = preview_import(&duplicate, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10, 7));
        assert!(error.cause.contains("repetida"));
        assert_eq!(preview_import(&csv, NumberLocale::PtBr).unwrap(), original);
    }

    #[test]
    fn schedule_errors_identify_the_real_cell_instead_of_budget_metadata() {
        let mut original = sample(2);
        original.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                90,
                PeriodBasis::Fixed30Days,
                3,
            )
            .unwrap(),
        );
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();

        let zero_deadline = csv.replacen("SCHEDULE;2026-11-01;90;", "SCHEDULE;2026-11-01;0;", 1);
        let error = preview_import(&zero_deadline, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (7, 3));

        let duplicated = csv.replacen("periodo-3;2;", "periodo-2;2;", 1);
        let error = preview_import(&duplicated, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10, 6));

        let (mut records, _) = parse_csv(
            csv.strip_prefix(FORMAT_VERSION)
                .unwrap()
                .strip_prefix("\r\n")
                .unwrap(),
            HEADER.len(),
            100,
        )
        .unwrap();
        records
            .iter_mut()
            .rfind(|record| record[0] == "PERIOD")
            .unwrap()[15] = "0".into();
        let mut bad_rateio = format!("{FORMAT_VERSION}\r\n");
        for record in &records {
            csv_row(record, &mut bad_rateio);
        }
        let error = preview_import(&bad_rateio, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10, 16));
        assert!(error.cause.contains("100%"));

        let no_periods = csv
            .lines()
            .filter(|line| !line.starts_with("PERIOD;"))
            .collect::<Vec<_>>()
            .join("\n");
        let error = preview_import(&no_periods, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (7, 1));

        assert_eq!(preview_import(&csv, NumberLocale::PtBr).unwrap(), original);
    }
    #[test]
    fn last_period_error_after_ten_thousand_services_reports_physical_cell() {
        let mut original = sample(10_000);
        original.schedule = Some(
            BudgetSchedule::linear(
                "2026-11-01".parse().unwrap(),
                90,
                PeriodBasis::Fixed30Days,
                3,
            )
            .unwrap(),
        );
        let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
        let corrupted = csv.replacen("periodo-3;2;", "periodo-2;2;", 1);
        let error = preview_import(&corrupted, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (10_008, 6));
        assert_eq!(preview_import(&csv, NumberLocale::PtBr).unwrap(), original);
    }
    #[test]
    fn control_chars_and_quoted_carriage_returns_reject_whole_batch() {
        let original = sample(10_000);
        let valid = export_csv(&original, NumberLocale::Canonical).unwrap();
        let mut invalid = valid.clone();
        let target = "Descrição técnica";
        let pos = invalid.rfind(target).unwrap();
        invalid.replace_range(pos..pos + target.len(), "Descrição\u{001b}[31m");
        let error = preview_import(&invalid, NumberLocale::Canonical).unwrap_err();
        assert_eq!((error.row, error.column), (10_004, 14));
        assert!(error.cause.contains("Conteúdo executável"));

        let mut quoted = valid;
        let pos = quoted.rfind(target).unwrap();
        quoted.replace_range(pos..pos + target.len(), "\"Nota\rmaliciosa\"");
        let error = preview_import(&quoted, NumberLocale::Canonical).unwrap_err();
        assert_eq!((error.row, error.column), (10_004, 14));
        assert!(error.cause.contains("CR isolada"));
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
