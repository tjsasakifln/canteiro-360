//! Calendar rules from C360-DECIMAL-1, §9, and the private decision CR-02.
//!
//! All dates are civil dates. A schedule keeps its original anchor; projecting a
//! pause never changes an approved term or the instrument's validity. These
//! functions calculate previews, not permission to perform or approve an act.

use crate::{CivilDate, UtcInstant};
use chrono::{
    Datelike, Days, LocalResult, Months, NaiveTime, Offset, SecondsFormat, TimeZone, Utc,
};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::{collections::BTreeMap, fmt, str::FromStr};

/// Domain failures intentionally do not retain user input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CalendarError {
    #[error("O cálculo ultrapassa as datas admitidas, entre os anos 0001 e 9999.")]
    DateRange,
    #[error("A data final deve ser igual ou posterior à data inicial.")]
    ReversedInterval,
    #[error("Informe uma frente válida para apurar a paralisação.")]
    InvalidFront,
    #[error("Há paralisações sobrepostas na mesma frente. Confira os intervalos.")]
    OverlappingPauses,
    #[error("Informe a data de referência para projetar uma paralisação ainda aberta.")]
    OpenPauseNeedsReference,
    #[error("A referência da projeção não pode anteceder a paralisação aberta.")]
    ReferenceBeforePause,
    #[error("Escolha um fuso regional reconhecido, como America/Sao_Paulo.")]
    InvalidTimeZone,
    #[error("Informe data e hora local válidas, com precisão de até microssegundos.")]
    InvalidLocalDateTime,
    #[error("Esse horário local não existiu nesse fuso. Confira a mudança de horário.")]
    NonexistentLocalDateTime,
    #[error("Esse horário local ocorreu duas vezes. Informe um instante com offset explícito.")]
    AmbiguousLocalDateTime,
    #[error("Esse fuso histórico usa segundos no deslocamento. Consulte o instante em UTC.")]
    SubminuteOffset,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CalendarPolicy {
    #[serde(rename = "DIAS_FIXOS_SICOP")]
    FixedDays,
    #[default]
    #[serde(rename = "CALENDARIO_CIVIL")]
    Civil,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Cadence {
    #[serde(rename = "SEMANAL")]
    Weekly,
    #[serde(rename = "QUINZENAL")]
    Fortnightly,
    #[serde(rename = "MENSAL")]
    Monthly,
    #[serde(rename = "BIMESTRAL")]
    Bimonthly,
    #[serde(rename = "TRIMESTRAL")]
    Quarterly,
    #[serde(rename = "SEMESTRAL")]
    Semiannual,
    #[serde(rename = "ANUAL")]
    Annual,
}

impl Cadence {
    pub const fn fixed_days(self) -> u32 {
        match self {
            Self::Weekly => 7,
            Self::Fortnightly => 15,
            Self::Monthly => 30,
            Self::Bimonthly => 60,
            Self::Quarterly => 90,
            Self::Semiannual => 180,
            Self::Annual => 365,
        }
    }

    const fn civil_months(self) -> Option<u32> {
        match self {
            Self::Weekly | Self::Fortnightly => None,
            Self::Monthly => Some(1),
            Self::Bimonthly => Some(2),
            Self::Quarterly => Some(3),
            Self::Semiannual => Some(6),
            Self::Annual => Some(12),
        }
    }
}

/// Add calendar days. The first counted day is the day following `start`.
pub fn execution_end(start: CivilDate, days: u32) -> Result<CivilDate, CalendarError> {
    let end = start
        .as_naive_date()
        .checked_add_days(Days::new(u64::from(days)))
        .ok_or(CalendarError::DateRange)?;
    CivilDate::from_naive_date(end).map_err(|_| CalendarError::DateRange)
}

/// The duration of [start, end), without timezone or inclusive-day adjustments.
pub fn days_between(start: CivilDate, end: CivilDate) -> Result<u32, CalendarError> {
    let days = end
        .as_naive_date()
        .signed_duration_since(start.as_naive_date())
        .num_days();
    u32::try_from(days).map_err(|_| CalendarError::ReversedInterval)
}

/// Advance from the ORIGINAL anchor. Never feed a clamped February boundary
/// back as a new anchor: January 31 + two months is March 31, not March 28.
pub fn civil_months_after(anchor: CivilDate, months: u32) -> Result<CivilDate, CalendarError> {
    let end = anchor
        .as_naive_date()
        .checked_add_months(Months::new(months))
        .ok_or(CalendarError::DateRange)?;
    CivilDate::from_naive_date(end).map_err(|_| CalendarError::DateRange)
}

pub fn advance(
    anchor: CivilDate,
    policy: CalendarPolicy,
    cadence: Cadence,
    periods: u32,
) -> Result<CivilDate, CalendarError> {
    if policy == CalendarPolicy::Civil
        && let Some(months) = cadence.civil_months()
    {
        return civil_months_after(
            anchor,
            months
                .checked_mul(periods)
                .ok_or(CalendarError::DateRange)?,
        );
    }
    execution_end(
        anchor,
        cadence
            .fixed_days()
            .checked_mul(periods)
            .ok_or(CalendarError::DateRange)?,
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CalendarPeriod {
    pub start: CivilDate,
    pub end: CivilDate,
    pub days: u32,
    /// The formal term ended before the next full cadence boundary.
    pub residual: bool,
}

/// Produce contiguous periods ending EXACTLY on the formal term. The date
/// domain bounds the maximum result (weekly periods across 0001..9999).
pub fn schedule(
    anchor: CivilDate,
    formal_end: CivilDate,
    policy: CalendarPolicy,
    cadence: Cadence,
) -> Result<Vec<CalendarPeriod>, CalendarError> {
    days_between(anchor, formal_end)?;
    let mut periods = Vec::new();
    let mut start = anchor;
    let mut index = 1_u32;
    while start < formal_end {
        // If the cadence falls beyond 9999 but the supplied formal end does
        // not, the last period is still a valid residual within the domain.
        let boundary = match advance(anchor, policy, cadence, index) {
            Ok(value) => Some(value),
            Err(CalendarError::DateRange) => None,
            Err(error) => return Err(error),
        };
        let end = boundary.map_or(formal_end, |date| date.min(formal_end));
        periods.push(CalendarPeriod {
            start,
            end,
            days: days_between(start, end)?,
            residual: boundary.is_none_or(|date| date > formal_end),
        });
        start = end;
        index = index.checked_add(1).ok_or(CalendarError::DateRange)?;
    }
    Ok(periods)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionWindow {
    pub start: CivilDate,
    pub end: CivilDate,
    pub days: u32,
    pub valid_from: CivilDate,
    pub valid_to: CivilDate,
    /// A false value blocks a consumer's consolidation. It does not hide the
    /// calculated end date, which remains available to explain the correction.
    pub within_validity: bool,
}

pub fn execution_window(
    start: CivilDate,
    days: u32,
    valid_from: CivilDate,
    valid_to: CivilDate,
) -> Result<ExecutionWindow, CalendarError> {
    days_between(valid_from, valid_to)?;
    let end = execution_end(start, days)?;
    Ok(ExecutionWindow {
        start,
        end,
        days,
        valid_from,
        valid_to,
        within_validity: start >= valid_from && end <= valid_to,
    })
}

/// Untrusted interval input. Every operation validates the ENTIRE set before
/// adding any day; deserializing an interval does not make it an approved act.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PauseInterval {
    pub front: String,
    pub start: CivilDate,
    pub restart: Option<CivilDate>,
}

fn validate_front(front: &str) -> Result<(), CalendarError> {
    if front.is_empty()
        || front.len() > 128
        || front.trim() != front
        || front.chars().any(char::is_control)
    {
        return Err(CalendarError::InvalidFront);
    }
    Ok(())
}

pub fn validate_pauses(pauses: &[PauseInterval]) -> Result<(), CalendarError> {
    let mut fronts: BTreeMap<&str, Vec<&PauseInterval>> = BTreeMap::new();
    for pause in pauses {
        validate_front(&pause.front)?;
        if let Some(restart) = pause.restart {
            days_between(pause.start, restart)?;
            // Empty intervals have no occupied days and cannot overlap.
            if pause.start == restart {
                continue;
            }
        }
        fronts.entry(&pause.front).or_default().push(pause);
    }
    for intervals in fronts.values_mut() {
        intervals.sort_by_key(|pause| pause.start);
        for pair in intervals.windows(2) {
            if pair[0].restart.is_none_or(|end| pair[1].start < end) {
                return Err(CalendarError::OverlappingPauses);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PauseDays {
    pub front: String,
    pub closed_days: u32,
    pub open_days: u32,
    pub total_days: u32,
    pub has_open_pause: bool,
    /// Present only when an open pause needed a projection reference.
    pub as_of: Option<CivilDate>,
}

/// Count only the explicitly selected front. Never add overlapping days from
/// different fronts and label that sum an extension of the entire work.
pub fn pause_days(
    pauses: &[PauseInterval],
    front: &str,
    as_of: Option<CivilDate>,
) -> Result<PauseDays, CalendarError> {
    validate_front(front)?;
    validate_pauses(pauses)?;
    let mut closed_days = 0_u32;
    let mut open_days = 0_u32;
    let mut has_open_pause = false;
    for pause in pauses.iter().filter(|pause| pause.front == front) {
        match pause.restart {
            Some(restart) => {
                closed_days = closed_days
                    .checked_add(days_between(pause.start, restart)?)
                    .ok_or(CalendarError::DateRange)?;
            }
            None => {
                let reference = as_of.ok_or(CalendarError::OpenPauseNeedsReference)?;
                if reference < pause.start {
                    return Err(CalendarError::ReferenceBeforePause);
                }
                open_days = days_between(pause.start, reference)?;
                has_open_pause = true;
            }
        }
    }
    Ok(PauseDays {
        front: front.to_owned(),
        closed_days,
        open_days,
        total_days: closed_days
            .checked_add(open_days)
            .ok_or(CalendarError::DateRange)?,
        has_open_pause,
        as_of: if has_open_pause { as_of } else { None },
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExecutionProjection {
    pub front: String,
    /// Immutable input retained alongside the preview, never overwritten.
    pub formal_end: CivilDate,
    pub projected_end: CivilDate,
    pub valid_to: CivilDate,
    pub pause: PauseDays,
    pub within_validity: bool,
    /// True while the selected front has an open pause; executing is blocked.
    pub provisional: bool,
}

/// `formal_end` must precede the pauses in this batch in the consumer's
/// approved revision history. The consumer applies an approved batch once;
/// this pure preview neither mutates dates nor claims command idempotency.
pub fn project_execution(
    formal_end: CivilDate,
    valid_to: CivilDate,
    front: &str,
    pauses: &[PauseInterval],
    as_of: Option<CivilDate>,
) -> Result<ExecutionProjection, CalendarError> {
    let pause = pause_days(pauses, front, as_of)?;
    let projected_end = execution_end(formal_end, pause.total_days)?;
    Ok(ExecutionProjection {
        front: front.to_owned(),
        formal_end,
        projected_end,
        valid_to,
        within_validity: projected_end <= valid_to,
        provisional: pause.has_open_pause,
        pause,
    })
}

/// A checked IANA timezone, independent of a civil contract date.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeZoneId(Tz);

impl FromStr for TimeZoneId {
    type Err = CalendarError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() > 128 || value.trim() != value {
            return Err(CalendarError::InvalidTimeZone);
        }
        value
            .parse::<Tz>()
            .map(Self)
            .map_err(|_| CalendarError::InvalidTimeZone)
    }
}

impl fmt::Display for TimeZoneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name())
    }
}

impl Serialize for TimeZoneId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TimeZoneId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Local ISO text without offset, `YYYY-MM-DDTHH:MM:SS[.ffffff]`.
/// A fold is never guessed; callers with an explicit offset may parse an
/// existing UtcInstant, which retains the unambiguous audit instant.
pub fn local_to_utc(value: &str, zone: TimeZoneId) -> Result<UtcInstant, CalendarError> {
    let invalid = CalendarError::InvalidLocalDateTime;
    if !(19..=26).contains(&value.len())
        || !value.is_ascii()
        || value.as_bytes()[10] != b'T'
        || value.as_bytes()[13] != b':'
        || value.as_bytes()[16] != b':'
    {
        return Err(invalid);
    }
    let date = value[..10].parse::<CivilDate>().map_err(|_| invalid)?;
    let parse_pair = |start: usize| -> Result<u32, CalendarError> {
        let part = &value[start..start + 2];
        if !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid);
        }
        part.parse().map_err(|_| invalid)
    };
    let microseconds = if value.len() == 19 {
        0
    } else {
        if value.as_bytes()[19] != b'.' {
            return Err(invalid);
        }
        let fraction = &value[20..];
        if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid);
        }
        fraction.parse::<u32>().map_err(|_| invalid)? * 10_u32.pow(6 - fraction.len() as u32)
    };
    let time = NaiveTime::from_hms_micro_opt(
        parse_pair(11)?,
        parse_pair(14)?,
        parse_pair(17)?,
        microseconds,
    )
    .ok_or(invalid)?;
    match zone
        .0
        .from_local_datetime(&date.as_naive_date().and_time(time))
    {
        LocalResult::Single(instant) => UtcInstant::from_datetime(instant.with_timezone(&Utc))
            .map_err(|_| CalendarError::DateRange),
        LocalResult::Ambiguous(_, _) => Err(CalendarError::AmbiguousLocalDateTime),
        LocalResult::None => Err(CalendarError::NonexistentLocalDateTime),
    }
}

pub fn utc_to_local(instant: UtcInstant, zone: TimeZoneId) -> Result<String, CalendarError> {
    let local = instant.as_datetime().with_timezone(&zone.0);
    if !(1..=9999).contains(&local.year()) {
        return Err(CalendarError::DateRange);
    }
    // RFC3339 offsets have minute precision. Historical IANA offsets can have
    // seconds; chrono would round only the offset, producing text that resolves
    // to a DIFFERENT instant. Preserve the original UTC value instead of
    // silently shifting the event while formatting it.
    if local.offset().fix().local_minus_utc() % 60 != 0 {
        return Err(CalendarError::SubminuteOffset);
    }
    Ok(local.to_rfc3339_opts(SecondsFormat::Micros, false))
}
