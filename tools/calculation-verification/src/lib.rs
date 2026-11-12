//! Test transport for the real domain engine on native Rust and WebAssembly.
//!
//! This crate contains no expected financial result, reference fixture, or
//! independent implementation of a calculation. It only adapts requests to the
//! domain API and projects its real results into a stable comparison shape.
//! It is not linked into the production application.
#![forbid(unsafe_code)]

use canteiro_domain::{
    CivilDate,
    calendar::{self, Cadence, CalendarPolicy, PauseInterval},
    decimal::{
        self, Allocation, AllocationWeight, ExactDecimal, IndexValue, Money, Percent, Quantity,
        UnitPrice,
    },
    units::{MeasureUnit, QuantityWithUnit, UnitError},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{fmt::Debug, str::FromStr};

type Outcome = Result<Value, String>;

fn checked<T, E: Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

fn field<'a>(value: &'a Value, key: &str) -> Result<&'a Value, String> {
    value.get(key).ok_or_else(|| "Format".to_owned())
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    field(value, key)?
        .as_str()
        .ok_or_else(|| "Format".to_owned())
}

fn number(value: &Value, key: &str) -> Result<u32, String> {
    field(value, key)?
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| "Format".to_owned())
}

fn parse<T: FromStr>(value: &Value, key: &str) -> Result<T, String>
where
    T::Err: Debug,
{
    checked(text(value, key)?.parse())
}

fn typed<T>(raw: &Value) -> Result<T, String>
where
    T: FromStr + DeserializeOwned,
    T::Err: Debug,
{
    if let Some(text) = raw.as_str() {
        checked(text.parse())
    } else {
        // A deliberate numeric JSON input must reach the REAL type's serde
        // boundary. If a regression accepts the number, return that value and
        // let the independent expected rejection fail. Do not reject it here.
        serde_json::from_value(raw.clone()).map_err(|_| "Format".to_owned())
    }
}

fn rendered(value: ExactDecimal, scale: u32) -> Outcome {
    Ok(json!({"value": checked(value.fixed_text(scale))?}))
}

fn typed_parse(input: &Value) -> Outcome {
    let raw = field(input, "value")?;
    let value = match text(input, "kind")? {
        "quantity" => typed::<Quantity>(raw)?.value(),
        "unit_price" => typed::<UnitPrice>(raw)?.value(),
        "money" => typed::<Money>(raw)?.value(),
        "percent" => typed::<Percent>(raw)?.value(),
        "index" => typed::<IndexValue>(raw)?.value(),
        "exact" => typed::<ExactDecimal>(raw)?,
        _ => return Err("Format".to_owned()),
    };
    rendered(value, number(input, "output_scale")?)
}

fn arithmetic(input: &Value) -> Outcome {
    let left: ExactDecimal = parse(input, "left")?;
    let right: ExactDecimal = parse(input, "right")?;
    let scale = number(input, "output_scale")?;
    let value = checked(match text(input, "method")? {
        "add" => left.checked_add(right),
        "sub" => left.checked_sub(right),
        "mul" => left.checked_mul(right),
        "div_exact" => left.checked_div_exact(right),
        "divide_rounded" => left.divide_rounded(right, scale),
        _ => return Err("Format".to_owned()),
    })?;
    rendered(value, scale)
}

fn quantize(input: &Value) -> Outcome {
    let value: ExactDecimal = parse(input, "value")?;
    let scale = number(input, "scale")?;
    let result = checked(match text(input, "mode")? {
        "truncate" => value.truncate(scale),
        "round_away" => value.round_away(scale),
        _ => return Err("Format".to_owned()),
    })?;
    rendered(result, scale)
}

fn measurement(input: &Value) -> Outcome {
    let delta = checked(decimal::unit_measurement(
        parse(input, "contracted")?,
        parse(input, "previous")?,
        parse(input, "current")?,
        parse(input, "unit_price")?,
    ))?;
    Ok(json!({
        "quantity": checked(delta.quantity.value().fixed_text(6))?,
        "amount": checked(delta.period_value.value().fixed_text(2))?,
    }))
}

