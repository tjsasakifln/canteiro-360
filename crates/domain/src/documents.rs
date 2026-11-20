//! Conteúdo documental permitido. A inspeção local nunca substitui o antivírus.
use serde::{Deserialize, Serialize};

pub const MAX_DOCUMENT_BYTES: usize = 5 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DocumentFormat {
    Pdf,
    Png,
    Text,
    Csv,
}

impl DocumentFormat {
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Png => "image/png",
            Self::Text => "text/plain; charset=utf-8",
            Self::Csv => "text/csv; charset=utf-8",
        }
    }
}

#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum DocumentError {
    #[error("Selecione um arquivo não vazio com até 5 MiB.")]
    Size,
    #[error("Use um nome simples, sem pastas, com até 180 caracteres.")]
    Name,
    #[error(
        "Use PDF sem conteúdo ativo, PNG, TXT ou CSV. O conteúdo deve corresponder à extensão."
    )]
    Format,
    #[error(
        "O arquivo contém conteúdo ativo ou não permitido. Obtenha uma cópia segura com o emissor e preserve o original assinado."
    )]
    Prohibited,
    #[error(
        "Este PDF usa recursos ainda não aceitos, como criptografia, objetos comprimidos ou tamanho de fluxo indireto. Preserve o original, especialmente se estiver assinado."
    )]
    RestrictedPdf,
    #[error(
        "Este PDF excede os limites de complexidade aceitos. Preserve o original, especialmente se estiver assinado."
    )]
    Complexity,
    #[error(
        "A análise do PDF não ficou disponível nesta tentativa. Preserve o arquivo e tente novamente."
    )]
    InspectionUnavailable,
    #[error("O arquivo está incompleto ou sua estrutura não é válida. Gere uma nova cópia.")]
    Structure,
}

pub fn validate_file_name(name: &str) -> Result<&str, DocumentError> {
    let trimmed = name.trim();
    if trimmed != name
        || trimmed.is_empty()
        || trimmed.chars().count() > 180
        || trimmed
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '\0'))
        || trimmed.starts_with('.')
    {
        return Err(DocumentError::Name);
    }
    Ok(trimmed)
}

