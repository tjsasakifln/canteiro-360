use canteiro_domain::CivilDate;
use serde::{Deserialize, Serialize};

/// Input limits describe the field, not a rounding instruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DecimalKind {
    Money,
    UnitPrice,
    Quantity,
    Percent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FieldError {
    pub code: String,
    pub message: String,
}

impl FieldError {
    pub(super) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FieldError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedDecimal {
    pub canonical: String,
    pub display: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParsedDate {
    pub canonical: String,
    pub display: String,
}

/// An explicitly selected Brazilian convention avoids a locale guess. A lone
/// dot is deliberately rejected: the user must make the interpretation clear.
pub fn parse_decimal_pt_br(input: &str, kind: DecimalKind) -> Result<ParsedDecimal, FieldError> {
    if input.len() > 128 {
        return Err(FieldError::new(
            "length",
            "O valor é muito longo. Confira os dígitos informados.",
        ));
    }
    let mut input = input.trim();
    if matches!(kind, DecimalKind::Money | DecimalKind::UnitPrice) {
        input = input.strip_prefix("R$").unwrap_or(input).trim();
    }
    if kind == DecimalKind::Percent {
        input = input.strip_suffix('%').unwrap_or(input).trim();
    }
    if input.is_empty() {
        return Err(FieldError::new("required", "Informe o valor deste campo."));
    }
    if input.starts_with('-') {
        return Err(FieldError::new(
            "negative",
            "Informe um valor igual ou maior que zero.",
        ));
    }
    if input.contains('.') && !input.contains(',') {
        return Err(FieldError::new(
            "ambiguous",
            "O ponto sozinho é ambíguo. Para milhar, escreva 1.234,00; para decimal, use vírgula, como 1,234.",
        ));
    }
    let mut parts = input.split(',');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some() || integer.is_empty() {
        return Err(decimal_format_error());
    }
    let integer = if integer.contains('.') {
        let groups: Vec<_> = integer.split('.').collect();
        if !(1..=3).contains(&groups[0].len())
            || groups[0].starts_with('0')
            || !groups[0].bytes().all(|byte| byte.is_ascii_digit())
            || !groups
                .iter()
                .skip(1)
                .all(|group| group.len() == 3 && group.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(decimal_format_error());
        }
        groups.concat()
    } else {
        if !integer.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(decimal_format_error());
        }
        integer.to_owned()
    };
    let scale = match kind {
        DecimalKind::Money => 2,
        _ => 6,
    };
    let fraction = match fraction {
        Some(value) if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) => {
            return Err(decimal_format_error());
        }
        Some(value) if value.len() > scale => {
            return Err(FieldError::new(
                "scale",
                &format!("Use no máximo {scale} casas decimais. O valor não foi arredondado."),
            ));
        }
        Some(value) => value,
        None => "",
    };
    let integer = integer.trim_start_matches('0');
    let integer = if integer.is_empty() { "0" } else { integer };
    let max_integer = if kind == DecimalKind::Money { 18 } else { 14 };
    if integer.len() > max_integer {
        return Err(FieldError::new(
            "range",
            "O valor ultrapassa o limite deste campo. Confira a magnitude informada.",
        ));
    }
    let canonical = if fraction.is_empty() {
        integer.to_owned()
    } else {
        format!("{integer}.{fraction}")
    };
    let exact = Exact::parse(&canonical)?;
    if kind == DecimalKind::Percent && exact.scaled(6)? > 100_000_000 {
        return Err(FieldError::new(
            "range",
            "Informe um percentual entre 0 e 100.",
        ));
    }
    let canonical = exact.text(if kind == DecimalKind::Money { 2 } else { 0 });
    Ok(ParsedDecimal {
        display: format_decimal_pt_br(&canonical),
        canonical,
    })
}

fn decimal_format_error() -> FieldError {
    FieldError::new(
        "format",
        "Use o formato brasileiro: 1234,56 ou 1.234,56, sem misturar separadores.",
    )
}

