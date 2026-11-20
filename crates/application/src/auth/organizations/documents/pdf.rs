//! O parser nativo confere catálogo, referências e páginas antes da quarentena.
use canteiro_domain::documents::DocumentError;
use lopdf::{Document, LoadOptions, Object};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

const MAX_DEPTH: usize = 32;
const MAX_TOKENS: usize = 200_000;
static PARSERS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// O prazo limita a espera HTTP. O permit continua na thread até ela terminar,
/// mesmo após timeout: não simula cancelamento de um parser bloqueante.
pub(super) async fn validate_bounded(bytes: &[u8]) -> Result<(), DocumentError> {
    let permit = PARSERS
        .try_acquire()
        .map_err(|_| DocumentError::InspectionUnavailable)?;
    let bytes = bytes.to_vec();
    tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            validate(&bytes)
        }),
    )
    .await
    .map_err(|_| DocumentError::InspectionUnavailable)?
    .map_err(|_| DocumentError::InspectionUnavailable)?
}

#[derive(Clone, Copy)]
enum Frame {
    Array,
    Dictionary {
        length: Option<usize>,
        trailer: bool,
    },
}

fn delimiter(byte: u8) -> bool {
    byte.is_ascii_whitespace() || b"()<>[]{}/%\0".contains(&byte)
}

fn whitespace(bytes: &[u8], index: &mut usize) {
    loop {
        while bytes
            .get(*index)
            .is_some_and(|b| b.is_ascii_whitespace() || *b == 0)
        {
            *index += 1;
        }
        if bytes.get(*index) != Some(&b'%') {
            return;
        }
        while bytes.get(*index).is_some_and(|b| !b"\r\n".contains(b)) {
            *index += 1;
        }
    }
}

fn pdf_name(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut value = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'#' {
            let digits = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            value.push(u8::from_str_radix(digits, 16).ok()?);
            index += 3;
        } else {
            value.push(bytes[index]);
            index += 1;
        }
    }
    Some(value)
}

fn unsigned(bytes: &[u8], index: &mut usize) -> Result<usize, DocumentError> {
    whitespace(bytes, index);
    let start = *index;
    while bytes.get(*index).is_some_and(u8::is_ascii_digit) {
        *index += 1;
    }
    if bytes.get(*index).is_some_and(|b| !delimiter(*b)) {
        return Err(DocumentError::Structure);
    }
    std::str::from_utf8(&bytes[start..*index])
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(DocumentError::Structure)
}

struct XrefEntry {
    offset: usize,
    object: u32,
    generation: u16,
}

fn classic_xref(
    bytes: &[u8],
    index: &mut usize,
    entries: &mut Vec<XrefEntry>,
    count: &mut usize,
) -> Result<(), DocumentError> {
    loop {
        whitespace(bytes, index);
        if bytes[*index..].starts_with(b"trailer")
            && bytes.get(*index + 7).is_none_or(|b| delimiter(*b))
        {
            return Ok(());
        }
        let first = unsigned(bytes, index)?;
        let length = unsigned(bytes, index)?;
        *count = count.checked_add(length).ok_or(DocumentError::Complexity)?;
        if *count > MAX_TOKENS {
            return Err(DocumentError::Complexity);
        }
        for relative in 0..length {
            let offset = unsigned(bytes, index)?;
            let generation = unsigned(bytes, index)?;
            whitespace(bytes, index);
            let state = *bytes.get(*index).ok_or(DocumentError::Structure)?;
            *index += 1;
            if !matches!(state, b'n' | b'f') || bytes.get(*index).is_some_and(|b| !delimiter(*b)) {
                return Err(DocumentError::Structure);
            }
            if state == b'n' {
                entries.push(XrefEntry {
                    offset,
                    object: first
                        .checked_add(relative)
                        .and_then(|v| v.try_into().ok())
                        .ok_or(DocumentError::Structure)?,
                    generation: generation
                        .try_into()
                        .map_err(|_| DocumentError::Structure)?,
                });
            }
        }
    }
}