fn telescopic(input: &Value) -> Outcome {
    let contracted: Quantity = parse(input, "contracted")?;
    let price: UnitPrice = parse(input, "unit_price")?;
    let accumulated = field(input, "accumulated")?
        .as_array()
        .ok_or_else(|| "Format".to_owned())?;
    let mut previous = checked(Quantity::new(ExactDecimal::ZERO))?;
    let mut total = ExactDecimal::ZERO;
    let mut periods = Vec::with_capacity(accumulated.len());
    for value in accumulated {
        let current: Quantity = typed(value)?;
        let delta = checked(decimal::unit_measurement(
            contracted, previous, current, price,
        ))?;
        periods.push(checked(delta.period_value.value().fixed_text(2))?);
        total = checked(total.checked_add(delta.period_value.value()))?;
        previous = current;
    }
    Ok(json!({"periods": periods, "total": checked(total.fixed_text(2))?}))
}

fn weights(input: &Value) -> Result<Vec<AllocationWeight>, String> {
    // Deserialize the domain input, including its string-only ExactDecimal.
    serde_json::from_value(field(input, "rows")?.clone()).map_err(|_| "Format".to_owned())
}

fn allocation_result(mut allocation: Allocation) -> Outcome {
    // Canonical comparison order is an adapter concern. The engine itself
    // allocates residuals using its original position/identifier contract.
    allocation.parts.sort_by(|a, b| a.id.cmp(&b.id));
    let mut parts = Vec::with_capacity(allocation.parts.len());
    for part in allocation.parts {
        parts.push(json!({
            "id": part.id,
            "value": checked(part.value.fixed_text(allocation.scale))?,
        }));
    }
    Ok(json!({
        "parts": parts,
        "total": checked(allocation.total.fixed_text(allocation.scale))?,
    }))
}

fn units(input: &Value) -> Outcome {
    let pairs = field(input, "pairs")?
        .as_array()
        .ok_or_else(|| "Format".to_owned())?;
    let quantity = checked(Quantity::new(ExactDecimal::ONE))?;
    let mut compatible = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let left: MeasureUnit = parse(pair, "left")?;
        let right: MeasureUnit = parse(pair, "right")?;
        let left = QuantityWithUnit {
            value: quantity,
            unit: left,
        };
        let right = QuantityWithUnit {
            value: quantity,
            unit: right,
        };
        compatible.push(match left.checked_add(right) {
            Ok(_) => true,
            Err(UnitError::Incompatible) => false,
            Err(error) => return Err(format!("{error:?}")),
        });
    }
    Ok(json!({"compatible": compatible}))
}

fn calendar_options(input: &Value) -> Result<(CalendarPolicy, Cadence), String> {
    let policy = match text(input, "policy")? {
        "civil" => CalendarPolicy::Civil,
        "fixed_days" => CalendarPolicy::FixedDays,
        _ => return Err("Format".to_owned()),
    };
    let cadence = match text(input, "cadence")? {
        "weekly" => Cadence::Weekly,
        "fortnightly" => Cadence::Fortnightly,
        "monthly" => Cadence::Monthly,
        "bimonthly" => Cadence::Bimonthly,
        "quarterly" => Cadence::Quarterly,
        "semiannual" => Cadence::Semiannual,
        "annual" => Cadence::Annual,
        _ => return Err("Format".to_owned()),
    };
    Ok((policy, cadence))
}

fn calendar_sequence(input: &Value) -> Outcome {
    let anchor: CivilDate = parse(input, "anchor")?;
    let (policy, cadence) = calendar_options(input)?;
    let periods = field(input, "periods")?
        .as_array()
        .ok_or_else(|| "Format".to_owned())?;
    let mut dates = Vec::with_capacity(periods.len());
    for period in periods {
        let period = period
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| "Format".to_owned())?;
        dates.push(checked(calendar::advance(anchor, policy, cadence, period))?.to_string());
    }
    Ok(json!({"dates": dates}))
}

fn pause_days(input: &Value) -> Outcome {
    let pauses: Vec<PauseInterval> = serde_json::from_value(field(input, "intervals")?.clone())
        .map_err(|_| "Format".to_owned())?;
    let as_of = match input.get("as_of") {
        None | Some(Value::Null) => None,
        Some(value) => Some(typed::<CivilDate>(value)?),
    };
    let result = checked(calendar::pause_days(&pauses, text(input, "front")?, as_of))?;
    Ok(json!({
        "closed_days": result.closed_days,
        "open_days": result.open_days,
        "total_days": result.total_days,
        "has_open_pause": result.has_open_pause,
    }))
}