/// Formats already canonical decimal text. Invalid input remains visible;
/// formatting never silently turns it into a numerical zero.
pub fn format_decimal_pt_br(canonical: &str) -> String {
    let Ok(value) = Exact::parse(canonical) else {
        return canonical.to_owned();
    };
    let fraction_size = canonical
        .split_once('.')
        .map_or(0, |(_, value)| value.len());
    let normalized = value.text(fraction_size);
    let (integer, fraction) = normalized.split_once('.').unwrap_or((&normalized, ""));
    let mut formatted = String::with_capacity(normalized.len() + integer.len() / 3);
    for (index, ch) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index).is_multiple_of(3) {
            formatted.push('.');
        }
        formatted.push(ch);
    }
    if !fraction.is_empty() {
        formatted.push(',');
        formatted.push_str(fraction);
    }
    formatted
}

pub fn parse_date_pt_br(input: &str) -> Result<ParsedDate, FieldError> {
    let input = input.trim();
    if input.len() != 10
        || input.as_bytes()[2] != b'/'
        || input.as_bytes()[5] != b'/'
        || !input.bytes().enumerate().all(|(i, byte)| {
            if i == 2 || i == 5 {
                byte == b'/'
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        return Err(FieldError::new(
            "date",
            "Informe uma data válida no formato dia/mês/ano, como 12/11/2026.",
        ));
    }
    let canonical = format!("{}-{}-{}", &input[6..10], &input[3..5], &input[0..2]);
    canonical.parse::<CivilDate>().map_err(|_| {
        FieldError::new(
            "date",
            "Esta data não existe no calendário. Confira dia, mês e ano.",
        )
    })?;
    Ok(ParsedDate {
        canonical,
        display: input.to_owned(),
    })
}

pub fn parse_unit(input: &str) -> Result<String, FieldError> {
    match input.trim() {
        "un" | "unidade" => Ok("un".into()),
        "m" => Ok("m".into()),
        "m²" | "m2" => Ok("m²".into()),
        "m³" | "m3" => Ok("m³".into()),
        "kg" => Ok("kg".into()),
        "h" => Ok("h".into()),
        _ => Err(FieldError::new(
            "unit",
            "Selecione a unidade do serviço: un, m, m², m³, kg ou h.",
        )),
    }
}

/// Small checked fixed-point value used by previews only. It cannot round or
/// saturate a user input, and does not replace the full calculation engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Exact {
    pub coefficient: u128,
    pub scale: u32,
}

impl Exact {
    pub fn parse(value: &str) -> Result<Self, FieldError> {
        let mut parts = value.split('.');
        let integer = parts.next().unwrap_or_default();
        let fraction = parts.next();
        if integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || parts.next().is_some()
            || fraction.is_some_and(|f| {
                f.is_empty() || f.len() > 6 || !f.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Err(decimal_format_error());
        }
        let fraction = fraction.unwrap_or("");
        let coefficient = format!("{integer}{fraction}")
            .parse::<u128>()
            .map_err(|_| overflow_error())?;
        Ok(Self {
            coefficient,
            scale: fraction.len() as u32,
        })
    }

    pub fn scaled(self, scale: u32) -> Result<u128, FieldError> {
        if scale < self.scale {
            return Err(overflow_error());
        }
        self.coefficient
            .checked_mul(
                10_u128
                    .checked_pow(scale - self.scale)
                    .ok_or_else(overflow_error)?,
            )
            .ok_or_else(overflow_error)
    }

    pub fn text(self, minimum_scale: usize) -> String {
        let divisor = 10_u128.pow(self.scale);
        let integer = self.coefficient / divisor;
        let mut fraction = if self.scale == 0 {
            String::new()
        } else {
            format!(
                "{:0width$}",
                self.coefficient % divisor,
                width = self.scale as usize
            )
        };
        while fraction.ends_with('0') && fraction.len() > minimum_scale {
            fraction.pop();
        }
        while fraction.len() < minimum_scale {
            fraction.push('0');
        }
        if fraction.is_empty() {
            integer.to_string()
        } else {
            format!("{integer}.{fraction}")
        }
    }

    pub fn cents_times(self, rhs: Self) -> Result<u128, FieldError> {
        let coefficient = self
            .coefficient
            .checked_mul(rhs.coefficient)
            .ok_or_else(overflow_error)?;
        // At most 28 significant digits for this preliminary arithmetic.
        if coefficient >= 10_u128.pow(28) {
            return Err(overflow_error());
        }
        let scale = self.scale + rhs.scale;
        if scale <= 2 {
            coefficient
                .checked_mul(10_u128.pow(2 - scale))
                .ok_or_else(overflow_error)
        } else {
            Ok(coefficient / 10_u128.pow(scale - 2))
        }
    }
}

pub(super) fn overflow_error() -> FieldError {
    FieldError::new(
        "overflow",
        "Esta combinação de quantidade e preço ultrapassa o limite da prévia. Confira os valores.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_brazilian_values_preserve_precision_and_report_the_interpretation() {
        for (raw, kind, canonical, display) in [
            ("1.234,56", DecimalKind::Money, "1234.56", "1.234,56"),
            (" R$ 1.234,50 ", DecimalKind::Money, "1234.50", "1.234,50"),
            ("0", DecimalKind::Money, "0.00", "0,00"),
            ("00012,300000", DecimalKind::Quantity, "12.3", "12,3"),
            ("1,234", DecimalKind::Quantity, "1.234", "1,234"),
            ("0,335", DecimalKind::UnitPrice, "0.335", "0,335"),
            (
                "10,123456",
                DecimalKind::UnitPrice,
                "10.123456",
                "10,123456",
            ),
            (" 100,000000 % ", DecimalKind::Percent, "100", "100"),
            (
                "999999999999999999,99",
                DecimalKind::Money,
                "999999999999999999.99",
                "999.999.999.999.999.999,99",
            ),
        ] {
            let value = parse_decimal_pt_br(raw, kind).unwrap();
            assert_eq!(value.canonical, canonical, "{raw}");
            assert_eq!(value.display, display, "{raw}");
        }
    }

    #[test]
    fn ambiguity_excess_precision_and_malformed_values_fail_without_rounding() {
        for value in [
            "1.234",
            "1.23",
            "1,234.56",
            "12.34,56",
            "1.234.56,00",
            "1,,2",
            "1,",
            ",5",
            "1 234,56",
            "1e3",
            "NaN",
            "Infinity",
            "-1",
            "+1",
            "0.123,00",
            "１,２",
            "",
            "0,0000001",
        ] {
            assert!(
                parse_decimal_pt_br(value, DecimalKind::UnitPrice).is_err(),
                "{value}"
            );
        }
        assert_eq!(
            parse_decimal_pt_br("1.234", DecimalKind::Quantity)
                .unwrap_err()
                .code,
            "ambiguous"
        );
        assert_eq!(
            parse_decimal_pt_br("1,234", DecimalKind::Money)
                .unwrap_err()
                .code,
            "scale"
        );
        assert!(parse_decimal_pt_br("100,000001", DecimalKind::Percent).is_err());
        assert!(parse_decimal_pt_br("100000000000000", DecimalKind::Quantity).is_err());
        assert!(parse_decimal_pt_br("1000000000000000000", DecimalKind::Money).is_err());
        assert_eq!(format_decimal_pt_br("invalid"), "invalid");
    }

    #[test]
    fn civil_dates_and_units_never_guess_foreign_formats_or_invent_units() {
        assert_eq!(
            parse_date_pt_br("29/02/2028").unwrap().canonical,
            "2028-02-29"
        );
        assert_eq!(
            parse_date_pt_br("01/02/2026").unwrap().canonical,
            "2026-02-01"
        );
        for date in [
            "29/02/2026",
            "31/04/2026",
            "12/31/2026",
            "2026-11-12",
            "1/2/2026",
            "00/01/2026",
            "01/01/0000",
            "😄/11/2026",
        ] {
            assert!(parse_date_pt_br(date).is_err(), "{date}");
        }
        assert_eq!(parse_unit(" m2 ").unwrap(), "m²");
        assert_eq!(parse_unit("m3").unwrap(), "m³");
        let invalid_markup = ["un<", "script>"].concat();
        for unit in ["", "m4", "M", "t", invalid_markup.as_str()] {
            assert!(parse_unit(unit).is_err());
        }
    }
}