/// Limites anteriores à recursão do parser. Strings e fluxos são ignorados pelo
/// tamanho declarado; fluxos com Length indireto são recusados conservadoramente.
/// Esta etapa só impõe limites. A validade documental continua com lopdf.
fn preflight(bytes: &[u8]) -> Result<(), DocumentError> {
    if bytes.len() > canteiro_domain::documents::MAX_DOCUMENT_BYTES {
        return Err(DocumentError::Size);
    }
    let mut stack = Vec::new();
    let mut index = 0;
    let mut tokens = 0;
    let mut objects = 0;
    let mut closed_dictionary = None;
    let mut object_starts = BTreeMap::new();
    let mut inside_object = false;
    let mut recent_words: Vec<(usize, &[u8])> = Vec::new();
    let mut trailer_next = false;
    let mut xref_sections = BTreeSet::new();
    let mut xref_entries = Vec::new();
    let mut xref_count = 0;
    let mut xref_targets = Vec::new();
    let mut external_starts = BTreeSet::new();
    while index < bytes.len() {
        whitespace(bytes, &mut index);
        if index == bytes.len() {
            break;
        }
        tokens += 1;
        if tokens > MAX_TOKENS {
            return Err(DocumentError::Complexity);
        }
        let previous_dictionary = closed_dictionary.take();
        let is_trailer = std::mem::take(&mut trailer_next);
        if delimiter(bytes[index]) {
            recent_words.clear();
        }
        match bytes[index] {
            b'[' => {
                stack.push(Frame::Array);
                index += 1;
            }
            b']' => {
                if !matches!(stack.pop(), Some(Frame::Array)) {
                    return Err(DocumentError::Structure);
                }
                index += 1;
            }
            b'<' if bytes.get(index + 1) == Some(&b'<') => {
                stack.push(Frame::Dictionary {
                    length: None,
                    trailer: is_trailer,
                });
                index += 2;
            }
            b'>' if bytes.get(index + 1) == Some(&b'>') => {
                let Some(Frame::Dictionary { length, .. }) = stack.pop() else {
                    return Err(DocumentError::Structure);
                };
                closed_dictionary = Some(length);
                index += 2;
            }
            b'(' => {
                index += 1;
                let mut depth = 1;
                while depth > 0 {
                    match bytes.get(index).copied().ok_or(DocumentError::Structure)? {
                        b'\\' => {
                            index += 1;
                            bytes.get(index).ok_or(DocumentError::Structure)?;
                        }
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    if depth + stack.len() > MAX_DEPTH {
                        return Err(DocumentError::Complexity);
                    }
                    index += 1;
                }
            }
            b'<' => {
                index += 1;
                while bytes.get(index) != Some(&b'>') {
                    let byte = bytes.get(index).ok_or(DocumentError::Structure)?;
                    if !byte.is_ascii_hexdigit() && !byte.is_ascii_whitespace() {
                        return Err(DocumentError::Structure);
                    }
                    index += 1;
                }
                index += 1;
            }
            b'/' => {
                index += 1;
                let start = index;
                while bytes.get(index).is_some_and(|b| !delimiter(*b)) {
                    index += 1;
                }
                let name = pdf_name(&bytes[start..index]).ok_or(DocumentError::Structure)?;
                if matches!(
                    name.as_slice(),
                    b"ObjStm" | b"XRef" | b"XRefStm" | b"Encrypt"
                ) {
                    return Err(DocumentError::RestrictedPdf);
                }
                if name == b"Length" {
                    whitespace(bytes, &mut index);
                    let start = index;
                    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                        index += 1;
                    }
                    let length = std::str::from_utf8(&bytes[start..index])
                        .ok()
                        .and_then(|value| value.parse::<usize>().ok())
                        .ok_or(DocumentError::RestrictedPdf)?;
                    whitespace(bytes, &mut index);
                    if !matches!(bytes.get(index), Some(b'/' | b'>')) {
                        return Err(DocumentError::RestrictedPdf);
                    }
                    if let Some(Frame::Dictionary { length: target, .. }) = stack.last_mut() {
                        *target = Some(length);
                    }
                } else if name == b"Prev"
                    && matches!(stack.last(), Some(Frame::Dictionary { trailer: true, .. }))
                {
                    xref_targets.push(unsigned(bytes, &mut index)?);
                }
            }
            b')' | b'>' | b'{' | b'}' => return Err(DocumentError::Structure),
            _ => {
                let start = index;
                while bytes.get(index).is_some_and(|b| !delimiter(*b)) {
                    index += 1;
                }
                if index == start {
                    return Err(DocumentError::Structure);
                }
                match &bytes[start..index] {
                    b"obj" => {
                        if inside_object || !stack.is_empty() || recent_words.len() != 2 {
                            return Err(DocumentError::Structure);
                        }
                        let object: u32 = std::str::from_utf8(recent_words[0].1)
                            .ok()
                            .and_then(|s| s.parse().ok())
                            .ok_or(DocumentError::Structure)?;
                        let generation: u16 = std::str::from_utf8(recent_words[1].1)
                            .ok()
                            .and_then(|s| s.parse().ok())
                            .ok_or(DocumentError::Structure)?;
                        if object == 0 {
                            return Err(DocumentError::Structure);
                        }
                        object_starts.insert(recent_words[0].0, (object, generation));
                        inside_object = true;
                        recent_words.clear();
                        objects += 1;
                        if objects > 100_000 {
                            return Err(DocumentError::Complexity);
                        }
                    }
                    b"endobj" => {
                        if !inside_object || !stack.is_empty() {
                            return Err(DocumentError::Structure);
                        }
                        inside_object = false;
                        recent_words.clear();
                    }
                    b"xref" => {
                        if inside_object || !stack.is_empty() {
                            return Err(DocumentError::Structure);
                        }
                        xref_sections.insert(start);
                        classic_xref(bytes, &mut index, &mut xref_entries, &mut xref_count)?;
                        recent_words.clear();
                    }
                    b"trailer" => {
                        if inside_object || !stack.is_empty() {
                            return Err(DocumentError::Structure);
                        }
                        trailer_next = true;
                        recent_words.clear();
                    }
                    b"startxref" => {
                        if inside_object || !stack.is_empty() {
                            return Err(DocumentError::Structure);
                        }
                        external_starts.insert(start);
                        xref_targets.push(unsigned(bytes, &mut index)?);
                        recent_words.clear();
                    }
                    b"stream" => {
                        let length = previous_dictionary
                            .flatten()
                            .ok_or(DocumentError::RestrictedPdf)?;
                        while matches!(bytes.get(index), Some(b' ' | b'\t')) {
                            index += 1;
                        }
                        if bytes.get(index) == Some(&b'\r') {
                            index += 1;
                            if bytes.get(index) == Some(&b'\n') {
                                index += 1;
                            }
                        } else if bytes.get(index) == Some(&b'\n') {
                            index += 1;
                        } else {
                            return Err(DocumentError::Structure);
                        }
                        index = index
                            .checked_add(length)
                            .filter(|end| *end <= bytes.len())
                            .ok_or(DocumentError::Structure)?;
                        while bytes.get(index).is_some_and(|b| b.is_ascii_whitespace()) {
                            index += 1;
                        }
                        if !bytes[index..].starts_with(b"endstream")
                            || bytes.get(index + 9).is_some_and(|b| !delimiter(*b))
                        {
                            return Err(DocumentError::Structure);
                        }
                        index += 9;
                        recent_words.clear();
                    }
                    _ => {
                        if recent_words.len() == 2 {
                            recent_words.remove(0);
                        }
                        recent_words.push((start, &bytes[start..index]));
                    }
                }
            }
        }
        if stack.len() > MAX_DEPTH {
            return Err(DocumentError::Complexity);
        }
    }
    if !stack.is_empty() || inside_object {
        return Err(DocumentError::Structure);
    }
    // lopdf procura o último marcador bruto de EOF/startxref. Exigir que esse
    // mesmo marcador tenha sido reconhecido fora de fluxos e strings impede
    // um rodapé oculto de selecionar outra tabela após esta conferência.
    let end = bytes
        .iter()
        .rposition(|b| !b" \t\n\r\0\x0c".contains(b))
        .map_or(0, |p| p + 1);
    if end < 5 || &bytes[end - 5..end] != b"%%EOF" {
        return Err(DocumentError::Structure);
    }
    let eof = end - 5;
    let marker = bytes[..eof]
        .windows(9)
        .rposition(|v| v == b"startxref")
        .ok_or(DocumentError::Structure)?;
    if eof <= 25 || marker < eof - 25 || !external_starts.contains(&marker) {
        return Err(DocumentError::Structure);
    }
    let pointer = std::str::from_utf8(&bytes[marker + 9..eof])
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .ok_or(DocumentError::Structure)?;
    if !xref_sections.contains(&pointer) {
        return Err(DocumentError::Structure);
    }
    // Referências não podem fazer o parser entrar em bytes ignorados pelo lexer,
    // como objetos ocultos dentro de strings, comentários ou fluxos sobrepostos.
    if xref_targets
        .iter()
        .any(|offset| !xref_sections.contains(offset))
        || xref_entries.iter().any(|entry| {
            object_starts.get(&entry.offset) != Some(&(entry.object, entry.generation))
        })
    {
        return Err(DocumentError::Structure);
    }
    Ok(())
}