/// O tipo declarado pelo navegador não participa da decisão.
pub fn inspect_content(name: &str, bytes: &[u8]) -> Result<DocumentFormat, DocumentError> {
    validate_file_name(name)?;
    if bytes.is_empty() || bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::Size);
    }
    const EICAR: &[u8] = b"EICAR-STANDARD-ANTIVIRUS-TEST-FILE";
    if bytes.windows(EICAR.len()).any(|v| v == EICAR) {
        return Err(DocumentError::Prohibited);
    }
    let extension = name.rsplit_once('.').map(|(_, s)| s.to_ascii_lowercase());
    match extension.as_deref() {
        Some("pdf") => inspect_pdf(bytes).map(|()| DocumentFormat::Pdf),
        Some("png") => inspect_png(bytes).map(|()| DocumentFormat::Png),
        Some("txt") | Some("csv") => {
            let text = std::str::from_utf8(bytes).map_err(|_| DocumentError::Format)?;
            if text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
            {
                return Err(DocumentError::Prohibited);
            }
            if text.starts_with("%PDF-") || bytes.starts_with(b"\x89PNG") {
                return Err(DocumentError::Format);
            }
            if extension.as_deref() == Some("csv") {
                // Excel pode executar fórmulas mesmo quando a célula está entre aspas.
                for cell in text.split([';', ',', '\n', '\r']) {
                    let value = cell.trim_start_matches(|c: char| {
                        c.is_whitespace() || c == '"' || c == '\u{feff}'
                    });
                    if value.starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠']) {
                        return Err(DocumentError::Prohibited);
                    }
                }
                Ok(DocumentFormat::Csv)
            } else {
                Ok(DocumentFormat::Text)
            }
        }
        _ => Err(DocumentError::Format),
    }
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn inspect_pdf(bytes: &[u8]) -> Result<(), DocumentError> {
    if !(bytes.starts_with(b"%PDF-1.") || bytes.starts_with(b"%PDF-2.0")) {
        return Err(DocumentError::Format);
    }
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    if !bytes[..end].ends_with(b"%%EOF")
        || !bytes.windows(9).any(|v| v == b"startxref")
        || !bytes.windows(8).any(|v| v == b"/Catalog")
    {
        return Err(DocumentError::Structure);
    }
    // Nomes PDF admitem escapes #hh. Objetos comprimidos e criptografia são
    // recusados, pois impediriam esta inspeção conservadora de nomes ativos.
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#'
            && i + 2 < bytes.len()
            && let (Some(a), Some(b)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            decoded.push((a * 16 + b).to_ascii_lowercase());
            i += 3;
        } else {
            decoded.push(bytes[i].to_ascii_lowercase());
            i += 1;
        }
    }
    if [
        b"/encrypt".as_slice(),
        b"/objstm".as_slice(),
        b"/xref".as_slice(),
    ]
    .iter()
    .any(|word| decoded.windows(word.len()).any(|v| v == *word))
    {
        return Err(DocumentError::RestrictedPdf);
    }
    const FORBIDDEN: [&[u8]; 10] = [
        b"/javascript",
        b"/js",
        b"/openaction",
        b"/aa",
        b"/launch",
        b"/embeddedfile",
        b"/richmedia",
        b"/xfa",
        b"/submitform",
        b"/importdata",
    ];
    if FORBIDDEN
        .iter()
        .any(|word| decoded.windows(word.len()).any(|v| v == *word))
    {
        return Err(DocumentError::Prohibited);
    }
    let marker = bytes[..end]
        .windows(9)
        .rposition(|v| v == b"startxref")
        .ok_or(DocumentError::Structure)?;
    let tail =
        std::str::from_utf8(&bytes[marker + 9..end - 5]).map_err(|_| DocumentError::Structure)?;
    let pointer = tail
        .trim()
        .parse::<usize>()
        .map_err(|_| DocumentError::Structure)?;
    if pointer < 9 || pointer >= marker || !bytes[pointer..].starts_with(b"xref") {
        return Err(DocumentError::Structure);
    }
    Ok(())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn inspect_png(bytes: &[u8]) -> Result<(), DocumentError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(DocumentError::Format);
    }
    let mut offset = 8usize;
    let mut header = false;
    let mut data = false;
    while offset < bytes.len() {
        if bytes.len() - offset < 12 {
            return Err(DocumentError::Structure);
        }
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .ok_or(DocumentError::Structure)?;
        if end > bytes.len() {
            return Err(DocumentError::Structure);
        }
        let kind = &bytes[offset + 4..offset + 8];
        let payload = &bytes[offset + 8..end - 4];
        let crc = u32::from_be_bytes(bytes[end - 4..end].try_into().unwrap());
        if crc32(&bytes[offset + 4..end - 4]) != crc {
            return Err(DocumentError::Structure);
        }
        if !header && kind != b"IHDR" {
            return Err(DocumentError::Structure);
        }
        match kind {
            b"IHDR" => {
                if header || length != 13 {
                    return Err(DocumentError::Structure);
                }
                let width = u32::from_be_bytes(payload[..4].try_into().unwrap());
                let height = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                if width == 0
                    || height == 0
                    || width > 16_000
                    || height > 16_000
                    || u64::from(width) * u64::from(height) > 64_000_000
                    || !matches!(
                        (payload[9], payload[8]),
                        (0, 1 | 2 | 4 | 8 | 16)
                            | (2, 8 | 16)
                            | (3, 1 | 2 | 4 | 8)
                            | (4 | 6, 8 | 16)
                    )
                    || payload[10] != 0
                    || payload[11] != 0
                    || payload[12] > 1
                {
                    return Err(DocumentError::Structure);
                }
                header = true;
            }
            b"IDAT" => {
                data |= length > 0;
            }
            b"IEND" => {
                return if data && length == 0 && end == bytes.len() {
                    Ok(())
                } else {
                    Err(DocumentError::Structure)
                };
            }
            b"PLTE" | b"tRNS" | b"gAMA" | b"cHRM" | b"sRGB" | b"pHYs" => {}
            _ => return Err(DocumentError::Prohibited),
        }
        offset = end;
    }
    Err(DocumentError::Structure)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn false_extensions_truncation_and_active_pdf_are_rejected() {
        assert_eq!(
            inspect_content("relatorio.pdf", b"<html>teste</html>"),
            Err(DocumentError::Format)
        );
        assert_eq!(
            inspect_content("foto.png", b"\x89PNG\r\n\x1a\n"),
            Err(DocumentError::Structure)
        );
        assert_eq!(
            inspect_content(
                "relatorio.pdf",
                b"%PDF-1.7 /Catalog /J#61vaScript startxref 0 %%EOF"
            ),
            Err(DocumentError::Prohibited)
        );
        assert_eq!(
            inspect_content(
                "relatorio.pdf",
                b"%PDF-1.7 /Catalog /ObjStm startxref 0 %%EOF"
            ),
            Err(DocumentError::RestrictedPdf)
        );
        assert_eq!(
            inspect_content("relatorio.pdf", b"%PDF-1.7 /Catalog startxref 0 %%EOF"),
            Err(DocumentError::Structure)
        );
        assert_eq!(
            inspect_content("relatorio.pdf", b"%PDF-1.7 /Catalog startxref 0"),
            Err(DocumentError::Structure)
        );
    }
    #[test]
    fn text_is_bounded_and_spreadsheet_formulas_are_not_accepted() {
        assert_eq!(
            inspect_content("nota.txt", "Observação: concreto recebido.\n".as_bytes()),
            Ok(DocumentFormat::Text)
        );
        assert_eq!(
            inspect_content("planilha.csv", b"item;quantidade\r\nconcreto;12"),
            Ok(DocumentFormat::Csv)
        );
        assert_eq!(
            inspect_content("planilha.csv", b"item;\"  =HYPERLINK(\"x\")\""),
            Err(DocumentError::Prohibited)
        );
        assert_eq!(
            inspect_content("nota.txt", b"\x00texto"),
            Err(DocumentError::Prohibited)
        );
        assert_eq!(
            inspect_content("../nota.txt", b"Texto"),
            Err(DocumentError::Name)
        );
        assert_eq!(inspect_content("nota.txt", &[]), Err(DocumentError::Size));
        assert_eq!(
            inspect_content("nota.txt", &vec![b'a'; MAX_DOCUMENT_BYTES + 1]),
            Err(DocumentError::Size)
        );
    }
}
