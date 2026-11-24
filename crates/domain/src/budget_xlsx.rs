//! Exportação XLSX oficial, sem fórmulas, macros, relações externas ou perda decimal.
//! Exporta ZIP armazenado e admite também DEFLATE limitado, com partes fixas.
use crate::budget_csv::{CellError, FORMAT_VERSION, MAX_BYTES, NumberLocale, parse_csv, safe};
use std::fmt::Write;

pub const XLSX_VERSION: &str = "CANTEIRO-ORCAMENTO-XLSX-1";
const MAX_XML: usize = 64 * 1024 * 1024;
pub const MAX_PACKAGE_BYTES: usize = MAX_XML + 8192;
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
    let entries: Vec<_> = parts
        .iter()
        .map(|&(name, data)| (name, data, data, 0))
        .collect();
    zip_package(&entries)
}

fn zip_package(parts: &[(&str, &[u8], &[u8], u16)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for &(name, data, encoded, method) in parts {
        let offset = out.len() as u32;
        let size = data.len() as u32;
        let compressed = encoded.len() as u32;
        let crc = crc32(data);
        let name_len = name.len() as u16;
        u32le(&mut out, 0x0403_4b50);
        for v in [20, 0, method, 0, 33] {
            u16le(&mut out, v);
        }
        for v in [crc, compressed, size] {
            u32le(&mut out, v);
        }
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(encoded);

        u32le(&mut central, 0x0201_4b50);
        for v in [20, 20, 0, method, 0, 33] {
            u16le(&mut central, v);
        }
        for v in [crc, compressed, size] {
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

const PART_NAMES: [&str; 5] = [
    "[Content_Types].xml",
    "_rels/.rels",
    "xl/workbook.xml",
    "xl/_rels/workbook.xml.rels",
    "xl/worksheets/sheet1.xml",
];

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes(bytes[at..at + 2].try_into().expect("dois bytes"))
}

fn dword(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("quatro bytes"))
}

// Reject a dishonest advertised length, incomplete stream, additional compressed
// streams and trailing bytes. Allocate at most the advertised size plus one byte.
fn inflate_part(data: &[u8], size: usize) -> Option<Vec<u8>> {
    use flate2::{Decompress, FlushDecompress, Status};

    let mut decoder = Decompress::new(false);
    let mut output = vec![0; size.checked_add(1)?];
    let status = decoder
        .decompress(data, &mut output, FlushDecompress::Finish)
        .ok()?;
    if status != Status::StreamEnd
        || decoder.total_in() != data.len() as u64
        || decoder.total_out() != size as u64
    {
        return None;
    }
    output.truncate(size);
    Some(output)
}

/// Read only a single-disk ZIP32 with the five named parts, stored or DEFLATE.
/// Every local entry must agree with the central directory and CRC. No
/// descriptors, comments, extra fields, encryption, overlaps or hidden data.
fn unpack_official(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return None;
    }
    let end = bytes.len().checked_sub(22)?;
    let trailer = bytes.get(end..)?;
    if dword(trailer, 0) != 0x0605_4b50
        || word(trailer, 4) != 0
        || word(trailer, 6) != 0
        || word(trailer, 8) != 5
        || word(trailer, 10) != 5
        || word(trailer, 20) != 0
    {
        return None;
    }
    let central_start = dword(trailer, 16) as usize;
    let central_size = dword(trailer, 12) as usize;
    if central_start.checked_add(central_size)? != end {
        return None;
    }
    let mut central = bytes.get(central_start..end)?;
    let mut parts = vec![None; PART_NAMES.len()];
    let mut ranges = Vec::with_capacity(PART_NAMES.len());
    let mut total_size = 0_usize;
    for _ in PART_NAMES {
        let entry = central.get(..46)?;
        let flags = word(entry, 8);
        let method = word(entry, 10);
        let checksum = dword(entry, 16);
        let compressed = dword(entry, 20) as usize;
        let size = dword(entry, 24) as usize;
        let name_len = usize::from(word(entry, 28));
        let offset = dword(entry, 42) as usize;
        if dword(entry, 0) != 0x0201_4b50
            || word(entry, 6) > 20
            || flags & !0x0806 != 0
            || !matches!(method, 0 | 8)
            || (method == 0 && (compressed != size || flags & 6 != 0))
            || word(entry, 30) != 0
            || word(entry, 32) != 0
            || word(entry, 34) != 0
            || size > MAX_XML
        {
            return None;
        }
        total_size = total_size.checked_add(size)?;
        if total_size > MAX_PACKAGE_BYTES {
            return None;
        }
        let name = central.get(46..46 + name_len)?;
        let index = PART_NAMES.iter().position(|part| part.as_bytes() == name)?;
        let fixed_size = match index {
            0 => Some(CONTENT_TYPES.len()),
            1 => Some(ROOT_RELS.len()),
            2 => Some(WORKBOOK.len()),
            3 => Some(WORKBOOK_RELS.len()),
            _ => None,
        };
        if parts[index].is_some() || fixed_size.is_some_and(|expected| size != expected) {
            return None;
        }
        central = central.get(46 + name_len..)?;

        let local = bytes.get(offset..central_start)?;
        let header = local.get(..30)?;
        if dword(header, 0) != 0x0403_4b50
            || word(header, 4) != word(entry, 6)
            || word(header, 6) != flags
            || word(header, 8) != method
            || dword(header, 10) != dword(entry, 12)
            || dword(header, 14) != checksum
            || dword(header, 18) as usize != compressed
            || dword(header, 22) as usize != size
            || usize::from(word(header, 26)) != name_len
            || word(header, 28) != 0
            || local.get(30..30 + name_len)? != name
        {
            return None;
        }
        let data_start = 30 + name_len;
        let data_end = data_start.checked_add(compressed)?;
        let data = local.get(data_start..data_end)?;
        let decoded = if method == 8 {
            inflate_part(data, size)?
        } else {
            data.to_vec()
        };
        if crc32(&decoded) != checksum {
            return None;
        }
        parts[index] = Some(decoded);
        ranges.push((offset, offset.checked_add(data_end)?));
    }
    if !central.is_empty() {
        return None;
    }
    ranges.sort_unstable();
    let mut next = 0;
    for (start, end) in ranges {
        if start != next {
            return None;
        }
        next = end;
    }
    if next != central_start {
        return None;
    }
    parts.into_iter().collect()
}

fn official_sheet(bytes: &[u8]) -> Result<String, CellError> {
    let invalid = || error(1, 1, "Pacote XLSX fora do formato oficial seguro.");
    let mut parts = unpack_official(bytes).ok_or_else(invalid)?;
    if parts[0] != CONTENT_TYPES.as_bytes()
        || parts[1] != ROOT_RELS.as_bytes()
        || parts[2] != WORKBOOK.as_bytes()
        || parts[3] != WORKBOOK_RELS.as_bytes()
    {
        return Err(invalid());
    }
    String::from_utf8(parts.pop().ok_or_else(invalid)?).map_err(|_| invalid())
}

const SHEET_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#;
const SHEET_END: &str = "</sheetData></worksheet>";

fn xml_character(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
    )
}

