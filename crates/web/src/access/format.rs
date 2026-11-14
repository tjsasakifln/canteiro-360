use canteiro_contracts::forms::{DecimalKind, format_decimal_pt_br, parse_decimal_pt_br};
#[cfg(any(feature = "hydrate", test))]
use chrono::Timelike;
use chrono::{NaiveDateTime, TimeZone};

pub(super) fn money_input(value: &str) -> Result<String, String> {
    parse_decimal_pt_br(value, DecimalKind::Money)
        .map(|parsed| parsed.canonical)
        .map_err(|error| error.message)
}

pub(super) fn money(value: &str) -> String {
    format!("R$ {}", format_decimal_pt_br(value))
}

pub(super) fn instant(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%d/%m/%Y às %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "Data indisponível".into())
}

pub(super) fn local_instant(value: &str) -> Result<String, String> {
    let naive = NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%dT%H:%M")
        .map_err(|_| "Informe a data e o horário completos, no horário de Brasília.".to_string())?;
    chrono_tz::America::Sao_Paulo
        .from_local_datetime(&naive)
        .single()
        .map(|date| date.to_rfc3339())
        .ok_or_else(|| {
            "Este horário não é único no fuso de Brasília. Escolha outro horário.".into()
        })
}

pub(super) fn local_date_time(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%Y-%m-%dT%H:%M")
                .to_string()
        })
        .unwrap_or_default()
}

pub(super) fn current_local_time() -> String {
    #[cfg(feature = "hydrate")]
    {
        local_date_time(
            &js_sys::Date::new_0()
                .to_iso_string()
                .as_string()
                .unwrap_or_default(),
        )
    }
    #[cfg(not(feature = "hydrate"))]
    {
        String::new()
    }
}

/// A minute-based control must never round the source's beginning backwards.
#[cfg(any(feature = "hydrate", test))]
fn delegation_start_at(source: &str, current: &str) -> Option<String> {
    let source = chrono::DateTime::parse_from_rfc3339(source).ok()?;
    let current = chrono::DateTime::parse_from_rfc3339(current).ok()?;
    let source_minute = source.with_second(0)?.with_nanosecond(0)?;
    let earliest = if source_minute < source {
        source_minute.checked_add_signed(chrono::Duration::minutes(1))?
    } else {
        source_minute
    };
    let current_minute = current.with_second(0)?.with_nanosecond(0)?;
    Some(
        earliest
            .max(current_minute)
            .with_timezone(&chrono_tz::America::Sao_Paulo)
            .format("%Y-%m-%dT%H:%M")
            .to_string(),
    )
}

pub(super) fn delegation_start(source: &str) -> String {
    #[cfg(feature = "hydrate")]
    {
        delegation_start_at(
            source,
            &js_sys::Date::new_0()
                .to_iso_string()
                .as_string()
                .unwrap_or_default(),
        )
        .or_else(|| delegation_start_at(source, source))
        .unwrap_or_default()
    }
    #[cfg(not(feature = "hydrate"))]
    {
        local_date_time(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn money_never_guesses_ambiguous_input_or_rounds() {
        assert_eq!(money_input("1.234,56").unwrap(), "1234.56");
        assert!(money_input("1.234").is_err());
        assert!(money_input("1,239").is_err());
        assert!(money_input("-100,00").is_err());
    }
    #[test]
    fn brazilian_local_time_is_sent_with_explicit_offset() {
        assert_eq!(
            local_instant("2026-11-14T09:30").unwrap(),
            "2026-11-14T09:30:00-03:00"
        );
        assert_eq!(local_date_time("2026-11-14T12:30:00Z"), "2026-11-14T09:30");
        assert!(local_instant("2026-02-30T09:30").is_err());
        assert!(local_instant("2018-11-04T00:30").is_err());
    }
    #[test]
    fn suggested_delegation_starts_now_without_rounding_before_source() {
        assert_eq!(
            delegation_start_at("2020-01-01T00:00:00Z", "2026-11-14T12:30:45Z").as_deref(),
            Some("2026-11-14T09:30")
        );
        assert_eq!(
            delegation_start_at("2026-11-14T12:30:20.123Z", "2026-11-14T12:30:45Z").as_deref(),
            Some("2026-11-14T09:31")
        );
        assert_eq!(
            delegation_start_at("2026-11-14T12:35:00Z", "2026-11-14T12:30:45Z").as_deref(),
            Some("2026-11-14T09:35")
        );
    }
}