pub(super) fn validate(bytes: &[u8]) -> Result<(), DocumentError> {
    preflight(bytes)?;
    let document = Document::load_mem_with_options(
        bytes,
        LoadOptions {
            strict: true,
            max_decompressed_size: Some(10 * 1024 * 1024),
            ..Default::default()
        },
    )
    .map_err(|_| DocumentError::Structure)?;
    if document.is_encrypted() {
        return Err(DocumentError::RestrictedPdf);
    }
    if document.objects.is_empty() || document.objects.len() > 100_000 {
        return Err(DocumentError::Structure);
    }
    let catalog = document.catalog().map_err(|_| DocumentError::Structure)?;
    let root = catalog
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(|_| DocumentError::Structure)?;
    let mut seen = BTreeSet::new();
    let mut queue = vec![(root, 0usize)];
    let mut pages = 0usize;
    while let Some((id, depth)) = queue.pop() {
        if depth > 32 || !seen.insert(id) {
            return Err(DocumentError::Structure);
        }
        let dictionary = document
            .get_object(id)
            .and_then(Object::as_dict)
            .map_err(|_| DocumentError::Structure)?;
        match dictionary
            .get(b"Type")
            .and_then(Object::as_name)
            .map_err(|_| DocumentError::Structure)?
        {
            b"Pages" => {
                let children = dictionary
                    .get(b"Kids")
                    .and_then(Object::as_array)
                    .map_err(|_| DocumentError::Structure)?;
                if children.is_empty() || children.len() > 10_000 {
                    return Err(DocumentError::Structure);
                }
                for child in children {
                    queue.push((
                        child.as_reference().map_err(|_| DocumentError::Structure)?,
                        depth + 1,
                    ));
                }
            }
            b"Page" => {
                pages += 1;
                if pages > 10_000 {
                    return Err(DocumentError::Structure);
                }
            }
            _ => return Err(DocumentError::Structure),
        }
    }
    if pages == 0 {
        return Err(DocumentError::Structure);
    }
    let mut count = 0usize;
    for object in document.objects.values() {
        inspect(&document, object, 0, &mut count)?;
    }
    Ok(())
}