fn xml_reference(reference: &str) -> Option<char> {
    let ch = match reference {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let digits = reference.strip_prefix('#')?;
            let (digits, radix) = match digits.strip_prefix('x') {
                Some(hex) => (hex, 16),
                None => (digits, 10),
            };
            if digits.is_empty()
                || digits.len() > 10
                || !digits.chars().all(|ch| ch.is_digit(radix))
            {
                return None;
            }
            char::from_u32(u32::from_str_radix(digits, radix).ok()?)?
        }
    };
    xml_character(ch).then_some(ch)
}

// ST_Xstring escaping is decoded once, AFTER XML references. In particular
// _x005F_x0041_ means the literal string "_x0041_", not "A".
fn xstring_escape(raw: &str) -> Option<(u16, &str)> {
    let bytes = raw.as_bytes().get(..7)?;
    if bytes[0] != b'_'
        || bytes[1] != b'x'
        || bytes[6] != b'_'
        || !bytes[2..6].iter().all(u8::is_ascii_hexdigit)
    {
        return None;
    }
    let unit = u16::from_str_radix(&raw[2..6], 16).ok()?;
    Some((unit, &raw[7..]))
}

fn decode_text(mut raw: &str, row: usize, col: usize) -> Result<String, CellError> {
    let invalid = || error(row, col, "Texto XML fora do formato oficial.");
    if raw.len() > 70_000 || raw.contains("]]>") {
        return Err(invalid());
    }
    let mut xml = String::new();
    while !raw.is_empty() {
        if let Some(reference) = raw.strip_prefix('&') {
            let (name, next) = reference.split_once(';').ok_or_else(invalid)?;
            xml.push(xml_reference(name).ok_or_else(invalid)?);
            raw = next;
        } else {
            let ch = raw.chars().next().ok_or_else(invalid)?;
            if ch == '<' || !xml_character(ch) {
                return Err(invalid());
            }
            raw = &raw[ch.len_utf8()..];
            // XML 1.0 normalizes literal CR/CRLF, but not numeric references.
            if ch == '\r' {
                xml.push('\n');
                raw = raw.strip_prefix('\n').unwrap_or(raw);
            } else {
                xml.push(ch);
            }
        }
    }

    let mut raw = xml.as_str();
    let mut value = String::new();
    while !raw.is_empty() {
        if let Some((unit, next)) = xstring_escape(raw) {
            let (ch, next) = if (0xd800..=0xdbff).contains(&unit) {
                let (low, tail) = xstring_escape(next).ok_or_else(invalid)?;
                if !(0xdc00..=0xdfff).contains(&low) {
                    return Err(invalid());
                }
                let scalar =
                    0x10000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(low) - 0xdc00);
                (char::from_u32(scalar).ok_or_else(invalid)?, tail)
            } else {
                (char::from_u32(u32::from(unit)).ok_or_else(invalid)?, next)
            };
            if !xml_character(ch) {
                return Err(invalid());
            }
            value.push(ch);
            raw = next;
        } else {
            let ch = raw.chars().next().ok_or_else(invalid)?;
            value.push(ch);
            raw = &raw[ch.len_utf8()..];
        }
    }
    if value.len() > 10_000 || !safe(&value) {
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
        let address = format!("<c r=\"{letter}{row}\"");
        let Some(suffix) = rest.strip_prefix(&address) else {
            continue;
        };
        if let Some(next) = suffix
            .strip_prefix(" t=\"inlineStr\"><is><t xml:space=\"preserve\">")
            .or_else(|| suffix.strip_prefix(" t=\"inlineStr\"><is><t>"))
        {
            let (raw, next) = next
                .split_once("</t></is></c>")
                .ok_or_else(|| invalid(index + 1))?;
            *cell = decode_text(raw, row, index + 1)?;
            *rest = next;
        } else if let Some(next) = [
            "/>",
            "></c>",
            " t=\"inlineStr\"><is><t/></is></c>",
            " t=\"inlineStr\"><is/></c>",
        ]
        .iter()
        .find_map(|prefix| suffix.strip_prefix(*prefix))
        {
            *rest = next;
        } else {
            return Err(invalid(index + 1));
        }
    }
    if let Some(next) = rest.strip_prefix("</row>") {
        *rest = next;
    } else {
        // Diagnostic only: never accept an extra, repeated or out-of-order cell.
        let column = rest
            .strip_prefix("<c r=\"")
            .and_then(|value| value.split_once('"'))
            .and_then(|(address, _)| {
                let letters = address.bytes().take_while(u8::is_ascii_uppercase);
                let column = letters.fold(0_usize, |acc, b| {
                    acc.saturating_mul(26)
                        .saturating_add(usize::from(b - b'A' + 1))
                });
                (column != 0).then_some(column)
            })
            .unwrap_or(1);
        return Err(invalid(column));
    }
    Ok(cells)
}

