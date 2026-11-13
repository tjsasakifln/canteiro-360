use super::decimal::{Exact, overflow_error};
use super::{
    DecimalKind, FieldError, format_decimal_pt_br, parse_date_pt_br, parse_decimal_pt_br,
    parse_unit,
};
use canteiro_domain::decimal::{CalculationPolicy, unit_measurement};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MeasurementQuery {
    pub page: u32,
    pub page_size: u32,
    pub search: String,
    pub sort: String,
    pub direction: String,
}

impl Default for MeasurementQuery {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 25,
            search: String::new(),
            sort: "code".into(),
            direction: "asc".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MeasurementRow {
    pub policy: CalculationPolicy,
    pub id: u32,
    pub code: String,
    pub description: String,
    pub unit: String,
    pub contracted: String,
    pub previous: String,
    pub current: String,
    pub difference: String,
    pub unit_price: String,
    pub period_value: String,
    pub balance: String,
    pub balance_value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MeasurementPage {
    pub organization: String,
    pub page: u32,
    pub page_size: u32,
    pub total: u32,
    pub total_pages: u32,
    pub search: String,
    pub sort: String,
    pub direction: String,
    pub rows: Vec<MeasurementRow>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeasurementForm {
    pub row_id: u32,
    pub current: String,
    pub unit_price: String,
    pub percent: String,
    pub date: String,
    pub unit: String,
    pub notes: String,
}

impl Default for MeasurementForm {
    fn default() -> Self {
        Self {
            row_id: 1,
            current: "35".into(),
            unit_price: "10,00".into(),
            percent: "5".into(),
            date: "12/11/2026".into(),
            unit: "m²".into(),
            notes: String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormFieldError {
    pub field: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormPreview {
    pub errors: Vec<FormFieldError>,
    /// Successfully interpreted fields remain available even when a sibling is
    /// invalid. Raw form input belongs to the caller and is never overwritten.
    pub canonical: BTreeMap<String, String>,
    pub preview: Option<MeasurementRow>,
}

fn organization_index(organization: &str) -> Result<usize, FieldError> {
    match organization {
        "aurora" => Ok(0),
        "horizonte" => Ok(1),
        _ => Err(FieldError::new(
            "not_found",
            "Esta empresa fictícia não está disponível. Volte ao exemplo da obra.",
        )),
    }
}

fn query_error(message: &str) -> FieldError {
    FieldError::new("query", message)
}

fn check_query(query: &MeasurementQuery) -> Result<(), FieldError> {
    if query.page == 0 || query.page > 10_000 {
        return Err(query_error("A página deve ficar entre 1 e 10000."));
    }
    if ![25, 50, 100].contains(&query.page_size) {
        return Err(query_error("Escolha 25, 50 ou 100 serviços por página."));
    }
    if query.search.chars().count() > 80 || query.search.chars().any(char::is_control) {
        return Err(query_error(
            "Use uma busca com até 80 caracteres, sem quebras de linha.",
        ));
    }
    if ![
        "code",
        "description",
        "unit",
        "contracted",
        "previous",
        "current",
        "difference",
        "unit_price",
        "period_value",
        "balance",
    ]
    .contains(&query.sort.as_str())
    {
        return Err(query_error(
            "Escolha uma coluna disponível para ordenar a tabela.",
        ));
    }
    if !["asc", "desc"].contains(&query.direction.as_str()) {
        return Err(query_error("Escolha ordenação crescente ou decrescente."));
    }
    Ok(())
}

/// The fixture exists only as a bounded, immutable demonstration. Browser code
/// must request a page from the server, not render or filter this entire set.
fn fixture(organization: &str) -> Result<&'static Vec<MeasurementRow>, FieldError> {
    static DATASETS: OnceLock<[Vec<MeasurementRow>; 2]> = OnceLock::new();
    let index = organization_index(organization)?;
    let sets = DATASETS.get_or_init(|| [build_fixture("AUR"), build_fixture("HOR")]);
    Ok(&sets[index])
}

fn build_fixture(prefix: &str) -> Vec<MeasurementRow> {
    (1..=10_000).map(|id| build_row(prefix, id)).collect()
}

// Generating one preview never allocates the full server-side dataset. This
// helper is the only fixture construction reachable from browser validation.
fn build_row(prefix: &str, id: u32) -> MeasurementRow {
    const SERVICES: [(&str, &str); 6] = [
        ("Alvenaria de vedação", "m²"),
        ("Componente demonstrativo", "un"),
        ("Concreto de fundação", "m³"),
        ("Tubulação de água", "m"),
        ("Armadura de aço", "kg"),
        ("Equipe de apoio", "h"),
    ];
    let (description, unit) = SERVICES[((id - 1) % SERVICES.len() as u32) as usize];
    let mut row = MeasurementRow {
        policy: CalculationPolicy::default(),
        id,
        code: format!("{prefix}-{id:05}"),
        description: format!("{description} {id:05}"),
        unit: unit.into(),
        contracted: "100".into(),
        previous: if id == 2 { "1".into() } else { "20".into() },
        current: if id == 2 { "2".into() } else { "35".into() },
        difference: String::new(),
        unit_price: if id == 2 {
            "0.335".into()
        } else {
            (10 + ((id - 1) % 7)).to_string()
        },
        period_value: String::new(),
        balance: String::new(),
        balance_value: String::new(),
    };
    fill_preview_values(&mut row).expect("The fixed demonstration is within its documented limits");
    row
}

pub fn demo_row(organization: &str, id: u32) -> Result<MeasurementRow, FieldError> {
    let index = organization_index(organization)?;
    if !(1..=10_000).contains(&id) {
        return Err(FieldError::new(
            "not_found",
            "Este serviço fictício não está disponível. Reabra a tabela.",
        ));
    }
    Ok(build_row(if index == 0 { "AUR" } else { "HOR" }, id))
}

fn sorted_matching_rows<'a>(
    organization: &str,
    query: &MeasurementQuery,
) -> Result<Vec<&'a MeasurementRow>, FieldError> {
    organization_index(organization)?;
    check_query(query)?;
    let needle = query.search.trim().to_lowercase();
    let mut rows: Vec<_> = fixture(organization)?
        .iter()
        .filter(|row| {
            needle.is_empty()
                || row.code.to_lowercase().contains(&needle)
                || row.description.to_lowercase().contains(&needle)
                || row.unit.to_lowercase().contains(&needle)
        })
        .collect();
    rows.sort_by(|a, b| {
        let order = match query.sort.as_str() {
            "code" => a.code.cmp(&b.code),
            "description" => a.description.cmp(&b.description),
            "unit" => a.unit.cmp(&b.unit),
            column => numeric_sort_value(a, column).cmp(&numeric_sort_value(b, column)),
        };
        let order = if query.direction == "desc" {
            order.reverse()
        } else {
            order
        };
        if order == Ordering::Equal {
            a.id.cmp(&b.id)
        } else {
            order
        }
    });
    Ok(rows)
}

fn numeric_sort_value(row: &MeasurementRow, column: &str) -> u128 {
    let value = match column {
        "contracted" => &row.contracted,
        "previous" => &row.previous,
        "current" => &row.current,
        "difference" => &row.difference,
        "unit_price" => &row.unit_price,
        "period_value" => &row.period_value,
        "balance" => &row.balance,
        _ => unreachable!("Sort columns were checked before use"),
    };
    Exact::parse(value)
        .and_then(|value| value.scaled(6))
        .expect("Fixture decimals fit in the checked scale")
}

pub fn demo_page(
    organization: &str,
    query: &MeasurementQuery,
) -> Result<MeasurementPage, FieldError> {
    let rows = sorted_matching_rows(organization, query)?;
    let total = rows.len() as u32;
    let total_pages = total.div_ceil(query.page_size).max(1);
    if query.page > total_pages {
        return Err(query_error(
            "Esta página não existe nos resultados. Volte à primeira página.",
        ));
    }
    let start = ((query.page - 1) * query.page_size) as usize;
    Ok(MeasurementPage {
        organization: organization.to_owned(),
        page: query.page,
        page_size: query.page_size,
        total,
        total_pages,
        search: query.search.trim().to_owned(),
        sort: query.sort.clone(),
        direction: query.direction.clone(),
        rows: rows
            .into_iter()
            .skip(start)
            .take(query.page_size as usize)
            .cloned()
            .collect(),
    })
}

/// Exports the filtered result set, explicitly independent of the current page.
/// Every textual cell comes from the fixed fixture, never from submitted notes
/// or the search string; decimal cells cannot become spreadsheet formulas.
pub fn demo_export(organization: &str, query: &MeasurementQuery) -> Result<String, FieldError> {
    let rows = sorted_matching_rows(organization, query)?;
    let mut output = String::from(
        "Empresa fictícia\tCódigo\tServiço\tUnidade\tQuantidade contratada (unidade da linha)\tAnterior acumulado (unidade da linha)\tAtual acumulado (unidade da linha)\tDiferença no período (unidade da linha)\tPreço para simulação (R$/unidade)\tValor do período (R$)\tSaldo de quantidade (unidade da linha)\tSaldo de valor (R$)\n",
    );
    for row in rows {
        let values = [
            row.contracted.as_str(),
            row.previous.as_str(),
            row.current.as_str(),
            row.difference.as_str(),
            row.unit_price.as_str(),
            row.period_value.as_str(),
            row.balance.as_str(),
            row.balance_value.as_str(),
        ];
        output.push_str(&format!(
            "{}\t{}\t{}\t{}",
            organization, row.code, row.description, row.unit
        ));
        for value in values {
            output.push('\t');
            output.push_str(&format_decimal_pt_br(value));
        }
        output.push('\n');
    }
    Ok(output)
}

fn push_field_error(errors: &mut Vec<FormFieldError>, field: &str, error: FieldError) {
    errors.push(FormFieldError {
        field: field.into(),
        code: error.code,
        message: error.message,
    });
}

/// Stateless validation of fictitious data. It returns a preview, never a
/// committed transaction, approval, database draft or financial recognition.
pub fn validate_measurement_form(
    organization: &str,
    form: &MeasurementForm,
) -> Result<FormPreview, FieldError> {
    let mut row = demo_row(organization, form.row_id)?;
    let mut result = FormPreview {
        errors: vec![],
        canonical: BTreeMap::new(),
        preview: None,
    };
    for (field, input, kind) in [
        ("current", &form.current, DecimalKind::Quantity),
        ("unit_price", &form.unit_price, DecimalKind::UnitPrice),
        ("percent", &form.percent, DecimalKind::Percent),
    ] {
        match parse_decimal_pt_br(input, kind) {
            Ok(value) => {
                result.canonical.insert(field.into(), value.canonical);
            }
            Err(error) => push_field_error(&mut result.errors, field, error),
        }
    }
    match parse_date_pt_br(&form.date) {
        Ok(value) => {
            result.canonical.insert("date".into(), value.canonical);
        }
        Err(error) => push_field_error(&mut result.errors, "date", error),
    }
    match parse_unit(&form.unit) {
        Ok(unit) if unit == row.unit => {
            result.canonical.insert("unit".into(), unit);
        }
        Ok(_) => push_field_error(
            &mut result.errors,
            "unit",
            FieldError::new(
                "unit_mismatch",
                &format!(
                    "Este serviço usa {}. A unidade não pode mudar sem converter as quantidades.",
                    row.unit
                ),
            ),
        ),
        Err(error) => push_field_error(&mut result.errors, "unit", error),
    }
    if form.notes.chars().count() > 1000
        || form
            .notes
            .chars()
            .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        push_field_error(
            &mut result.errors,
            "notes",
            FieldError::new(
                "length",
                "Use até 1000 caracteres de observação, sem caracteres de controle.",
            ),
        );
    } else {
        result.canonical.insert("notes".into(), form.notes.clone());
    }
    if result
        .canonical
        .get("unit_price")
        .is_some_and(|price| price != &row.unit_price)
        && form.notes.trim().is_empty()
    {
        push_field_error(
            &mut result.errors,
            "notes",
            FieldError::new(
                "required",
                "Explique nas observações por que o preço da simulação difere do preço apresentado na tabela.",
            ),
        );
    }

    if let Some(current) = result.canonical.get("current") {
        let actual = Exact::parse(current)?.scaled(6)?;
        let previous = Exact::parse(&row.previous)?.scaled(6)?;
        let contracted = Exact::parse(&row.contracted)?.scaled(6)?;
        if actual < previous {
            push_field_error(
                &mut result.errors,
                "current",
                FieldError::new(
                    "decreasing",
                    &format!(
                        "O acumulado atual não pode ser menor que {} {}. Uma redução exige correção autorizada.",
                        format_decimal_pt_br(&row.previous),
                        row.unit
                    ),
                ),
            );
        } else if actual > contracted {
            push_field_error(
                &mut result.errors,
                "current",
                FieldError::new(
                    "balance",
                    &format!(
                        "O acumulado permitido é de até {} {}. Restam {} {} para medir a partir do anterior de {} {}.",
                        format_decimal_pt_br(&row.contracted),
                        row.unit,
                        format_decimal_pt_br(
                            &Exact {
                                coefficient: contracted - previous,
                                scale: 6
                            }
                            .text(0)
                        ),
                        row.unit,
                        format_decimal_pt_br(&row.previous),
                        row.unit
                    ),
                ),
            );
        }
    }
    if result.errors.is_empty() {
        row.current = result.canonical["current"].clone();
        row.unit_price = result.canonical["unit_price"].clone();
        match fill_preview_values(&mut row) {
            Ok(()) => {
                result.preview = Some(row);
            }
            Err(error) => push_field_error(&mut result.errors, "unit_price", error),
        }
    }
    Ok(result)
}

pub fn preview_measurement(
    organization: &str,
    form: &MeasurementForm,
) -> Result<FormPreview, FieldError> {
    validate_measurement_form(organization, form)
}

fn fill_preview_values(row: &mut MeasurementRow) -> Result<(), FieldError> {
    let preview = unit_measurement(
        row.contracted.parse().map_err(|_| overflow_error())?,
        row.previous.parse().map_err(|_| overflow_error())?,
        row.current.parse().map_err(|_| overflow_error())?,
        row.unit_price.parse().map_err(|_| overflow_error())?,
    )
    .map_err(|_| overflow_error())?;
    row.policy = preview.policy;
    row.difference = preview.quantity.to_string();
    row.balance = preview.remaining_quantity.to_string();
    row.period_value = preview.period_value.to_string();
    row.balance_value = preview.remaining_value.to_string();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ten_thousand_row_dataset_returns_only_a_page_and_stable_server_order() {
        let first = demo_page("aurora", &MeasurementQuery::default()).unwrap();
        assert_eq!(
            (first.total, first.total_pages, first.rows.len()),
            (10_000, 400, 25)
        );
        assert_eq!(first.rows[0].code, "AUR-00001");
        assert_eq!(first.rows[24].id, 25);
        let second = demo_page(
            "aurora",
            &MeasurementQuery {
                page: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(second.rows[0].id, 26);
        let desc = demo_page(
            "horizonte",
            &MeasurementQuery {
                page_size: 100,
                direction: "desc".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(desc.rows[0].code, "HOR-10000");
        assert_eq!(desc.rows.len(), 100);
        let by_value = MeasurementQuery {
            sort: "unit_price".into(),
            ..Default::default()
        };
        let numeric = demo_page("aurora", &by_value).unwrap();
        assert_eq!(numeric.rows[0].id, 2);
        assert_eq!(numeric.rows[1].id, 1);
        assert_eq!(numeric.rows[2].id, 8);
        assert_eq!(numeric, demo_page("aurora", &by_value).unwrap());
    }

    #[test]
    fn filters_count_all_results_and_company_prefix_never_falls_back() {
        let query = MeasurementQuery {
            search: "Alvenaria".into(),
            ..Default::default()
        };
        let result = demo_page("aurora", &query).unwrap();
        assert_eq!(result.total, 1667);
        assert_eq!(result.rows[1].id, 7);
        assert!(result.rows.iter().all(|row| row.code.starts_with("AUR-") && row.description.starts_with("Alvenaria")));
        let no_results = demo_page(
            "horizonte",
            &MeasurementQuery {
                search: "AUR-00001".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(no_results.total, 0);
        assert!(no_results.rows.is_empty());
        assert!(demo_page("nao-existe", &MeasurementQuery::default()).is_err());
        for query in [
            MeasurementQuery {
                page: 0,
                ..Default::default()
            },
            MeasurementQuery {
                page: 401,
                ..Default::default()
            },
            MeasurementQuery {
                page_size: 10_000,
                ..Default::default()
            },
            MeasurementQuery {
                sort: "';DROP TABLE anything".into(),
                ..Default::default()
            },
            MeasurementQuery {
                direction: "DESC".into(),
                ..Default::default()
            },
            MeasurementQuery {
                search: "x".repeat(81),
                ..Default::default()
            },
        ] {
            assert!(demo_page("aurora", &query).is_err());
        }
    }

    #[test]
    fn period_is_the_difference_of_truncated_accumulated_values() {
        let first = validate_measurement_form("aurora", &MeasurementForm::default()).unwrap();
        assert!(first.errors.is_empty());
        let first = first.preview.unwrap();
        assert_eq!(
            (&first.previous, &first.current, &first.difference),
            (&"20".to_string(), &"35".to_string(), &"15".to_string())
        );
        assert_eq!(first.period_value, "150.00");
        assert_eq!(first.balance, "65");
        assert_eq!(first.balance_value, "650.00");
        let cent = validate_measurement_form(
            "aurora",
            &MeasurementForm {
                row_id: 2,
                current: "2".into(),
                unit_price: "0,335".into(),
                unit: "un".into(),
                ..Default::default()
            },
        )
        .unwrap()
        .preview
        .unwrap();
        assert_eq!(cent.difference, "1");
        assert_eq!(cent.period_value, "0.34");
        assert_eq!(cent.balance_value, "32.83");
    }

    #[test]
    fn invalid_fields_do_not_destroy_other_interpretations_or_accept_excess_balance() {
        let input = MeasurementForm {
            current: "1.234".into(),
            unit_price: "1.234,56".into(),
            notes: "Manter esta observação".into(),
            ..Default::default()
        };
        let result = validate_measurement_form("aurora", &input).unwrap();
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].field, "current");
        assert_eq!(result.errors[0].code, "ambiguous");
        assert_eq!(result.canonical["unit_price"], "1234.56");
        assert_eq!(result.canonical["notes"], "Manter esta observação");
        assert!(result.preview.is_none());
        assert_eq!(input.current, "1.234");
        for (current, code) in [("19", "decreasing"), ("100,000001", "balance")] {
            let result = validate_measurement_form(
                "aurora",
                &MeasurementForm {
                    current: current.into(),
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(
                result
                    .errors
                    .iter()
                    .any(|error| error.field == "current" && error.code == code)
            );
            assert!(result.preview.is_none());
        }
        let result = validate_measurement_form(
            "aurora",
            &MeasurementForm {
                unit: "m³".into(),
                date: "31/02/2026".into(),
                percent: "101".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(result.errors.len(), 3);
        assert!(
            result
                .errors
                .iter()
                .any(|error| error.code == "unit_mismatch")
        );
        assert!(
            validate_measurement_form(
                "aurora",
                &MeasurementForm {
                    row_id: 10001,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let conditional = validate_measurement_form(
            "aurora",
            &MeasurementForm {
                unit_price: "11,00".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            conditional
                .errors
                .iter()
                .any(|error| error.field == "notes" && error.code == "required")
        );
        assert!(conditional.preview.is_none());
        let explained = validate_measurement_form(
            "aurora",
            &MeasurementForm {
                unit_price: "11,00".into(),
                notes: "Comparar preço hipotético de fornecimento.".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(explained.errors.is_empty());
        assert_eq!(explained.preview.unwrap().period_value, "165.00");
    }

    #[test]
    fn filtered_export_includes_the_complete_result_once_with_safe_cells() {
        let tsv = demo_export(
            "aurora",
            &MeasurementQuery {
                search: "Alvenaria".into(),
                page: 2,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tsv.lines().count(), 1668);
        assert!(tsv.lines().nth(1).unwrap().contains("AUR-00001"));
        assert!(tsv.lines().last().unwrap().contains("AUR-09997"));
        assert!(tsv.lines().all(|line| line.split('\t').count() == 12));
        assert!(
            tsv.lines()
                .flat_map(|line| line.split('\t'))
                .all(|cell| !cell.starts_with(['=', '+', '-', '@']))
        );
        let full = demo_export("horizonte", &MeasurementQuery::default()).unwrap();
        assert_eq!(full.lines().count(), 10001);
        assert!(!full.contains("AUR-"));
    }
}
