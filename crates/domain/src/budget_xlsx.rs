//! Exportação XLSX oficial, sem fórmulas, macros, relações externas ou perda decimal.
//! O pacote usa somente entradas ZIP armazenadas (sem compressão), com nomes fixos.
use crate::budget_csv::{CellError, FORMAT_VERSION, MAX_BYTES, NumberLocale, parse_csv, safe};
use std::fmt::Write;

pub const XLSX_VERSION: &str = "CANTEIRO-ORCAMENTO-XLSX-1";
const MAX_XML: usize = 64 * 1024 * 1024;
const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#;
const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#;
const WORKBOOK: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Orcamento" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
const WORKBOOK_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#;

fn error(row: usize, column: usize, cause: &str) -> CellError {
    CellError {
        row,
        column,
        cause: cause.into(),
    }
}

// As células são explicitamente texto, inclusive IDs e decimais. Excel não
// arredonda preços extensos, elimina zeros de códigos ou interpreta datas.
fn xml_text(out: &mut String, value: &str, row: usize, col: usize) -> Result<(), CellError> {
    for (offset, ch) in value.char_indices() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            '\u{fffe}' | '\u{ffff}' => {
                return Err(error(row, col, "Caractere inválido para XML."));
            }
            '_' => {
                // ST_Xstring reserva _xHHHH_; preservar uma sequência literal.
                let tail = value.as_bytes().get(offset + 1..offset + 7);
                if tail.is_some_and(|v| {
                    v[0] == b'x' && v[1..5].iter().all(u8::is_ascii_hexdigit) && v[5] == b'_'
                }) {
                    out.push_str("_x005F_");
                } else {
                    out.push('_');
                }
            }
            _ => out.push(ch),
        }
    }
    Ok(())
}

fn worksheet_cell(out: &mut String, row: usize, col: usize, value: &str) -> Result<(), CellError> {
    if value.is_empty() {
        return Ok(());
    }
    if !safe(value) {
        return Err(error(
            row,
            col,
            "Conteúdo executável ou fórmula não permitido.",
        ));
    }
    let letter = char::from(b'A' + (col - 1) as u8);
    write!(
        out,
        "<c r=\"{letter}{row}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">"
    )
    .expect("escrita em String");
    xml_text(out, value, row, col)?;
    out.push_str("</t></is></c>");
    Ok(())
}

/// Recebe exclusivamente o transporte CSV oficial já validado pelo serviço.
/// O marcador XLSX e a convenção decimal são explícitos nas células A1 e B1.
pub fn export_from_csv(csv: &str, locale: NumberLocale) -> Result<Vec<u8>, CellError> {
    if csv.len() > MAX_BYTES {
        return Err(error(1, 1, "Exportação acima do limite de 8 MiB."));
    }
    let rest = csv
        .strip_prefix(FORMAT_VERSION)
        .and_then(|s| s.strip_prefix("\r\n"))
        .ok_or_else(|| error(1, 1, "Versão CSV inválida para exportação XLSX."))?;
    let (rows, _) = parse_csv(rest, 19, 111_000)?;
    if rows.first().is_none_or(|row| {
        row.iter()
            .map(String::as_str)
            .ne(crate::budget_csv::HEADER.iter().copied())
    }) {
        return Err(error(2, 1, "Cabeçalho oficial do orçamento inválido."));
    }
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1">"#,
    );
    worksheet_cell(&mut sheet, 1, 1, XLSX_VERSION)?;
    let locale = match locale {
        NumberLocale::Canonical => "canonical",
        NumberLocale::PtBr => "pt_br",
    };
    worksheet_cell(&mut sheet, 1, 2, locale)?;
    sheet.push_str("</row>");
    for (index, cells) in rows.iter().enumerate() {
        let row = index + 2;
        write!(sheet, "<row r=\"{row}\">").expect("escrita em String");
        for (index, value) in cells.iter().enumerate() {
            worksheet_cell(&mut sheet, row, index + 1, value)?;
        }
        sheet.push_str("</row>");
        if sheet.len() > MAX_XML {
            return Err(error(row, 1, "Planilha XLSX acima do limite de 64 MiB."));
        }
    }
    sheet.push_str("</sheetData></worksheet>");
    Ok(stored_zip(&[
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes()),
        ("_rels/.rels", ROOT_RELS.as_bytes()),
        ("xl/workbook.xml", WORKBOOK.as_bytes()),
        ("xl/_rels/workbook.xml.rels", WORKBOOK_RELS.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ]))
}

fn u16le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn u32le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