/// Reimports the versioned package, stored or DEFLATE, with canonical XML.
/// General Office XML variants are not supported by this strict reader.
/// Reuses the authoritative CSV semantic validation without persisting anything.
pub fn preview_import(bytes: &[u8]) -> Result<crate::budgets::BudgetRevision, CellError> {
    preview_import_with_locale(bytes).map(|(snapshot, _)| snapshot)
}

/// Also returns the decimal convention declared in the original workbook.
pub fn preview_import_with_locale(
    bytes: &[u8],
) -> Result<(crate::budgets::BudgetRevision, NumberLocale), CellError> {
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
        physical += cells
            .iter()
            .map(|value| value.bytes().filter(|&b| b == b'\n').count())
            .sum::<usize>()
            + 1;
        crate::budget_csv::csv_row(&cells, &mut csv);
        if csv.len() > MAX_BYTES {
            return Err(error(row, 1, "Dados XLSX acima do limite de importação."));
        }
        row += 1;
    }
    crate::budget_csv::preview_import(&csv, locale)
        .map(|snapshot| (snapshot, locale))
        .map_err(|mut err| {
            err.row = starts.partition_point(|&start| start <= err.row).max(1);
            err
        })
}

#[cfg(test)]
pub(crate) fn recompress_for_test(bytes: &[u8]) -> Vec<u8> {
    use flate2::{Compression, write::DeflateEncoder};
    use std::io::Write as _;

    let parts = unpack_official(bytes).unwrap();
    let compressed: Vec<_> = parts
        .iter()
        .map(|data| {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        })
        .collect();
    let entries: Vec<_> = PART_NAMES
        .into_iter()
        .zip(parts.iter().zip(&compressed))
        .map(|(name, (data, encoded))| (name, data.as_slice(), encoded.as_slice(), 8))
        .collect();
    zip_package(&entries)
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
    fn deflate_package_accepts_reordered_parts_and_rejects_false_lengths() {
        use flate2::{Compression, write::DeflateEncoder};
        use std::io::Write as _;

        let csv = format!(
            "{FORMAT_VERSION}\r\n{}\r\n",
            crate::budget_csv::HEADER.join(";")
        );
        let original = export_from_csv(&csv, NumberLocale::Canonical).unwrap();
        let sheet = official_sheet(&original).unwrap();
        let parts = [
            (PART_NAMES[4], sheet.as_bytes()),
            (PART_NAMES[0], CONTENT_TYPES.as_bytes()),
            (PART_NAMES[1], ROOT_RELS.as_bytes()),
            (PART_NAMES[2], WORKBOOK.as_bytes()),
            (PART_NAMES[3], WORKBOOK_RELS.as_bytes()),
        ];
        let compressed: Vec<_> = parts
            .iter()
            .map(|&(_, data)| {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data).unwrap();
                encoder.finish().unwrap()
            })
            .collect();
        let mut entries: Vec<_> = parts
            .iter()
            .zip(&compressed)
            .map(|(&(name, data), encoded)| (name, data, encoded.as_slice(), 8))
            .collect();
        assert_eq!(official_sheet(&zip_package(&entries)).unwrap(), sheet);

        let mut trailing = compressed[0].clone();
        trailing.push(0);
        entries[0].2 = &trailing;
        assert!(official_sheet(&zip_package(&entries)).is_err());
        entries[0].2 = &compressed[0];
        entries[0].1 = b"short";
        assert!(official_sheet(&zip_package(&entries)).is_err());
    }

    #[test]
    fn strict_xml_rejects_formula_markup_and_external_entities() {
        assert_eq!(
            decode_text("A&amp;B _x005F_x000D_", 3, 2).unwrap(),
            "A&B _x000D_"
        );
        for value in [
            "<f>1+1</f>",
            "&external;",
            "=1+1",
            "&#61;1+1",
            "&#x3D;1+1",
            "_x003D_1+1",
            "&#x5F;x003D_1+1",
            "_x0000_",
            "_x202E_abc",
            "&#xD800;",
            "_xD800_",
            "&#xFFFF;",
            "]]>",
        ] {
            assert!(decode_text(value, 3, 2).is_err(), "{value}");
        }
        let mut wrong = r#"<row r="3"><c r="A3"><f>1+1</f></c></row>"#;
        assert!(read_row(&mut wrong, 3).is_err());
    }

    #[test]
    fn standard_xml_and_office_text_escapes_preserve_data() {
        for (encoded, expected) in [
            ("&#x41;&#66;&quot;&apos;", "AB\"'"),
            ("A_x000D_B", "A\rB"),
            ("A\r\nB", "A\nB"),
            ("_xD83D__xDE00_", "\u{1f600}"),
            ("_x005F_x0041_", "_x0041_"),
            ("&amp;lt;", "&lt;"),
            ("&lt;f&gt;texto&lt;/f&gt;", "<f>texto</f>"),
        ] {
            assert_eq!(decode_text(encoded, 3, 2).unwrap(), expected);
        }
        assert!(decode_text("_xD83D__x0041_", 3, 2).is_err());
        assert!(decode_text("&#9999999999;", 3, 2).is_err());
    }

    #[test]
    fn inline_text_without_space_attribute_and_explicit_empty_cells_are_safe() {
        let mut row = r#"<row r="3"><c r="A3" t="inlineStr"><is><t>ITEM</t></is></c><c r="B3" t="inlineStr"><is><t></t></is></c></row>"#;
        let cells = read_row(&mut row, 3).unwrap();
        assert_eq!(cells[0], "ITEM");
        assert_eq!(cells[1], "");
        assert!(row.is_empty());

        let mut row = r#"<row r="3"><c r="A3"/><c r="B3"></c><c r="C3" t="inlineStr"><is><t/></is></c><c r="D3" t="inlineStr"><is/></c><c r="E3" t="inlineStr"><is><t>ITEM</t></is></c></row>"#;
        let cells = read_row(&mut row, 3).unwrap();
        assert!(cells[..4].iter().all(String::is_empty));
        assert_eq!(cells[4], "ITEM");
        assert!(row.is_empty());

        let mut formula = r#"<row r="3"><c r="A3" t="inlineStr"><is><t>=1+1</t></is></c></row>"#;
        assert!(read_row(&mut formula, 3).is_err());
        let mut markup = r#"<row r="3"><c r="A3" t="inlineStr"><is><t><f>1</f></t></is></c></row>"#;
        assert!(read_row(&mut markup, 3).is_err());
    }

    #[test]
    fn malformed_xlsx_cell_reports_its_actual_column() {
        let mut formula = r#"<row r="3"><c r="Q3"><f>1+1</f></c></row>"#;
        let error = read_row(&mut formula, 3).unwrap_err();
        assert_eq!((error.row, error.column), (3, 17));

        let mut extra = r#"<row r="4"><c r="AA4" t="n"><v>1</v></c></row>"#;
        let error = read_row(&mut extra, 4).unwrap_err();
        assert_eq!((error.row, error.column), (4, 27));

        let mut wrong_row = r#"<row r="5"><c r="N6" t="n"><v>1</v></c></row>"#;
        let error = read_row(&mut wrong_row, 5).unwrap_err();
        assert_eq!((error.row, error.column), (5, 14));
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
