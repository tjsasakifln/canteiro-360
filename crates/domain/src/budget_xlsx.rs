//! Exportação XLSX oficial, sem fórmulas, macros, relações externas ou perda decimal.
//! Exporta ZIP armazenado e admite também DEFLATE limitado, com partes fixas.
use crate::budget_csv::{
    CellError, FORMAT_VERSION, MAX_BYTES, MAX_RECORDS, NumberLocale, parse_csv, safe,
};
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
    let (rows, _) = parse_csv(rest, SHEET_COLUMNS, MAX_RECORDS).map_err(|mut error| {
        // A linha da versão também ocupa a primeira linha do transporte.
        error.row += 1;
        error
    })?;
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
        if sheet.len() > MAX_XML - "</sheetData></worksheet>".len() {
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

const PART_NAMES: [&str; 6] = [
    "[Content_Types].xml",
    "_rels/.rels",
    "xl/workbook.xml",
    "xl/_rels/workbook.xml.rels",
    "xl/worksheets/sheet1.xml",
    "xl/sharedStrings.xml",
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

/// Read only a single-disk ZIP32 with the named parts, stored or DEFLATE.
/// Every local entry must agree with the central directory and CRC. No
/// unchecked descriptors, comments, extra fields, encryption, overlaps or hidden data.
fn unpack_official(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return None;
    }
    let end = bytes.len().checked_sub(22)?;
    let trailer = bytes.get(end..)?;
    let count = usize::from(word(trailer, 10));
    if dword(trailer, 0) != 0x0605_4b50
        || word(trailer, 4) != 0
        || word(trailer, 6) != 0
        || !matches!(count, 5 | 6)
        || usize::from(word(trailer, 8)) != count
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
    let names = &PART_NAMES[..count];
    let mut parts = vec![None; count];
    let mut ranges = Vec::with_capacity(count);
    let mut total_size = 0_usize;
    for _ in names {
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
            || flags & !0x080e != 0
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
        let index = names.iter().position(|part| part.as_bytes() == name)?;
        if parts[index].is_some() || (index < 4 && size > 8192) {
            return None;
        }
        central = central.get(46 + name_len..)?;

        let local = bytes.get(offset..central_start)?;
        let header = local.get(..30)?;
        let descriptor = flags & 8 != 0;
        let local_values = [dword(header, 14), dword(header, 18), dword(header, 22)];
        let central_values = [checksum, compressed as u32, size as u32];
        if dword(header, 0) != 0x0403_4b50
            || word(header, 4) != word(entry, 6)
            || word(header, 6) != flags
            || word(header, 8) != method
            || dword(header, 10) != dword(entry, 12)
            || !(local_values == central_values || (descriptor && local_values == [0; 3]))
            || usize::from(word(header, 26)) != name_len
            || word(header, 28) != 0
            || local.get(30..30 + name_len)? != name
        {
            return None;
        }
        let data_start = 30 + name_len;
        let data_end = data_start.checked_add(compressed)?;
        let data = local.get(data_start..data_end)?;
        let entry_end = if descriptor {
            // ZIP32 streaming writers may use a signed or unsigned descriptor.
            // Never scan for its signature: the central directory fixes its offset.
            let tail = local.get(data_end..)?;
            let matches_values = |raw: &[u8]| {
                raw.get(..12).is_some_and(|raw| {
                    [dword(raw, 0), dword(raw, 4), dword(raw, 8)] == central_values
                })
            };
            let length =
                if tail.starts_with(&0x0807_4b50_u32.to_le_bytes()) && matches_values(&tail[4..]) {
                    16
                } else if matches_values(tail) {
                    12
                } else {
                    return None;
                };
            data_end.checked_add(length)?
        } else {
            data_end
        };
        let decoded = if method == 8 {
            inflate_part(data, size)?
        } else {
            data.to_vec()
        };
        if crc32(&decoded) != checksum {
            return None;
        }
        parts[index] = Some(decoded);
        ranges.push((offset, offset.checked_add(entry_end)?));
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

const SHARED_CONTENT_TYPE: &str = r#"<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>"#;
const SHARED_RELATIONSHIP: &str = r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/>"#;

fn shared_content_types() -> String {
    CONTENT_TYPES.replace("</Types>", &format!("{SHARED_CONTENT_TYPE}</Types>"))
}

fn shared_workbook_rels() -> String {
    WORKBOOK_RELS.replace(
        "</Relationships>",
        &format!("{SHARED_RELATIONSHIP}</Relationships>"),
    )
}

// Aceita apenas declarações XML 1.0 em UTF-8 conhecidas, além de BOM e
// espaços de apresentação nas extremidades. O conteúdo estrutural continua
// sujeito à lista fechada de partes e à comparação integral.
fn xml_body(xml: &str) -> &str {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let xml = [
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#,
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<?xml version="1.0"?>"#,
    ]
    .into_iter()
    .find_map(|prefix| xml.strip_prefix(prefix))
    .unwrap_or(xml);
    xml.trim_matches([' ', '\t', '\r', '\n'])
}

// Only inter-element XML whitespace is optional in the fixed, text-free parts.
// Attribute values, element names, relationships and ordering remain exact.
fn xml_space(raw: &str) -> &str {
    raw.trim_start_matches([' ', '\t', '\r', '\n'])
}

fn same_xml(actual: &[u8], expected: &str) -> bool {
    std::str::from_utf8(actual).is_ok_and(|xml| {
        xml_body(xml)
            .split_inclusive('>')
            .map(xml_space)
            .eq(xml_body(expected).split_inclusive('>').map(xml_space))
    })
}

fn official_workbook(bytes: &[u8]) -> Result<(String, Vec<String>), CellError> {
    let invalid = || error(1, 1, "Pacote XLSX fora do formato oficial seguro.");
    let mut parts = unpack_official(bytes).ok_or_else(invalid)?;
    let has_shared = parts.len() == 6;
    let content_types = if has_shared {
        shared_content_types()
    } else {
        CONTENT_TYPES.to_owned()
    };
    let relationships = if has_shared {
        shared_workbook_rels()
    } else {
        WORKBOOK_RELS.to_owned()
    };
    if !same_xml(&parts[0], &content_types)
        || !same_xml(&parts[1], ROOT_RELS)
        || !same_xml(&parts[2], WORKBOOK)
        || !same_xml(&parts[3], &relationships)
    {
        return Err(invalid());
    }
    let shared = if has_shared {
        let xml = parts.pop().ok_or_else(invalid)?;
        read_shared_strings(std::str::from_utf8(&xml).map_err(|_| invalid())?)?
    } else {
        Vec::new()
    };
    let sheet = String::from_utf8(parts.pop().ok_or_else(invalid)?).map_err(|_| invalid())?;
    Ok((sheet, shared))
}

#[cfg(test)]
fn official_sheet(bytes: &[u8]) -> Result<String, CellError> {
    official_workbook(bytes).map(|(sheet, _)| sheet)
}

const SHEET_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#;
// O marcador de versão ocupa uma linha adicional em relação aos registros CSV.
const MAX_SHEET_ROWS: usize = MAX_RECORDS + 1;
const SHEET_COLUMNS: usize = 19;

// Metadata is advisory only: never use it to allocate or to skip cells.
fn bounded_index(raw: &str, maximum: usize) -> Option<usize> {
    if raw.is_empty() || raw.len() > 6 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value = raw.parse::<usize>().ok()?;
    (1..=maximum).contains(&value).then_some(value)
}

fn read_dimension(rest: &mut &str) -> Option<Option<usize>> {
    let Some(tail) = rest.strip_prefix(r#"<dimension ref="A1:S"#) else {
        return Some(None);
    };
    let (last_row, tail) = tail.split_once(r#""/>"#)?;
    let last_row = bounded_index(last_row, MAX_SHEET_ROWS)?;
    *rest = tail;
    Some(Some(last_row))
}

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

fn decode_xml_text(mut raw: &str, row: usize, col: usize) -> Result<String, CellError> {
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
    if value.len() > 10_000 {
        return Err(invalid());
    }
    Ok(value)
}

fn decode_text(raw: &str, row: usize, col: usize) -> Result<String, CellError> {
    let value = decode_xml_text(raw, row, col)?;
    if !safe(&value) {
        return Err(error(row, col, "Texto XML fora do formato oficial."));
    }
    Ok(value)
}

// Accept a bounded plain-text shared string table; rich text, phonetic runs,
// external references and arbitrary XML remain outside this official subset.
const SHARED_START: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main""#;
const MAX_SHARED_STRINGS: usize = 250_000;

fn read_shared_strings(xml: &str) -> Result<Vec<String>, CellError> {
    let invalid = || error(1, 1, "Tabela de textos XLSX fora do formato seguro.");
    let mut rest = xml_body(xml)
        .strip_prefix(xml_body(SHARED_START))
        .ok_or_else(invalid)?;
    let mut unique = None;
    if let Some(tail) = rest.strip_prefix(" count=\"") {
        let (count, tail) = tail.split_once('"').ok_or_else(invalid)?;
        let count = count.parse::<usize>().map_err(|_| invalid())?;
        if count > MAX_SHEET_ROWS * SHEET_COLUMNS {
            return Err(invalid());
        }
        rest = tail;
    }
    if let Some(tail) = rest.strip_prefix(" uniqueCount=\"") {
        let (count, tail) = tail.split_once('"').ok_or_else(invalid)?;
        let count = count.parse::<usize>().map_err(|_| invalid())?;
        if count > MAX_SHARED_STRINGS {
            return Err(invalid());
        }
        unique = Some(count);
        rest = tail;
    }
    rest = rest.strip_prefix('>').ok_or_else(invalid)?;
    let mut values = Vec::new();
    loop {
        rest = xml_space(rest);
        if rest == "</sst>" {
            break;
        }
        if values.len() >= MAX_SHARED_STRINGS {
            return Err(invalid());
        }
        let next = xml_space(rest.strip_prefix("<si>").ok_or_else(invalid)?);
        if let Some(tail) = next.strip_prefix("<t/>") {
            rest = xml_space(tail).strip_prefix("</si>").ok_or_else(invalid)?;
            values.push(String::new());
            continue;
        }
        let next = next
            .strip_prefix("<t xml:space=\"preserve\">")
            .or_else(|| next.strip_prefix("<t>"))
            .ok_or_else(invalid)?;
        let (raw, tail) = next.split_once("</t>").ok_or_else(invalid)?;
        rest = xml_space(tail).strip_prefix("</si>").ok_or_else(invalid)?;
        // Conteúdo executável é recusado no endereço da célula que o referencia.
        // Entradas não utilizadas também serão verificadas antes de aceitar o lote.
        values.push(decode_xml_text(raw, 1, 1)?);
    }
    if unique.is_some_and(|expected| expected != values.len()) {
        return Err(invalid());
    }
    Ok(values)
}

// Native SpreadsheetML numbers use a dot regardless of the workbook locale.
// Only fields declared numeric by the CSV schema may use this representation.
fn native_number(
    raw: &str,
    record_type: &str,
    index: usize,
    locale: NumberLocale,
) -> Option<String> {
    let decimal = matches!((record_type, index), ("ITEM", 15 | 16) | ("PERIOD", 15));
    let integer = matches!(
        (record_type, index),
        ("META" | "SCHEDULE", 2) | ("ITEM", 18) | ("PERIOD", 6)
    );
    if (!decimal && !integer) || raw.len() > 128 {
        return None;
    }
    let (mantissa, exponent) = match raw.split_once(['e', 'E']) {
        Some((mantissa, exponent)) if decimal => {
            let shift = exponent.parse::<i32>().ok()?;
            if !(-32..=32).contains(&shift) {
                return None;
            }
            (mantissa, Some(shift))
        }
        Some(_) => return None,
        None => (raw, None),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if whole.is_empty()
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
        || (mantissa.contains('.') && (!decimal || fraction.is_empty()))
    {
        return None;
    }

    // Move the decimal point using digits, never an f64 or rounded value.
    let canonical = if let Some(shift) = exponent {
        let digits = format!("{whole}{fraction}");
        let point = whole.len() as i32 + shift;
        if point <= 0 {
            format!("0.{}{digits}", "0".repeat((-point) as usize))
        } else {
            let point = point as usize;
            if point >= digits.len() {
                format!("{digits}{}", "0".repeat(point - digits.len()))
            } else {
                format!("{}.{}", &digits[..point], &digits[point..])
            }
        }
    } else {
        mantissa.to_owned()
    };
    Some(if decimal && locale == NumberLocale::PtBr {
        canonical.replace('.', ",")
    } else {
        canonical
    })
}

// Permit formatting whitespace between known XML elements, never inside
// literal text. The closing tags must still follow immediately in order.
fn inline_cell(mut rest: &str, row: usize, col: usize) -> Result<(String, &str), CellError> {
    let invalid = || error(row, col, "Linha ou célula fora do formato XLSX oficial.");
    rest = xml_space(rest);
    if let Some(tail) = rest.strip_prefix("<is/>") {
        let tail = xml_space(tail).strip_prefix("</c>").ok_or_else(invalid)?;
        return Ok((String::new(), tail));
    }
    rest = xml_space(rest.strip_prefix("<is>").ok_or_else(invalid)?);
    let (value, tail) = if let Some(tail) = rest.strip_prefix("<t/>") {
        (String::new(), tail)
    } else {
        let text = rest
            .strip_prefix("<t xml:space=\"preserve\">")
            .or_else(|| rest.strip_prefix("<t>"))
            .ok_or_else(invalid)?;
        let (raw, tail) = text.split_once("</t>").ok_or_else(invalid)?;
        (decode_text(raw, row, col)?, tail)
    };
    let tail = xml_space(tail).strip_prefix("</is>").ok_or_else(invalid)?;
    let tail = xml_space(tail).strip_prefix("</c>").ok_or_else(invalid)?;
    Ok((value, tail))
}

fn cell_value(rest: &str) -> Option<(&str, &str)> {
    let rest = xml_space(rest).strip_prefix("<v>")?;
    let (raw, tail) = rest.split_once("</v>")?;
    let tail = xml_space(tail).strip_prefix("</c>")?;
    Some((raw, tail))
}

fn read_row(
    rest: &mut &str,
    row: usize,
    locale: NumberLocale,
    shared: &[String],
) -> Result<Vec<String>, CellError> {
    let invalid = |col| error(row, col, "Linha ou célula fora do formato XLSX oficial.");
    let prefix = format!("<row r=\"{row}\"");
    let tail = rest.strip_prefix(&prefix).ok_or_else(|| invalid(1))?;
    *rest = if let Some(tail) = tail.strip_prefix(" spans=\"") {
        let (span, tail) = tail.split_once("\">").ok_or_else(|| invalid(1))?;
        let (first, last) = span.split_once(':').ok_or_else(|| invalid(1))?;
        let first = bounded_index(first, SHEET_COLUMNS).ok_or_else(|| invalid(1))?;
        let last = bounded_index(last, SHEET_COLUMNS).ok_or_else(|| invalid(1))?;
        if first > last {
            return Err(invalid(1));
        }
        tail
    } else {
        tail.strip_prefix('>').ok_or_else(|| invalid(1))?
    };
    let mut cells = vec![String::new(); crate::budget_csv::HEADER.len()];
    let mut record_type = String::new();
    for (index, cell) in cells.iter_mut().enumerate() {
        *rest = xml_space(rest);
        let letter = char::from(b'A' + index as u8);
        let address = format!("<c r=\"{letter}{row}\"");
        let Some(suffix) = rest.strip_prefix(&address) else {
            continue;
        };
        if let Some(next) = suffix.strip_prefix(" t=\"inlineStr\">") {
            let (value, next) = inline_cell(next, row, index + 1)?;
            *cell = value;
            *rest = next;
        } else if let Some(next) = suffix.strip_prefix(" t=\"s\">") {
            let (raw, next) = cell_value(next).ok_or_else(|| invalid(index + 1))?;
            if raw.is_empty() || raw.len() > 8 || !raw.bytes().all(|b| b.is_ascii_digit()) {
                return Err(invalid(index + 1));
            }
            let key = raw.parse::<usize>().map_err(|_| invalid(index + 1))?;
            let value = shared.get(key).ok_or_else(|| invalid(index + 1))?;
            if !safe(value) {
                return Err(invalid(index + 1));
            }
            cell.clone_from(value);
            *rest = next;
        } else if let Some(next) = suffix
            .strip_prefix(" t=\"n\">")
            .or_else(|| suffix.strip_prefix('>'))
        {
            if let Some(tail) = xml_space(next).strip_prefix("</c>") {
                if !suffix.starts_with('>') {
                    return Err(invalid(index + 1));
                }
                *rest = tail;
            } else {
                let (raw, next) = cell_value(next).ok_or_else(|| invalid(index + 1))?;
                *cell = native_number(raw, &record_type, index, locale)
                    .ok_or_else(|| invalid(index + 1))?;
                *rest = next;
            }
        } else if let Some(next) = suffix.strip_prefix("/>") {
            *rest = next;
        } else {
            return Err(invalid(index + 1));
        }
        if index == 0 {
            record_type.clone_from(cell);
        }
    }
    *rest = xml_space(rest);
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

/// Reimports the versioned package, stored or DEFLATE, with canonical XML or plain shared strings.
/// General Office XML variants are not supported by this strict reader.
/// Reuses the authoritative CSV semantic validation without persisting anything.
pub fn preview_import(bytes: &[u8]) -> Result<crate::budgets::BudgetRevision, CellError> {
    preview_import_with_locale(bytes).map(|(snapshot, _)| snapshot)
}

/// Also returns the decimal convention declared in the original workbook.
pub fn preview_import_with_locale(
    bytes: &[u8],
) -> Result<(crate::budgets::BudgetRevision, NumberLocale), CellError> {
    let (sheet, shared) = official_workbook(bytes)?;
    let mut rest = xml_body(&sheet)
        .strip_prefix(xml_body(SHEET_START))
        .ok_or_else(|| error(1, 1, "Cabeçalho XML não reconhecido."))?;
    rest = xml_space(rest);
    let declared_last_row =
        read_dimension(&mut rest).ok_or_else(|| error(1, 1, "Dimensão XLSX inválida."))?;
    rest = xml_space(rest)
        .strip_prefix("<sheetData>")
        .ok_or_else(|| error(1, 1, "Dados XML não reconhecidos."))?;
    rest = xml_space(rest);
    let version = read_row(&mut rest, 1, NumberLocale::Canonical, &shared)?;
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
    loop {
        rest = xml_space(rest);
        if rest
            .strip_prefix("</sheetData>")
            .is_some_and(|tail| xml_space(tail) == "</worksheet>")
        {
            break;
        }
        if row > MAX_SHEET_ROWS {
            return Err(error(row, 1, "Limite de registros XLSX excedido."));
        }
        let cells = read_row(&mut rest, row, locale, &shared)?;
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
    if declared_last_row.is_some_and(|last| last < row - 1) {
        return Err(error(row - 1, 1, "Dimensão XLSX menor que os dados."));
    }
    if shared.iter().any(|value| !safe(value)) {
        return Err(error(
            1,
            1,
            "Tabela de textos XLSX contém conteúdo executável.",
        ));
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

    // Rewrite only the last entry of an independently constructed ZIP fixture.
    // The central directory remains authoritative and its offset is updated.
    fn stream_last_entry(mut bytes: Vec<u8>, signed: bool) -> Vec<u8> {
        let end = bytes.len() - 22;
        let start = dword(&bytes, end + 16) as usize;
        let mut entry = start;
        let mut last = entry;
        while entry < end {
            last = entry;
            entry += 46 + usize::from(word(&bytes, entry + 28));
        }
        assert_eq!(entry, end);
        let local = dword(&bytes, last + 42) as usize;
        let flags = word(&bytes, last + 8) | 8;
        let values = bytes[last + 16..last + 28].to_vec();
        bytes[last + 8..last + 10].copy_from_slice(&flags.to_le_bytes());
        bytes[local + 6..local + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[local + 14..local + 26].fill(0);
        let mut descriptor = Vec::new();
        if signed {
            u32le(&mut descriptor, 0x0807_4b50);
        }
        descriptor.extend_from_slice(&values);
        let new_start = (start + descriptor.len()) as u32;
        bytes[end + 16..end + 20].copy_from_slice(&new_start.to_le_bytes());
        bytes.splice(start..start, descriptor);
        bytes
    }

    #[test]
    fn streaming_zip_requires_exact_descriptor_and_no_hidden_bytes() {
        let csv = format!(
            "{FORMAT_VERSION}\r\n{}\r\n",
            crate::budget_csv::HEADER.join(";")
        );
        let original = export_from_csv(&csv, NumberLocale::Canonical).unwrap();
        let expected = official_sheet(&original).unwrap();
        for base in [original.clone(), recompress_for_test(&original)] {
            for signed in [false, true] {
                let streamed = stream_last_entry(base.clone(), signed);
                assert_eq!(official_sheet(&streamed).unwrap(), expected);
                let end = streamed.len() - 22;
                let start = dword(&streamed, end + 16) as usize;
                for offset in [12, 8, 4] {
                    let mut bad = streamed.clone();
                    bad[start - offset] ^= 1;
                    assert!(official_sheet(&bad).is_err());
                }
                let mut hidden = streamed.clone();
                let shifted = (start + 1) as u32;
                hidden[end + 16..end + 20].copy_from_slice(&shifted.to_le_bytes());
                hidden.insert(start, 0);
                assert!(official_sheet(&hidden).is_err());
            }
        }
    }

    #[test]
    fn xml_declaration_variants_do_not_relax_relationship_validation() {
        let body = xml_body(ROOT_RELS);
        for declaration in [
            "",
            r#"<?xml version="1.0"?>"#,
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<?xml version="1.0" encoding="UTF-8" standalone="no"?>"#,
        ] {
            let variant = format!("\u{feff}{declaration}\r\n{body}\n");
            assert!(same_xml(variant.as_bytes(), ROOT_RELS));
        }
        let external = body.replace("xl/workbook.xml", "https://invalid.example/");
        assert!(!same_xml(external.as_bytes(), ROOT_RELS));
        let dtd = format!("<!DOCTYPE Relationships>{body}");
        assert!(!same_xml(dtd.as_bytes(), ROOT_RELS));
        assert!(!same_xml(&[0xff], ROOT_RELS));
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
        assert!(read_row(&mut wrong, 3, NumberLocale::Canonical, &[]).is_err());
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
        let cells = read_row(&mut row, 3, NumberLocale::Canonical, &[]).unwrap();
        assert_eq!(cells[0], "ITEM");
        assert_eq!(cells[1], "");
        assert!(row.is_empty());

        let mut row = r#"<row r="3"><c r="A3"/><c r="B3"></c><c r="C3" t="inlineStr"><is><t/></is></c><c r="D3" t="inlineStr"><is/></c><c r="E3" t="inlineStr"><is><t>ITEM</t></is></c></row>"#;
        let cells = read_row(&mut row, 3, NumberLocale::Canonical, &[]).unwrap();
        assert!(cells[..4].iter().all(String::is_empty));
        assert_eq!(cells[4], "ITEM");
        assert!(row.is_empty());

        let mut formula = r#"<row r="3"><c r="A3" t="inlineStr"><is><t>=1+1</t></is></c></row>"#;
        assert!(read_row(&mut formula, 3, NumberLocale::Canonical, &[]).is_err());
        let mut markup = r#"<row r="3"><c r="A3" t="inlineStr"><is><t><f>1</f></t></is></c></row>"#;
        assert!(read_row(&mut markup, 3, NumberLocale::Canonical, &[]).is_err());
    }

    #[test]
    fn native_numeric_cells_are_exact_and_schema_limited() {
        let original = r#"<row r="3"><c r="A3" t="inlineStr"><is><t>ITEM</t></is></c><c r="M3" t="inlineStr"><is><t>00001</t></is></c><c r="P3" t="n"><v>1.234567</v></c><c r="Q3"><v>123456789.12</v></c></row>"#;
        for (locale, quantity, price) in [
            (NumberLocale::Canonical, "1.234567", "123456789.12"),
            (NumberLocale::PtBr, "1,234567", "123456789,12"),
        ] {
            let mut row = original;
            let cells = read_row(&mut row, 3, locale, &[]).unwrap();
            assert_eq!(cells[12], "00001");
            assert_eq!(cells[15], quantity);
            assert_eq!(cells[16], price);
            assert!(row.is_empty());
        }
        for raw in [
            "-1",
            "+1",
            "1,2",
            "1.",
            ".5",
            "1.2.3",
            "<f>1</f>",
            "1e33",
            "1e-33",
            "1e",
            "1e2e3",
            "1e2+1",
            "1e2147483647",
        ] {
            assert!(native_number(raw, "ITEM", 15, NumberLocale::Canonical).is_none());
        }
        for (raw, exact) in [
            ("1e3", "1000"),
            ("1.23456E+3", "1234.56"),
            ("7.89E1", "78.9"),
            ("1E-6", "0.000001"),
            ("123.45e-2", "1.2345"),
        ] {
            assert_eq!(
                native_number(raw, "ITEM", 15, NumberLocale::Canonical).as_deref(),
                Some(exact)
            );
            assert_eq!(
                native_number(raw, "ITEM", 15, NumberLocale::PtBr),
                Some(exact.replace('.', ","))
            );
        }
        assert!(native_number("1e3", "META", 2, NumberLocale::Canonical).is_none());
        assert!(native_number("00001", "ITEM", 12, NumberLocale::Canonical).is_none());
        assert!(native_number("1.5", "META", 2, NumberLocale::Canonical).is_none());
        assert_eq!(
            native_number("12", "PERIOD", 6, NumberLocale::PtBr).as_deref(),
            Some("12")
        );

        let mut formula = r#"<row r="3"><c r="A3" t="inlineStr"><is><t>ITEM</t></is></c><c r="P3" t="n"><f>1+1</f><v>2</v></c></row>"#;
        let error = read_row(&mut formula, 3, NumberLocale::Canonical, &[]).unwrap_err();
        assert_eq!((error.row, error.column), (3, 16));
    }

    #[test]
    fn native_numbers_roundtrip_ten_thousand_services_without_float_conversion() {
        use crate::budgets::{BudgetLine, BudgetRevision, BudgetState, BudgetVariation};
        use crate::units::MeasureUnit;

        let original = BudgetRevision {
            budget_id: "Teste XLSX".into(),
            revision: 3,
            status: BudgetState::Review,
            responsible_id: format!("{:032x}", 1_u8).parse().unwrap(),
            variations: vec![BudgetVariation {
                id: "v1".into(),
                work_id: format!("{:032x}", 2_u8).parse().unwrap(),
                scope: "Obra fictícia".into(),
            }],
            lines: (0..10_000)
                .map(|i| BudgetLine {
                    id: format!("item-{i}"),
                    variation_id: "v1".into(),
                    group: "Pavimento".into(),
                    stage: "Revestimento".into(),
                    substage: "Preparação".into(),
                    code: format!("{i:05}"),
                    description: "Descrição técnica".into(),
                    unit: MeasureUnit::SquareMetre,
                    quantity: "1234.56".parse().unwrap(),
                    price: "78.90".parse().unwrap(),
                    catalog_id: Some("ref-2021".into()),
                    catalog_edition: Some(3),
                })
                .collect(),
            schedule: None,
        };
        let csv = crate::budget_csv::export_csv(&original, NumberLocale::PtBr).unwrap();
        let package = export_from_csv(&csv, NumberLocale::PtBr).unwrap();
        let sheet = official_sheet(&package).unwrap();
        let sheet = sheet
            .replace(
                r#" t="inlineStr"><is><t xml:space="preserve">1234,56</t></is></c>"#,
                r#" t="n"> <v>1.23456E+3</v> </c>"#,
            )
            .replace(
                r#" t="inlineStr"><is><t xml:space="preserve">78,900000</t></is></c>"#,
                r#"> <v>7.89E1</v> </c>"#,
            )
            .replace("<is><t", "<is> <t")
            .replace("</t></is></c>", "</t> </is> </c>");
        assert_eq!(sheet.matches("<v>").count(), 20_000);
        let bytes = stored_zip(&[
            (PART_NAMES[0], CONTENT_TYPES.as_bytes()),
            (PART_NAMES[1], ROOT_RELS.as_bytes()),
            (PART_NAMES[2], WORKBOOK.as_bytes()),
            (PART_NAMES[3], WORKBOOK_RELS.as_bytes()),
            (PART_NAMES[4], sheet.as_bytes()),
        ]);
        let (imported, locale) = preview_import_with_locale(&bytes).unwrap();
        assert_eq!(locale, NumberLocale::PtBr);
        assert_eq!(
            crate::budget_csv::export_csv(&imported, locale).unwrap(),
            csv
        );
        assert_eq!(imported.lines[0].code, "00000");

        let streamed = stream_last_entry(recompress_for_test(&bytes), true);
        let (imported, locale) = preview_import_with_locale(&streamed).unwrap();
        assert_eq!(locale, NumberLocale::PtBr);
        assert_eq!(
            crate::budget_csv::export_csv(&imported, locale).unwrap(),
            csv
        );
    }

    #[test]
    fn shared_strings_reject_invalid_indices_markup_and_metadata() {
        let xml = format!(
            "{SHARED_START} count=\"3\" uniqueCount=\"3\"><si><t>ITEM</t></si><si><t>00001</t></si><si><t/></si></sst>"
        );
        let values = read_shared_strings(&xml).unwrap();
        assert_eq!(values, ["ITEM", "00001", ""]);
        let pretty = format!(
            "{SHARED_START} uniqueCount=\"2\">\n  <si>\n    <t xml:space=\"preserve\">  A  </t>\n  </si>\n  <si>\n    <t/>\n  </si>\n</sst>"
        );
        assert_eq!(read_shared_strings(&pretty).unwrap(), ["  A  ", ""]);
        let injected = pretty.replace("</t>", "</t><f>1+1</f>");
        assert!(read_shared_strings(&injected).is_err());
        let mut row =
            r#"<row r="3"><c r="A3" t="s"><v>0</v></c><c r="M3" t="s"><v>1</v></c></row>"#;
        let cells = read_row(&mut row, 3, NumberLocale::Canonical, &values).unwrap();
        assert_eq!(cells[0], "ITEM");
        assert_eq!(cells[12], "00001");
        for raw in ["3", "-1", "1.0", "", "<f>0</f>"] {
            let row = format!(r#"<row r="3"><c r="M3" t="s"><v>{raw}</v></c></row>"#);
            let err = read_row(&mut row.as_str(), 3, NumberLocale::Canonical, &values).unwrap_err();
            assert_eq!((err.row, err.column), (3, 13));
        }
        for body in [
            r#" uniqueCount="2"><si><t>abc</t></si></sst>"#,
            r#" uniqueCount="250001"></sst>"#,
            r#"><si><t>&external;</t></si></sst>"#,
            r#"><si><r><t>abc</t></r></si></sst>"#,
        ] {
            assert!(read_shared_strings(&format!("{SHARED_START}{body}")).is_err());
        }
    }

    #[test]
    fn shared_strings_roundtrip_full_budget_and_reject_external_relationship() {
        use crate::budgets::{BudgetLine, BudgetRevision, BudgetState, BudgetVariation};
        use crate::units::MeasureUnit;

        let original = BudgetRevision {
            budget_id: "Teste XLSX".into(),
            revision: 3,
            status: BudgetState::Review,
            responsible_id: format!("{:032x}", 1_u8).parse().unwrap(),
            variations: vec![BudgetVariation {
                id: "v1".into(),
                work_id: format!("{:032x}", 2_u8).parse().unwrap(),
                scope: "Obra fictícia".into(),
            }],
            lines: (0..10_000)
                .map(|i| BudgetLine {
                    id: format!("item-{i}"),
                    variation_id: "v1".into(),
                    group: "Pavimento".into(),
                    stage: "Revestimento".into(),
                    substage: "Preparação".into(),
                    code: format!("{i:05}"),
                    description: "Descrição técnica".into(),
                    unit: MeasureUnit::SquareMetre,
                    quantity: "1234.56".parse().unwrap(),
                    price: "78.90".parse().unwrap(),
                    catalog_id: Some("ref-2021".into()),
                    catalog_edition: Some(3),
                })
                .collect(),
            schedule: None,
        };
        let csv = crate::budget_csv::export_csv(&original, NumberLocale::PtBr).unwrap();
        let package = export_from_csv(&csv, NumberLocale::PtBr).unwrap();
        let sheet = official_sheet(&package).unwrap().replace(
            r#" t="inlineStr"><is><t xml:space="preserve">Descrição técnica</t></is></c>"#,
            r#" t="s"><v>0</v></c>"#,
        );
        assert_eq!(sheet.matches(r#" t="s">"#).count(), 10_000);
        let shared = format!(
            "{SHARED_START} count=\"10000\" uniqueCount=\"1\"><si><t>Descrição técnica</t></si></sst>"
        );
        let types = shared_content_types();
        let rels = shared_workbook_rels();
        let mut parts = [
            (PART_NAMES[0], types.as_bytes()),
            (PART_NAMES[1], ROOT_RELS.as_bytes()),
            (PART_NAMES[2], WORKBOOK.as_bytes()),
            (PART_NAMES[3], rels.as_bytes()),
            (PART_NAMES[4], sheet.as_bytes()),
            (PART_NAMES[5], shared.as_bytes()),
        ];
        let bytes = stored_zip(&parts);
        for bytes in [bytes.clone(), recompress_for_test(&bytes)] {
            let (imported, locale) = preview_import_with_locale(&bytes).unwrap();
            assert_eq!(locale, NumberLocale::PtBr);
            assert_eq!(
                crate::budget_csv::export_csv(&imported, locale).unwrap(),
                csv
            );
        }
        let variants: Vec<_> = parts
            .iter()
            .map(|&(name, data)| {
                let body = xml_body(std::str::from_utf8(data).unwrap());
                (
                    name,
                    format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n{body}\n"),
                )
            })
            .collect();
        let entries: Vec<_> = variants
            .iter()
            .map(|(name, xml)| (*name, xml.as_bytes()))
            .collect();
        let (imported, locale) = preview_import_with_locale(&stored_zip(&entries)).unwrap();
        assert_eq!(locale, NumberLocale::PtBr);
        assert_eq!(
            crate::budget_csv::export_csv(&imported, locale).unwrap(),
            csv
        );

        let last_row = sheet.matches("<row ").count();
        let with_metadata = sheet
            .replace(
                "<sheetData>",
                &format!(r#"<dimension ref="A1:S{last_row}"/><sheetData>"#),
            )
            .replace(r#""><c r="#, r#"" spans="1:19"><c r="#);
        assert_eq!(with_metadata.matches(" spans=").count(), last_row);
        parts[4].1 = with_metadata.as_bytes();
        let (imported, locale) = preview_import_with_locale(&stored_zip(&parts)).unwrap();
        assert_eq!(
            crate::budget_csv::export_csv(&imported, locale).unwrap(),
            csv
        );
        let pretty_sheet = with_metadata
            .replace("><sheetData>", ">\n<sheetData>\n")
            .replace("</row><row", "</row>\n  <row")
            .replace("</c><c", "</c>\n    <c")
            .replace("</sheetData></worksheet>", "</sheetData>\n</worksheet>");
        let pretty_types = types.replace("><", ">\n  <");
        let pretty_shared = shared.replace("><", ">\n  <");
        parts[0].1 = pretty_types.as_bytes();
        parts[4].1 = pretty_sheet.as_bytes();
        parts[5].1 = pretty_shared.as_bytes();
        let (imported, locale) = preview_import_with_locale(&stored_zip(&parts)).unwrap();
        assert_eq!(
            crate::budget_csv::export_csv(&imported, locale).unwrap(),
            csv
        );
        parts[0].1 = types.as_bytes();

        let false_dimension = with_metadata.replace(&format!("A1:S{last_row}"), "A1:S2");
        parts[4].1 = false_dimension.as_bytes();
        assert!(preview_import(&stored_zip(&parts)).is_err());
        parts[4].1 = with_metadata.as_bytes();

        // A fórmula disfarçada em texto compartilhado deve apontar a célula
        // real do último serviço, e não A1 (posição da tabela de textos).
        let malicious_shared = shared
            .replace("uniqueCount=\"1\"", "uniqueCount=\"2\"")
            .replace("</sst>", "<si><t>_x003D_1+1</t></si></sst>");
        let malicious_sheet = sheet.replace(
            r#"<c r="N10004" t="s"><v>0</v></c>"#,
            r#"<c r="N10004" t="s"><v>1</v></c>"#,
        );
        assert_ne!(malicious_sheet, sheet);
        parts[4].1 = malicious_sheet.as_bytes();
        parts[5].1 = malicious_shared.as_bytes();
        let err = preview_import(&stored_zip(&parts)).unwrap_err();
        assert_eq!((err.row, err.column), (10_004, 14));
        // Mesmo uma entrada perigosa sem referência impede o aceite do lote.
        parts[4].1 = sheet.as_bytes();
        let err = preview_import(&stored_zip(&parts)).unwrap_err();
        assert_eq!((err.row, err.column), (1, 1));
        parts[5].1 = shared.as_bytes();

        let external = rels.replace(
            r#"Target="sharedStrings.xml""#,
            r#"Target="https://invalid.example/" TargetMode="External""#,
        );
        parts[3].1 = external.as_bytes();
        assert!(preview_import(&stored_zip(&parts)).is_err());
    }

    #[test]
    fn worksheet_metadata_is_bounded_and_never_hides_cells() {
        let mut rest = r#"<dimension ref="A1:S10004"/><sheetData>"#;
        assert_eq!(read_dimension(&mut rest), Some(Some(10_004)));
        assert_eq!(rest, "<sheetData>");
        assert_eq!(MAX_SHEET_ROWS, 110_504);
        let mut maximum = r#"<dimension ref="A1:S110504"/><sheetData>"#;
        assert_eq!(read_dimension(&mut maximum), Some(Some(MAX_SHEET_ROWS)));
        for malformed in [
            r#"<dimension ref="A1:S0"/>"#,
            r#"<dimension ref="A1:S110505"/>"#,
            r#"<dimension ref="A1:S9999999999999999"/>"#,
            r#"<dimension ref="A1:S3" evil="1"/>"#,
        ] {
            let mut malformed = malformed;
            assert_eq!(read_dimension(&mut malformed), None);
        }
        let mut row =
            r#"<row r="3" spans="1:19"><c r="A3" t="inlineStr"><is><t>ITEM</t></is></c></row>"#;
        let cells = read_row(&mut row, 3, NumberLocale::Canonical, &[]).unwrap();
        assert_eq!(cells[0], "ITEM");
        assert!(row.is_empty());
        for span in ["0:19", "19:1", "1:20", "1:19 20:30", "1:999999999"] {
            let row = format!(r#"<row r="3" spans="{span}"></row>"#);
            assert!(read_row(&mut row.as_str(), 3, NumberLocale::Canonical, &[]).is_err());
        }
        let mut formula = r#"<row r="3" spans="1:19"><c r="Q3"><f>1+1</f></c></row>"#;
        let err = read_row(&mut formula, 3, NumberLocale::Canonical, &[]).unwrap_err();
        assert_eq!((err.row, err.column), (3, 17));
    }

    #[test]
    fn structural_whitespace_never_changes_text_or_allows_extra_xml() {
        let mut row = r#"<row r="3"> <c r="A3" t="inlineStr">
            <is> <t> ITEM </t> </is> </c> </row>"#;
        let cells = read_row(&mut row, 3, NumberLocale::Canonical, &[]).unwrap();
        assert_eq!(cells[0], " ITEM ");
        assert!(row.is_empty());

        let mut row = r#"<row r="3"><c r="A3" t="s"> <v>0</v> </c>
            <c r="P3" t="n"> <v>1.25</v> </c>
            <c r="Q3"> <v>1234.56</v> </c></row>"#;
        let cells = read_row(&mut row, 3, NumberLocale::PtBr, &["ITEM".into()]).unwrap();
        assert_eq!(cells[15], "1,25");
        assert_eq!(cells[16], "1234,56");
        assert!(row.is_empty());

        for invalid in [
            r#"<c r="A3" t="inlineStr"> <is><t>abc</t><f>1</f></is></c>"#,
            r#"<c r="A3" t="inlineStr"> <is><t>abc</t></is><f>1</f></c>"#,
            r#"<c r="A3" t="s"> <v>0</v><f>1</f></c>"#,
            r#"<c r="A3" t="inlineStr"> <is><t>_x003D_1+1</t></is></c>"#,
        ] {
            let row = format!(r#"<row r="3">{invalid}</row>"#);
            let err = read_row(
                &mut row.as_str(),
                3,
                NumberLocale::Canonical,
                &["ITEM".into()],
            )
            .unwrap_err();
            assert_eq!((err.row, err.column), (3, 1));
        }

        let external = ROOT_RELS.replace("Target=", "TargetMode=\"External\" Target=");
        assert!(!same_xml(external.as_bytes(), ROOT_RELS));
        let extra = ROOT_RELS.replace("</Relationships>", "<extra/></Relationships>");
        assert!(!same_xml(extra.as_bytes(), ROOT_RELS));

        let mut formula = r#"<row r="3"> <c r="Q3"><f>1+1</f></c> </row>"#;
        let err = read_row(&mut formula, 3, NumberLocale::Canonical, &[]).unwrap_err();
        assert_eq!((err.row, err.column), (3, 17));
    }

    #[test]
    fn malformed_xlsx_cell_reports_its_actual_column() {
        let mut formula = r#"<row r="3"><c r="Q3"><f>1+1</f></c></row>"#;
        let error = read_row(&mut formula, 3, NumberLocale::Canonical, &[]).unwrap_err();
        assert_eq!((error.row, error.column), (3, 17));

        let mut extra = r#"<row r="4"><c r="AA4" t="n"><v>1</v></c></row>"#;
        let error = read_row(&mut extra, 4, NumberLocale::Canonical, &[]).unwrap_err();
        assert_eq!((error.row, error.column), (4, 27));

        let mut wrong_row = r#"<row r="5"><c r="N6" t="n"><v>1</v></c></row>"#;
        let error = read_row(&mut wrong_row, 5, NumberLocale::Canonical, &[]).unwrap_err();
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
    fn xlsx_export_preserves_physical_csv_cell_diagnostics() {
        let mut csv = format!("{FORMAT_VERSION}\r\n");
        csv.push_str(&crate::budget_csv::HEADER.join(";"));
        csv.push_str("\r\n");
        for _ in 0..9_999 {
            csv.push_str("ITEM;;;;;;;;;;;;00001;Descrição;m;1;2;;\r\n");
        }
        csv.push_str("ITEM;;;;;;;;;;;;00001;=1+1;m;1;2;;\r\n");
        let error = export_from_csv(&csv, NumberLocale::Canonical).unwrap_err();
        assert_eq!((error.row, error.column), (10_002, 14));
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