// ZIP32 armazenado, com tamanho e CRC conhecidos antes de escrever cada cabeçalho.
// Somente chamado com cinco partes fixas e XML limitado a 64 MiB.
fn stored_zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in parts {
        let offset = out.len() as u32;
        let size = data.len() as u32;
        let crc = crc32(data);
        let name_len = name.len() as u16;
        u32le(&mut out, 0x0403_4b50);
        for v in [20, 0, 0, 0, 33] {
            u16le(&mut out, v);
        }
        for v in [crc, size, size] {
            u32le(&mut out, v);
        }
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        u32le(&mut central, 0x0201_4b50);
        for v in [20, 20, 0, 0, 0, 33] {
            u16le(&mut central, v);
        }
        for v in [crc, size, size] {
            u32le(&mut central, v);
        }
        for v in [name_len, 0, 0, 0, 0] {
            u16le(&mut central, v);
        }
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    let length = central.len() as u32;
    out.extend_from_slice(&central);
    u32le(&mut out, 0x0605_4b50);
    for v in [0, 0, parts.len() as u16, parts.len() as u16] {
        u16le(&mut out, v);
    }
    u32le(&mut out, length);
    u32le(&mut out, start);
    u16le(&mut out, 0);
    out
}

/// Extracts only the five fixed parts of the official stored ZIP package.
/// Compressed archives and alternate layouts are deliberately not accepted here.
/// Checking the complete canonical package also verifies the central directory,
/// local headers, CRCs, entry order, and absence of trailing or additional data.
fn official_sheet(bytes: &[u8]) -> Result<&str, CellError> {
    let invalid = || error(1, 1, "Pacote XLSX fora do formato oficial armazenado.");
    if bytes.len() > MAX_XML + 8192 {
        return Err(invalid());
    }
    let mut rest = bytes;
    let mut parts = Vec::with_capacity(5);
    for name in [
        "[Content_Types].xml",
        "_rels/.rels",
        "xl/workbook.xml",
        "xl/_rels/workbook.xml.rels",
        "xl/worksheets/sheet1.xml",
    ] {
        let header = rest.get(..30).ok_or_else(invalid)?;
        if &header[..4] != b"PK\x03\x04" {
            return Err(invalid());
        }
        let size = u32::from_le_bytes(header[18..22].try_into().expect("quatro bytes")) as usize;
        if size > MAX_XML {
            return Err(invalid());
        }
        let start = 30 + name.len();
        let end = start.checked_add(size).ok_or_else(invalid)?;
        if rest.get(30..start) != Some(name.as_bytes()) {
            return Err(invalid());
        }
        let data = rest.get(start..end).ok_or_else(invalid)?;
        parts.push((name, data));
        rest = rest.get(end..).ok_or_else(invalid)?;
    }
    if parts[0].1 != CONTENT_TYPES.as_bytes()
        || parts[1].1 != ROOT_RELS.as_bytes()
        || parts[2].1 != WORKBOOK.as_bytes()
        || parts[3].1 != WORKBOOK_RELS.as_bytes()
        || stored_zip(&parts) != bytes
    {
        return Err(invalid());
    }
    std::str::from_utf8(parts[4].1).map_err(|_| invalid())
}

const SHEET_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#;
const SHEET_END: &str = "</sheetData></worksheet>";

fn decode_text(mut raw: &str, row: usize, col: usize) -> Result<String, CellError> {
    let original = raw;
    let invalid = || error(row, col, "Texto XML fora do formato oficial.");
    if raw.len() > 70_000 {
        return Err(invalid());
    }
    let mut value = String::new();
    while !raw.is_empty() {
        let mut decoded = false;
        for (encoded, ch) in [
            ("&amp;", '&'),
            ("&lt;", '<'),
            ("&gt;", '>'),
            ("&#13;", '\r'),
            ("_x005F_", '_'),
        ] {
            if let Some(next) = raw.strip_prefix(encoded) {
                value.push(ch);
                raw = next;
                decoded = true;
                break;
            }
        }
        if !decoded {
            let ch = raw.chars().next().ok_or_else(invalid)?;
            value.push(ch);
            raw = &raw[ch.len_utf8()..];
        }
    }
    if value.len() > 10_000 || !safe(&value) {
        return Err(invalid());
    }
    let mut canonical = String::new();
    xml_text(&mut canonical, &value, row, col)?;
    if canonical != original {
        return Err(invalid());
    }
    Ok(value)
}

fn read_row(rest: &mut &str, row: usize) -> Result<Vec<String>, CellError> {
    let invalid = |col| error(row, col, "Linha ou célula fora do formato XLSX oficial.");
    let prefix = format!("<row r=\"{row}\">");
    *rest = rest.strip_prefix(&prefix).ok_or_else(|| invalid(1))?;
    let mut cells = vec![String::new(); crate::budget_csv::HEADER.len()];
    for (index, cell) in cells.iter_mut().enumerate() {
        let letter = char::from(b'A' + index as u8);
        let prefix = format!(
            "<c r=\"{letter}{row}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">"
        );
        if let Some(next) = rest.strip_prefix(&prefix) {
            let (raw, next) = next
                .split_once("</t></is></c>")
                .ok_or_else(|| invalid(index + 1))?;
            *cell = decode_text(raw, row, index + 1)?;
            if cell.is_empty() {
                return Err(invalid(index + 1));
            }
            *rest = next;
        }
    }
    *rest = rest.strip_prefix("</row>").ok_or_else(|| invalid(1))?;
    Ok(cells)
}

