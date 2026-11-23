//! Importação de custos sem persistir a prévia. A edição e a fonte são validadas separadamente.
//! Não interpreta fórmulas, nem converte unidades ou competências silenciosamente.
use crate::{
    budget_csv::{CellError, MAX_BYTES, NumberLocale, locale_number, parse_csv},
    catalogs::{CatalogItem, CostCategory, label},
    decimal::UnitPrice,
    units::MeasureUnit,
};
use std::{collections::BTreeSet, str::FromStr};

pub const FORMAT_VERSION: &str = "CANTEIRO-CUSTOS-CSV-1";
const HEADER: &[&str] = &["codigo", "descricao", "categoria", "unidade", "preco"];
const MAX_ITEMS: usize = 50_000;

fn issue(row: usize, column: usize, cause: &str) -> CellError {
    CellError {
        row,
        column,
        cause: cause.into(),
    }
}

/// Confere todas as células antes de devolver itens. Uma falha impede a edição inteira.
/// Cabeçalho fixo e formato versionado tornam a interpretação reproduzível.
pub fn preview_import(input: &str, locale: NumberLocale) -> Result<Vec<CatalogItem>, CellError> {
    if input.len() > MAX_BYTES {
        return Err(issue(1, 1, "Arquivo acima do limite de 8 MiB."));
    }
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let rest = input
        .strip_prefix(FORMAT_VERSION)
        .and_then(|value| {
            value
                .strip_prefix("\r\n")
                .or_else(|| value.strip_prefix('\n'))
        })
        .ok_or_else(|| issue(1, 1, "Versão do catálogo desconhecida."))?;
    // A primeira linha do arquivo declara a versão; os erros do leitor
    // de dados precisam apontar a linha física vista por quem importa.
    let (rows, starts) = parse_csv(rest, HEADER.len(), MAX_ITEMS + 1).map_err(|mut error| {
        error.row += 1;
        error
    })?;
    if rows.first() != Some(&HEADER.iter().map(|s| (*s).to_owned()).collect()) {
        return Err(issue(
            2,
            1,
            "Cabeçalho diferente do formato de preços oficial.",
        ));
    }
    if rows.len() <= 1 {
        return Err(issue(3, 1, "Inclua ao menos um preço."));
    }
    let mut seen = BTreeSet::new();
    let mut items = Vec::with_capacity(rows.len() - 1);
    for (offset, row) in rows.into_iter().enumerate().skip(1) {
        let line = starts[offset] + 1;
        // Informe a célula efetiva, e não a coluna do código, quando a
        // descrição está vazia, fora do limite ou contém controles.
        // Usa-se a mesma regra do domínio para não aceitar prévias divergentes.
        if !label(&row[0], 80) {
            return Err(issue(line, 1, "Código vazio ou inválido (máximo de 80 bytes)."));
        }
        if !label(&row[1], 800) {
            return Err(issue(line, 2, "Descrição vazia ou inválida (máximo de 800 bytes)."));
        }
        let category = match row[2].as_str() {
            "material" => CostCategory::Material,
            "labor" => CostCategory::Labor,
            "equipment" => CostCategory::Equipment,
            "service" => CostCategory::Service,
            "transport" => CostCategory::Transport,
            _ => return Err(issue(line, 3, "Categoria desconhecida.")),
        };
        let unit = MeasureUnit::from_str(&row[3])
            .map_err(|_| issue(line, 4, "Unidade desconhecida; informe o símbolo exato."))?;
        let price: UnitPrice = locale_number(&row[4], locale, line, 5)?
            .parse()
            .map_err(|_| issue(line, 5, "Preço fora da precisão ou faixa admitida."))?;
        let item = CatalogItem {
            code: row[0].clone(),
            description: row[1].clone(),
            category,
            unit,
            price,
        };
        item.validate()
            .map_err(|_| issue(line, 1, "Código, descrição ou preço inválidos."))?;
        if !seen.insert(item.code.clone()) {
            return Err(issue(line, 1, "Código duplicado na edição."));
        }
        items.push(item);
    }
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_catalog_including_version_header() {
        let oversized = format!("{FORMAT_VERSION}\n{}", "x".repeat(MAX_BYTES));
        let error = preview_import(&oversized, NumberLocale::Canonical).unwrap_err();
        assert_eq!((error.row, error.column), (1, 1));
        assert_eq!(error.cause, "Arquivo acima do limite de 8 MiB.");
    }

    #[test]
    fn preserves_quoted_description_and_exact_brazilian_price() {
        let csv = format!(
            "{FORMAT_VERSION}\r\ncodigo;descricao;categoria;unidade;preco\r\n\
             A;\"Cimento; comum\";material;kg;1.234,567890\r\n"
        );
        let items = preview_import(&csv, NumberLocale::PtBr).unwrap();
        assert_eq!(items[0].description, "Cimento; comum");
        assert_eq!(items[0].price.to_string(), "1234.567890");
        assert_eq!(items[0].unit, MeasureUnit::Kilogram);
    }

    #[test]
    fn refuses_executable_cells_duplicate_codes_and_mixed_locales() {
        let base = format!("{FORMAT_VERSION}\ncodigo;descricao;categoria;unidade;preco\n");
        for value in ["=1+1", " \u{200b}@SUM(1)", "+HYPERLINK(1)"] {
            let content = format!("{base}A;{value};service;m;2,00\n");
            assert_eq!(
                preview_import(&content, NumberLocale::PtBr)
                    .unwrap_err()
                    .row,
                3
            );
        }
        let repeated = format!("{base}A;Serviço;service;m;2,00\nA;Outro;service;m;3,00\n");
        let error = preview_import(&repeated, NumberLocale::PtBr).unwrap_err();
        assert_eq!((error.row, error.column), (4, 1));
        let mixed = format!("{base}A;Serviço;service;m;1,234\n");
        assert!(preview_import(&mixed, NumberLocale::Canonical).is_err());
    }

    #[test]
    fn imports_ten_thousand_rows_without_precision_loss() {
        let mut csv = format!("{FORMAT_VERSION}\ncodigo;descricao;categoria;unidade;preco\n");
        for n in 0..10_000 {
            csv.push_str(&format!("I{n};Serviço {n};service;m²;12,345678\n"));
        }
        let rows = preview_import(&csv, NumberLocale::PtBr).unwrap();
        assert_eq!(rows.len(), 10_000);
        assert_eq!(rows[9_999].code, "I9999");
        assert_eq!(rows[9_999].price.to_string(), "12.345678");
    }

    #[test]
    fn rejects_broken_header_or_unclosed_quote_as_a_whole() {
        let wrong = format!("{FORMAT_VERSION}\ncodigo;descricao;categoria;unidade;valor\n");
        assert_eq!(
            preview_import(&wrong, NumberLocale::PtBr).unwrap_err().row,
            2
        );
        let broken = format!(
            "{FORMAT_VERSION}\ncodigo;descricao;categoria;unidade;preco\nA;\"sem fim;service;m;1\n"
        );
        assert!(preview_import(&broken, NumberLocale::PtBr).is_err());
    }
}