fn forbidden(name: &[u8]) -> bool {
    [
        b"JavaScript".as_slice(),
        b"JS",
        b"OpenAction",
        b"AA",
        b"Launch",
        b"EmbeddedFile",
        b"EmbeddedFiles",
        b"RichMedia",
        b"XFA",
        b"SubmitForm",
        b"ImportData",
    ]
    .iter()
    .any(|value| name.eq_ignore_ascii_case(value))
}

fn inspect(
    document: &Document,
    object: &Object,
    depth: usize,
    count: &mut usize,
) -> Result<(), DocumentError> {
    *count += 1;
    if *count > 200_000 || depth > 32 {
        return Err(DocumentError::Structure);
    }
    match object {
        Object::Name(name) if forbidden(name) => return Err(DocumentError::Prohibited),
        Object::Reference(id) if !document.objects.contains_key(id) => {
            return Err(DocumentError::Structure);
        }
        Object::Array(items) => {
            for item in items {
                inspect(document, item, depth + 1, count)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (key, value) in dictionary.iter() {
                if forbidden(key) {
                    return Err(DocumentError::Prohibited);
                }
                inspect(document, value, depth + 1, count)?;
            }
        }
        Object::Stream(stream) => {
            for (key, value) in stream.dict.iter() {
                if forbidden(key) {
                    return Err(DocumentError::Prohibited);
                }
                inspect(document, value, depth + 1, count)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document(active: bool, signature: bool) -> Vec<u8> {
        use lopdf::dictionary;
        let mut document = Document::with_version("1.7");
        document.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        let pages = document.new_object_id();
        let page=document.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
        document.objects.insert(
            pages,
            Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
        );
        let mut catalog = dictionary! {"Type"=>"Catalog","Pages"=>pages};
        if active {
            catalog.set(
                "OpenAction",
                dictionary! {"S"=>"JavaScript","JS"=>Object::string_literal("alert(1)")},
            );
        }
        if signature {
            catalog.set("AcroForm",dictionary!{"Fields"=>vec![Object::Dictionary(dictionary!{"FT"=>"Sig","T"=>Object::string_literal("Assinatura fictícia"),"V"=>Object::Dictionary(dictionary!{"Type"=>"Sig","Filter"=>"Adobe.PPKLite","SubFilter"=>"adbe.pkcs7.detached","ByteRange"=>vec![0.into(),10.into(),20.into(),30.into()]})})]});
        }
        let root = document.add_object(catalog);
        document.trailer.set("Root", root);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }
    #[test]
    fn parser_rejects_forged_pdf_and_accepts_passive_signature_container() {
        assert_eq!(
            validate(b"%PDF-1.7 /Catalog startxref 0 %%EOF"),
            Err(DocumentError::Structure)
        );
        assert_eq!(validate(&document(false, false)), Ok(()));
        assert_eq!(validate(&document(false, true)), Ok(()));
        assert_eq!(
            validate(&document(true, false)),
            Err(DocumentError::Prohibited)
        );
    }

    fn raw_document(extra: &str, stream: Option<&[u8]>) -> Vec<u8> {
        let mut objects = vec![
            format!("<< /Type /Catalog /Pages 2 0 R {extra} >>").into_bytes(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            if stream.is_some() {
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Contents 4 0 R >>".to_vec()
            } else {
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>".to_vec()
            },
        ];
        if let Some(stream) = stream {
            let mut object = format!("<< /Length {} >>\nstream\n", stream.len()).into_bytes();
            object.extend_from_slice(stream);
            object.extend_from_slice(b"\nendstream");
            objects.push(object);
        }
        let mut bytes = b"%PDF-1.7\n".to_vec();
        let mut offsets = vec![0];
        for (index, object) in objects.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend_from_slice(format!("{} 0 obj\n", index + 1).as_bytes());
            bytes.extend_from_slice(object);
            bytes.extend_from_slice(b"\nendobj\n");
        }
        let xref = bytes.len();
        bytes.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes(),
        );
        for offset in offsets.iter().skip(1) {
            bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        bytes
    }

    #[test]
    fn parser_limits_nested_objects_and_tokens_before_recursion() {
        let nested = |depth| format!("/Extra {}0{}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(validate(&raw_document(&nested(30), None)), Ok(()));
        assert_eq!(
            validate(&raw_document(&nested(33), None)),
            Err(DocumentError::Complexity)
        );
        assert_eq!(
            validate(&raw_document(&nested(10_380), None)),
            Err(DocumentError::Complexity)
        );
        let large_array = format!("/Extra [{}]", "0 ".repeat(MAX_TOKENS));
        assert_eq!(
            validate(&raw_document(&large_array, None)),
            Err(DocumentError::Complexity)
        );
    }

    #[test]
    fn stream_bytes_do_not_bypass_limits_or_consume_nesting_budget() {
        let stream = format!("%{}\nq 1 0 0 1 0 0 cm Q", "[".repeat(10_380));
        assert_eq!(validate(&raw_document("", Some(stream.as_bytes()))), Ok(()));
        assert_eq!(
            preflight(b"<< /Length 1 0 R >>\nstream\nx\nendstream"),
            Err(DocumentError::RestrictedPdf)
        );
        assert_eq!(
            preflight(b"<< /Len#67th 1 0 R >>\nstream\nx\nendstream"),
            Err(DocumentError::RestrictedPdf)
        );
        assert_eq!(
            preflight(b"<< /Length 1 >>\nstream\nxx\nendstream"),
            Err(DocumentError::Structure)
        );
        assert_eq!(
            preflight(b"<< /Length 1 >>\nstream\nx\nendstream\n[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[[["),
            Err(DocumentError::Complexity)
        );
    }

    #[test]
    fn xref_cannot_load_objects_hidden_inside_stream_bytes() {
        // 33 níveis bastam para testar a fronteira, sem provocar estouro de pilha.
        let hidden = format!(
            "5 0 obj\n<< /Nested {}0{} >>\nendobj",
            "[".repeat(33),
            "]".repeat(33)
        );
        let mut bytes = raw_document("/Extra 5 0 R", Some(hidden.as_bytes()));
        let offset = bytes.windows(7).position(|v| v == b"5 0 obj").unwrap();
        let header = bytes.windows(9).position(|v| v == b"xref\n0 5\n").unwrap();
        bytes[header + 7] = b'6';
        let size = bytes.windows(7).position(|v| v == b"/Size 5").unwrap();
        bytes[size + 6] = b'6';
        let trailer = bytes.windows(8).position(|v| v == b"trailer\n").unwrap();
        bytes.splice(trailer..trailer, format!("{offset:010} 00000 n \n").bytes());
        assert_eq!(preflight(&bytes), Err(DocumentError::Structure));
        assert_eq!(validate(&bytes), Err(DocumentError::Structure));
    }

    #[test]
    fn incremental_xref_requires_real_sections_and_matching_object_numbers() {
        let mut bytes = raw_document("", None);
        let old_xref = bytes.windows(7).position(|v| v == b"xref\n0 ").unwrap();
        let object = bytes.len();
        bytes.extend_from_slice(b"4 0 obj\n(null)\nendobj\n");
        let new_xref = bytes.len();
        bytes.extend_from_slice(format!("xref\n4 1\n{object:010} 00000 n \ntrailer\n<< /Size 5 /Root 1 0 R /Prev {old_xref} >>\nstartxref\n{new_xref}\n%%EOF\n").as_bytes());
        assert_eq!(validate(&bytes), Ok(()));
        let mut mismatch = bytes.clone();
        mismatch[new_xref + 5] = b'3';
        assert_eq!(preflight(&mismatch), Err(DocumentError::Structure));
        let forged_previous = String::from_utf8(bytes)
            .unwrap()
            .replace(&format!("/Prev {old_xref}"), "/Prev 1");
        assert_eq!(
            preflight(forged_previous.as_bytes()),
            Err(DocumentError::Structure)
        );
    }

    #[test]
    fn hidden_terminal_xref_cannot_override_checked_outer_footer() {
        let mut bytes = raw_document("", None);
        let old_xref = bytes.windows(7).position(|v| v == b"xref\n0 ").unwrap();
        let stream_header = b"4 0 obj\n<< /Length 200 >>\nstream\n";
        let hidden_xref = bytes.len() + stream_header.len();
        let mut payload = format!("xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size 4 /Root 1 0 R /Prev {old_xref} >>\nstartxref\n{hidden_xref}\n%%EOF\n").into_bytes();
        assert!(payload.len() < 200);
        payload.resize(200, b' ');
        bytes.extend_from_slice(stream_header);
        bytes.extend_from_slice(&payload);
        bytes.extend_from_slice(b"\nendstream\nendobj\n");
        assert_eq!(preflight(&bytes), Err(DocumentError::Structure));
        assert_eq!(validate(&bytes), Err(DocumentError::Structure));
        assert_eq!(
            canteiro_domain::documents::inspect_content("rodape.pdf", &bytes),
            Err(DocumentError::Structure)
        );
    }

    #[tokio::test]
    async fn bounded_parser_accepts_passive_signature_and_preserves_bytes() {
        let bytes = document(false, true);
        let original = bytes.clone();
        assert!(canteiro_domain::documents::inspect_content("assinatura.pdf", &bytes).is_ok());
        assert_eq!(validate_bounded(&bytes).await, Ok(()));
        assert_eq!(bytes, original);
    }
}