/// Reimports the exact versioned stored package emitted by export_from_csv.
/// General Office ZIP/XML variants are not supported by this strict reader.
/// Reuses the authoritative CSV semantic validation without persisting anything.
pub fn preview_import(bytes: &[u8]) -> Result<crate::budgets::BudgetRevision, CellError> {
    let sheet = official_sheet(bytes)?;
    let mut rest = sheet
        .strip_prefix(SHEET_START)
        .ok_or_else(|| error(1, 1, "Cabeçalho XML não reconhecido."))?;
    let version = read_row(&mut rest, 1)?;
    if version[0] != XLSX_VERSION || version[2..].iter().any(|v| !v.is_empty()) {
        return Err(error(1, 1, "Versão XLSX inválida."));
    }
    let locale = match version[1].as_str() {
        "canonical" => NumberLocale::Canonical,
        "pt_br" => NumberLocale::PtBr,
        _ => return Err(error(1, 2, "Convenção decimal XLSX inválida.")),
    };
    let mut csv = format!("{FORMAT_VERSION}\r\n");
    let mut starts = vec![1];
    let mut physical = 2;
    let mut row = 2;
    while rest != SHEET_END {
        if row > 101_001 {
            return Err(error(row, 1, "Limite de registros XLSX excedido."));
        }
        let cells = read_row(&mut rest, row)?;
        starts.push(physical);
        for (index, value) in cells.iter().enumerate() {
            if index > 0 {
                csv.push(';');
            }
            csv.push('"');
            csv.push_str(&value.replace('"', "\"\""));
            csv.push('"');
            physical += value.bytes().filter(|&b| b == b'\n').count();
        }
        csv.push_str("\r\n");
        physical += 1;
        if csv.len() > MAX_BYTES {
            return Err(error(row, 1, "Dados XLSX acima do limite de importação."));
        }
        row += 1;
    }
    crate::budget_csv::preview_import(&csv, locale).map_err(|mut err| {
        err.row = starts.partition_point(|&start| start <= err.row).max(1);
        err
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_archive_rejects_tampered_and_additional_parts() {
        let csv = format!(
            "{FORMAT_VERSION}\r\n{}\r\n",
            crate::budget_csv::HEADER.join(";")
        );
        let bytes = export_from_csv(&csv, NumberLocale::Canonical).unwrap();
        assert!(official_sheet(&bytes).unwrap().contains(XLSX_VERSION));
        let mut corrupted = bytes.clone();
        corrupted[14] ^= 1;
        assert!(official_sheet(&corrupted).is_err());
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(official_sheet(&trailing).is_err());
        assert!(official_sheet(&bytes[..bytes.len() - 1]).is_err());
        let mut compressed = bytes.clone();
        compressed[8] = 8;
        assert!(official_sheet(&compressed).is_err());
        let mut directory = bytes.clone();
        let end = directory.len() - 22;
        directory[end + 10] = 6;
        assert!(official_sheet(&directory).is_err());
    }

    #[test]
    fn strict_xml_rejects_formula_markup_and_external_entities() {
        assert_eq!(
            decode_text("A&amp;B _x005F_x000D_", 3, 2).unwrap(),
            "A&B _x000D_"
        );
        for value in ["<f>1+1</f>", "&external;", "=1+1", "_x000D_", "&#x41;"] {
            assert!(decode_text(value, 3, 2).is_err());
        }
        let mut wrong = r#"<row r="3"><c r="A3"><f>1+1</f></c></row>"#;
        assert!(read_row(&mut wrong, 3).is_err());
    }

    #[test]
    fn crc_matches_zip_reference_vector() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn text_is_escaped_without_creating_formula_or_external_relationship() {
        let mut out = String::new();
        worksheet_cell(&mut out, 5, 14, "A&B <f> _x000D_\r").unwrap();
        assert_eq!(
            out,
            r#"<c r="N5" t="inlineStr"><is><t xml:space="preserve">A&amp;B &lt;f&gt; _x005F_x000D_&#13;</t></is></c>"#
        );
        assert!(worksheet_cell(&mut out, 5, 14, "\u{ffff}").is_err());
    }

    #[test]
    fn ten_thousand_rows_are_emitted_with_exact_text_and_fixed_package_parts() {
        let mut csv = format!("{FORMAT_VERSION}\r\n");
        csv.push_str(&crate::budget_csv::HEADER.join(";"));
        csv.push_str("\r\n");
        for _ in 0..10_000 {
            csv.push_str("ITEM;;;;;;;;;;;;00001;Descrição & texto;m;1,234567;1234,56;;\r\n");
        }
        let bytes = export_from_csv(&csv, NumberLocale::PtBr).unwrap();
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        assert_eq!(&bytes[bytes.len() - 22..bytes.len() - 18], b"PK\x05\x06");
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("CANTEIRO-ORCAMENTO-XLSX-1"));
        assert!(raw.contains("pt_br"));
        assert!(raw.contains(r#"<c r="M10002" t="inlineStr"><is><t xml:space="preserve">00001"#));
        assert!(raw.contains(r#"<c r="Q10002" t="inlineStr"><is><t xml:space="preserve">1234,56"#));
        assert!(!raw.contains("<f>"));
        assert!(!raw.contains("TargetMode=\"External\""));
    }
}
