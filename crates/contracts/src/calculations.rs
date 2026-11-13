//! Bounded previews. They do not write a contract, payment or approved schedule.
use crate::forms::FieldError;
use canteiro_domain::{
    CivilDate,
    calendar::{Cadence, CalendarPeriod, CalendarPolicy, days_between, schedule},
    decimal::{
        AllocationWeight, CalculationPolicy, ExactDecimal, Money, allocate_money,
        allocate_percentages,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationRequest {
    pub policy: CalculationPolicy,
    pub total: Money,
    pub shares: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SharePreview {
    pub position: u32,
    pub amount: String,
    pub percent: String,
    pub received_cent: bool,
    pub received_percent_residual: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AllocationPreview {
    pub policy: CalculationPolicy,
    pub total: String,
    pub shares: Vec<SharePreview>,
}

fn failure(code: &str, message: impl ToString) -> FieldError {
    FieldError {
        code: code.to_owned(),
        message: message.to_string(),
    }
}

pub fn preview_allocation(request: &AllocationRequest) -> Result<AllocationPreview, FieldError> {
    if !(1..=100).contains(&request.shares) {
        return Err(failure(
            "shares",
            "Informe entre 1 e 100 parcelas para esta prévia.",
        ));
    }
    let weights: Vec<_> = (1..=request.shares)
        .map(|position| AllocationWeight {
            id: format!("parcela-{position:03}"),
            position,
            weight: ExactDecimal::ONE,
        })
        .collect();
    let amount = allocate_money(request.total, &weights).map_err(|e| failure("amount", e))?;
    let percent = allocate_percentages(&weights).map_err(|e| failure("amount", e))?;
    let shares = amount
        .parts
        .iter()
        .zip(&percent.parts)
        .map(|(a, p)| {
            Ok(SharePreview {
                position: a.position,
                amount: a.value.fixed_text(2).map_err(|e| failure("amount", e))?,
                percent: p.value.fixed_text(2).map_err(|e| failure("amount", e))?,
                received_cent: a.received_residual,
                received_percent_residual: p.received_residual,
            })
        })
        .collect::<Result<Vec<_>, FieldError>>()?;
    Ok(AllocationPreview {
        policy: request.policy,
        total: request.total.to_string(),
        shares,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarRequest {
    pub policy: CalculationPolicy,
    pub calendar_policy: CalendarPolicy,
    pub cadence: Cadence,
    pub anchor: CivilDate,
    pub formal_end: CivilDate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SchedulePreview {
    pub policy: CalculationPolicy,
    pub calendar_policy: CalendarPolicy,
    pub anchor: CivilDate,
    pub formal_end: CivilDate,
    pub duration_days: u32,
    pub periods: Vec<CalendarPeriod>,
}

pub fn preview_calendar(request: &CalendarRequest) -> Result<SchedulePreview, FieldError> {
    let days = days_between(request.anchor, request.formal_end).map_err(|e| failure("date", e))?;
    if days > 36_600 {
        return Err(failure(
            "date_range",
            "Esta prévia admite até 36.600 dias. Reduza o intervalo para conferir o cronograma.",
        ));
    }
    let periods = schedule(
        request.anchor,
        request.formal_end,
        request.calendar_policy,
        request.cadence,
    )
    .map_err(|e| failure("date", e))?;
    Ok(SchedulePreview {
        policy: request.policy,
        calendar_policy: request.calendar_policy,
        anchor: request.anchor,
        formal_end: request.formal_end,
        duration_days: days,
        periods,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn previews_explain_residual_and_reject_unbounded_or_unknown_input() {
        let request = AllocationRequest {
            policy: CalculationPolicy::default(),
            total: "100.00".parse().unwrap(),
            shares: 3,
        };
        let result = preview_allocation(&request).unwrap();
        assert_eq!(
            result
                .shares
                .iter()
                .map(|r| r.amount.as_str())
                .collect::<Vec<_>>(),
            ["33.34", "33.33", "33.33"]
        );
        assert!(result.shares[0].received_cent);
        assert_eq!(result.shares[0].percent, "33.34");
        for shares in [0, 101, u32::MAX] {
            assert!(
                preview_allocation(&AllocationRequest {
                    shares,
                    ..request.clone()
                })
                .is_err()
            );
        }
        assert!(
            serde_json::from_str::<AllocationRequest>(
                r#"{"policy":"future","total":"100.00","shares":3}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AllocationRequest>(
                r#"{"policy":"C360-DECIMAL-1","total":100,"shares":3}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AllocationRequest>(
                r#"{"policy":"C360-DECIMAL-1","total":"100.00","shares":3,"rounding":"up"}"#
            )
            .is_err()
        );
        let calendar = CalendarRequest {
            policy: CalculationPolicy::default(),
            calendar_policy: CalendarPolicy::Civil,
            cadence: Cadence::Monthly,
            anchor: "2027-01-31".parse().unwrap(),
            formal_end: "2027-03-31".parse().unwrap(),
        };
        let result = preview_calendar(&calendar).unwrap();
        assert_eq!(result.duration_days, 59);
        assert_eq!(result.periods[0].end.to_string(), "2027-02-28");
        assert_eq!(result.periods[1].end.to_string(), "2027-03-31");
        assert!(
            preview_calendar(&CalendarRequest {
                formal_end: "9999-12-31".parse().unwrap(),
                ..calendar
            })
            .is_err()
        );
    }
}