fn dispatch(request: &Value) -> Outcome {
    let input = field(request, "inputs")?;
    match text(request, "operation")? {
        "typed_parse" => typed_parse(input),
        "arithmetic" => arithmetic(input),
        "quantize" => quantize(input),
        "negotiated_price" => {
            let result = checked(decimal::negotiated_unit_price(
                parse(input, "cost")?,
                parse(input, "bdi")?,
                parse(input, "discount")?,
            ))?;
            rendered(result.value(), 6)
        }
        "unit_measurement" => measurement(input),
        "telescopic" => telescopic(input),
        "allocate_money" => allocation_result(checked(decimal::allocate_money(
            parse(input, "total")?,
            &weights(input)?,
        ))?),
        "allocate_percentages" => {
            allocation_result(checked(decimal::allocate_percentages(&weights(input)?))?)
        }
        "unit_compatibility" => units(input),
        "calendar_sequence" => calendar_sequence(input),
        "schedule" => {
            let (policy, cadence) = calendar_options(input)?;
            let periods = checked(calendar::schedule(
                parse(input, "anchor")?,
                parse(input, "formal_end")?,
                policy,
                cadence,
            ))?;
            Ok(json!({"periods": periods}))
        }
        "execution_window" => {
            let result = checked(calendar::execution_window(
                parse(input, "start")?,
                number(input, "days")?,
                parse(input, "valid_from")?,
                parse(input, "valid_to")?,
            ))?;
            Ok(json!({"end": result.end, "within_validity": result.within_validity}))
        }
        "pause_days" => pause_days(input),
        "utc_to_local" => Ok(json!({"value": checked(calendar::utc_to_local(
            parse(input, "instant")?,
            parse(input, "zone")?,
        ))?})),
        "local_to_utc" => Ok(json!({"value": checked(calendar::local_to_utc(
            text(input, "local")?,
            parse(input, "zone")?,
        ))?.to_string()})),
        "global_measurement" => {
            let result = checked(decimal::global_measurement(
                parse(input, "stage")?,
                parse(input, "previous_percent")?,
                parse(input, "current_percent")?,
            ))?;
            Ok(json!({"amount": checked(result.period_value.value().fixed_text(2))?}))
        }
        "adjustment" => {
            let result = checked(decimal::adjustment(
                parse(input, "eligible")?,
                parse(input, "index_base")?,
                parse(input, "index_current")?,
            ))?;
            Ok(json!({"adjustment": checked(result.amount.value().fixed_text(2))?}))
        }
        _ => Err("UnknownOperation".to_owned()),
    }
}

/// Execute one request. The adapter never reads the `expected` field.
pub fn run_case(request: &Value) -> Value {
    match dispatch(request) {
        Ok(result) => result,
        Err(error) => json!({"error": error}),
    }
}

/// Execute a single JSON request through the same adapter in the browser.
#[cfg_attr(feature = "wasm", wasm_bindgen::prelude::wasm_bindgen)]
pub fn verify_calculation_vector(request_json: &str) -> String {
    match serde_json::from_str::<Value>(request_json) {
        Ok(request) => run_case(&request).to_string(),
        Err(_) => json!({"error": "Format"}).to_string(),
    }
}

/// Execute every determinant and finite property sample in a reference-shaped
/// document. Expected results and explanatory metadata are deliberately ignored.
pub fn verify_document(input: &str) -> String {
    let document = match serde_json::from_str::<Value>(input) {
        Ok(document) => document,
        Err(_) => return json!({"error": "Format"}).to_string(),
    };
    let Some(vectors) = document.get("vectors").and_then(Value::as_array) else {
        return json!({"error": "Format"}).to_string();
    };
    let mut results = Vec::new();
    for request in vectors.iter().chain(
        document
            .get("property_samples")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("samples").and_then(Value::as_array))
            .flatten(),
    ) {
        let Some(id) = request.get("id").and_then(Value::as_str) else {
            return json!({"error": "Format"}).to_string();
        };
        results.push(json!({"id": id, "result": run_case(request)}));
    }
    Value::Array(results).to_string()
}
