//! Exact arithmetic for the versioned C360-DECIMAL-1 policy.
//!
//! `rust_decimal::Decimal` is the value representation. Integer scratch arithmetic
//! proves that an operation is exact before constructing its result. In particular,
//! `checked_mul`/`checked_div` from a decimal library are not sufficient: they may
//! fit a result by reducing its scale. No such reduction is permitted here.

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::{collections::BTreeSet, fmt, str::FromStr};

pub const POLICY_ID: &str = "C360-DECIMAL-1";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum CalculationPolicy {
    #[default]
    #[serde(rename = "C360-DECIMAL-1")]
    C360Decimal1,
}

impl CalculationPolicy {
    pub fn require(value: &str) -> Result<Self, DecimalError> {
        if value == POLICY_ID {
            Ok(Self::C360Decimal1)
        } else {
            Err(DecimalError::UnknownPolicy)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DecimalError {
    #[error("Informe um decimal canônico com ponto e sem separador de milhar.")]
    Format,
    #[error("O valor tem mais casas decimais que o campo permite.")]
    Scale,
    #[error("O valor ou o resultado ultrapassa o limite permitido.")]
    Overflow,
    #[error("O cálculo perderia precisão fora das etapas de arredondamento permitidas.")]
    Precision,
    #[error("Não é possível dividir por zero.")]
    DivisionByZero,
    #[error("Este valor deve ser igual ou maior que zero.")]
    Negative,
    #[error("O valor está fora da faixa permitida para esta operação.")]
    Range,
    #[error("O acumulado ultrapassa a quantidade contratada.")]
    InsufficientBalance,
    #[error("A redução do acumulado exige uma correção aprovada em fluxo próprio.")]
    Decreasing,
    #[error(
        "O valor anterior não corresponde ao acumulado e ao preço do segmento. Confira sua versão."
    )]
    InconsistentPrevious,
    #[error("Informe entre uma e dez mil parcelas para o rateio.")]
    EmptyAllocation,
    #[error("Cada parcela do rateio deve ter um identificador diferente.")]
    DuplicateIdentifier,
    #[error("A soma dos pesos do rateio deve ser maior que zero.")]
    ZeroWeight,
    #[error("Informe um identificador estável para cada parcela do rateio.")]
    InvalidIdentifier,
    #[error(
        "Esta política de cálculo não é suportada. Preserve a versão original e confira a migração."
    )]
    UnknownPolicy,
}

/// A checked decimal with at most 28 significant digits and at most 28 places.
/// Its constructor, operations and deserializer all preserve the same invariant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactDecimal(Decimal);

impl ExactDecimal {
    pub const ZERO: Self = Self(Decimal::ZERO);
    pub const ONE: Self = Self(Decimal::ONE);

    fn from_parts(mut coefficient: BigInt, mut scale: u32) -> Result<Self, DecimalError> {
        if coefficient.is_zero() {
            return Ok(Self::ZERO);
        }
        while scale > 0 && (&coefficient % 10_u8).is_zero() {
            coefficient /= 10_u8;
            scale -= 1;
        }
        if scale > 28 {
            return Err(DecimalError::Precision);
        }
        let digits = coefficient.abs().to_string().len();
        if digits > 28 {
            return Err(if digits.saturating_sub(scale as usize) > 28 {
                DecimalError::Overflow
            } else {
                DecimalError::Precision
            });
        }
        let coefficient = coefficient.to_i128().ok_or(DecimalError::Overflow)?;
        // The checked 28-digit coefficient is strictly inside Decimal's 96-bit
        // mantissa and scale <= 28. This constructor cannot round or overflow.
        Ok(Self(Decimal::from_i128_with_scale(coefficient, scale)))
    }

    pub fn as_decimal(self) -> Decimal {
        self.0
    }

    pub fn scale(self) -> u32 {
        self.0.scale()
    }

    pub fn is_negative(self) -> bool {
        self.0.is_sign_negative() && !self.0.is_zero()
    }

    pub fn is_zero(self) -> bool {
        self.0.is_zero()
    }

    fn coefficient(self) -> BigInt {
        BigInt::from(self.0.mantissa())
    }

    fn aligned(self, scale: u32) -> BigInt {
        self.coefficient() * ten(scale - self.scale())
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, DecimalError> {
        let scale = self.scale().max(rhs.scale());
        Self::from_parts(self.aligned(scale) + rhs.aligned(scale), scale)
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, DecimalError> {
        let scale = self.scale().max(rhs.scale());
        Self::from_parts(self.aligned(scale) - rhs.aligned(scale), scale)
    }

    pub fn checked_mul(self, rhs: Self) -> Result<Self, DecimalError> {
        Self::from_parts(
            self.coefficient() * rhs.coefficient(),
            self.scale() + rhs.scale(),
        )
    }

    /// Division succeeds only if its exact finite result fits the policy.
    pub fn checked_div_exact(self, rhs: Self) -> Result<Self, DecimalError> {
        if rhs.is_zero() {
            return Err(DecimalError::DivisionByZero);
        }
        if self.is_zero() {
            return Ok(Self::ZERO);
        }
        let mut numerator = self.coefficient() * ten(rhs.scale());
        let mut denominator = rhs.coefficient() * ten(self.scale());
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = gcd(numerator.abs(), denominator.clone());
        numerator /= &divisor;
        denominator /= divisor;
        let mut twos = 0;
        let mut fives = 0;
        while (&denominator % 2_u8).is_zero() {
            denominator /= 2_u8;
            twos += 1;
        }
        while (&denominator % 5_u8).is_zero() {
            denominator /= 5_u8;
            fives += 1;
        }
        if denominator != BigInt::from(1_u8) {
            return Err(DecimalError::Precision);
        }
        let scale = twos.max(fives);
        if scale > 28 {
            return Err(DecimalError::Precision);
        }
        numerator *= BigInt::from(2_u8).pow(scale - twos);
        numerator *= BigInt::from(5_u8).pow(scale - fives);
        Self::from_parts(numerator, scale)
    }

    /// Explicit nearest rounding, with ties away from zero. Only the policy's
    /// designated factor/price steps call this primitive in financial operations.
    pub fn divide_rounded(self, rhs: Self, scale: u32) -> Result<Self, DecimalError> {
        if scale > 28 {
            return Err(DecimalError::Scale);
        }
        if rhs.is_zero() {
            return Err(DecimalError::DivisionByZero);
        }
        let numerator = self.coefficient() * ten(rhs.scale() + scale);
        let denominator = rhs.coefficient() * ten(self.scale());
        Self::from_parts(round_ratio(numerator, denominator), scale)
    }

    /// Discards fractional places in the direction of zero, including credits.
    pub fn truncate(self, scale: u32) -> Result<Self, DecimalError> {
        if scale > 28 {
            return Err(DecimalError::Scale);
        }
        if scale >= self.scale() {
            return Ok(self);
        }
        Self::from_parts(self.coefficient() / ten(self.scale() - scale), scale)
    }

    pub fn round_away(self, scale: u32) -> Result<Self, DecimalError> {
        if scale > 28 {
            return Err(DecimalError::Scale);
        }
        if scale >= self.scale() {
            return Ok(self);
        }
        Self::from_parts(
            round_ratio(self.coefficient(), ten(self.scale() - scale)),
            scale,
        )
    }

    /// Formats at the requested scale without rounding. An incompatible scale
    /// is an error, never an instruction to truncate a value for presentation.
    pub fn fixed_text(self, scale: u32) -> Result<String, DecimalError> {
        let coefficient = self.scaled_bigint(scale)?;
        let negative = coefficient.is_negative();
        let digits = coefficient.abs().to_string();
        let digits = format!("{:0>width$}", digits, width = scale as usize + 1);
        let split = digits.len() - scale as usize;
        let sign = if negative { "-" } else { "" };
        if scale == 0 {
            Ok(format!("{sign}{digits}"))
        } else {
            Ok(format!("{sign}{}.{}", &digits[..split], &digits[split..]))
        }
    }

    fn scaled_bigint(self, scale: u32) -> Result<BigInt, DecimalError> {
        if scale > 28 {
            return Err(DecimalError::Scale);
        }
        if scale >= self.scale() {
            Ok(self.coefficient() * ten(scale - self.scale()))
        } else {
            let divisor = ten(self.scale() - scale);
            let coefficient = self.coefficient();
            if !(&coefficient % &divisor).is_zero() {
                return Err(DecimalError::Precision);
            }
            Ok(coefficient / divisor)
        }
    }

    pub fn scaled_integer(self, scale: u32) -> Result<i128, DecimalError> {
        self.scaled_bigint(scale)?
            .to_i128()
            .ok_or(DecimalError::Overflow)
    }
}

impl FromStr for ExactDecimal {
    type Err = DecimalError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let scale = input_scale(value)?;
        if scale > 28 {
            return Err(DecimalError::Scale);
        }
        let coefficient = value
            .replace('.', "")
            .parse::<BigInt>()
            .map_err(|_| DecimalError::Format)?;
        if value.starts_with('-') && coefficient.is_zero() {
            return Err(DecimalError::Format);
        }
        Self::from_parts(coefficient, scale)
    }
}

impl fmt::Display for ExactDecimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for ExactDecimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ExactDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

fn input_scale(value: &str) -> Result<u32, DecimalError> {
    if value.is_empty() || value.len() > 90 {
        return Err(DecimalError::Format);
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    if integer.is_empty()
        || !integer.bytes().all(|c| c.is_ascii_digit())
        || (integer.len() > 1 && integer.starts_with('0'))
        || (unsigned.contains('.') && fraction.is_empty())
        || !fraction.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(DecimalError::Format);
    }
    Ok(fraction.len() as u32)
}

fn ten(exponent: u32) -> BigInt {
    BigInt::from(10_u8).pow(exponent)
}

fn gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
    while !b.is_zero() {
        let remainder = a % &b;
        a = b;
        b = remainder;
    }
    a
}

fn round_ratio(numerator: BigInt, denominator: BigInt) -> BigInt {
    let mut quotient = &numerator / &denominator;
    let remainder = &numerator % &denominator;
    if remainder.abs() * 2_u8 >= denominator.abs() {
        if numerator.is_negative() == denominator.is_negative() {
            quotient += 1_u8;
        } else {
            quotient -= 1_u8;
        }
    }
    quotient
}

fn validate_bound(value: ExactDecimal, precision: u32, scale: u32) -> Result<(), DecimalError> {
    if value.scale() > scale {
        return Err(DecimalError::Scale);
    }
    if value.coefficient().abs() * ten(scale - value.scale()) >= ten(precision) {
        return Err(DecimalError::Overflow);
    }
    Ok(())
}

macro_rules! bounded_decimal {
    ($name:ident, $precision:literal, $scale:literal, $display_scale:literal, $nonnegative:literal, $positive:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(ExactDecimal);

        impl $name {
            pub fn new(value: ExactDecimal) -> Result<Self, DecimalError> {
                validate_bound(value, $precision, $scale)?;
                if $nonnegative && value.is_negative() {
                    return Err(DecimalError::Negative);
                }
                if $positive && value.is_zero() {
                    return Err(DecimalError::Range);
                }
                Ok(Self(value))
            }

            pub fn value(self) -> ExactDecimal {
                self.0
            }
        }

        impl FromStr for $name {
            type Err = DecimalError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if input_scale(value)? > $scale {
                    return Err(DecimalError::Scale);
                }
                Self::new(value.parse()?)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let scale = self.0.scale().max($display_scale);
                f.write_str(&self.0.fixed_text(scale).map_err(|_| fmt::Error)?)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(D::Error::custom)
            }
        }
    };
}

// Money permits an explicitly signed correction; ordinary operations below
// independently reject negative obligations. Constructors cannot authorize one.
bounded_decimal!(Money, 20, 2, 2, false, false);
bounded_decimal!(Quantity, 20, 6, 0, true, false);
bounded_decimal!(UnitPrice, 20, 6, 6, true, false);
bounded_decimal!(Percent, 12, 6, 0, true, false);
bounded_decimal!(IndexValue, 20, 10, 10, true, true);
// A positive index ratio can round to zero at the policy's ten places.
// Indices themselves are strictly positive; their rounded factor is nonnegative.
bounded_decimal!(Factor, 20, 10, 10, true, false);

fn nonnegative(value: ExactDecimal) -> Result<(), DecimalError> {
    if value.is_negative() {
        Err(DecimalError::Negative)
    } else {
        Ok(())
    }
}

fn bounded_percent(value: Percent, maximum: u32) -> Result<ExactDecimal, DecimalError> {
    let value = value.value();
    if value > ExactDecimal(Decimal::from(maximum)) {
        return Err(DecimalError::Range);
    }
    Ok(value)
}

fn hundred() -> ExactDecimal {
    ExactDecimal(Decimal::from(100_u32))
}

/// BDI and discount are applied once to an explicitly classified direct cost.
/// A directly negotiated selling price must not be passed as a new direct cost.
pub fn negotiated_unit_price(
    cost: UnitPrice,
    bdi: Percent,
    discount: Percent,
) -> Result<UnitPrice, DecimalError> {
    let bdi = bounded_percent(bdi, 1000)?.checked_div_exact(hundred())?;
    let discount = bounded_percent(discount, 100)?.checked_div_exact(hundred())?;
    let gross_factor = ExactDecimal::ONE.checked_add(bdi)?;
    let net_factor = ExactDecimal::ONE.checked_sub(discount)?;
    UnitPrice::new(
        cost.value()
            .checked_mul(gross_factor)?
            .checked_mul(net_factor)?
            .round_away(6)?,
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MeasurementDelta {
    pub policy: CalculationPolicy,
    pub quantity: Quantity,
    pub previous_accumulated: Money,
    pub current_accumulated: Money,
    pub period_value: Money,
    pub remaining_quantity: Quantity,
    pub remaining_value: Money,
}

fn accumulated(quantity: Quantity, price: UnitPrice) -> Result<Money, DecimalError> {
    Money::new(quantity.value().checked_mul(price.value())?.truncate(2)?)
}

/// Pure preview for a single immutable economic segment, with a fixed price and
/// one common unit. It neither approves a correction nor persists a measurement.
pub fn unit_measurement(
    contracted: Quantity,
    previous: Quantity,
    current: Quantity,
    price: UnitPrice,
) -> Result<MeasurementDelta, DecimalError> {
    unit_measurement_with_previous(
        contracted,
        previous,
        current,
        price,
        accumulated(previous, price)?,
    )
}

/// A definitive consumer supplies the recognized previous amount from its locked
/// version. A different price/amount cannot silently reprice the previous facts.
pub fn unit_measurement_with_previous(
    contracted: Quantity,
    previous: Quantity,
    current: Quantity,
    price: UnitPrice,
    previous_recognized: Money,
) -> Result<MeasurementDelta, DecimalError> {
    if previous > contracted || current > contracted {
        return Err(DecimalError::InsufficientBalance);
    }
    if current < previous {
        return Err(DecimalError::Decreasing);
    }
    let previous_accumulated = accumulated(previous, price)?;
    if previous_accumulated != previous_recognized {
        return Err(DecimalError::InconsistentPrevious);
    }
    let current_accumulated = accumulated(current, price)?;
    let contracted_value = accumulated(contracted, price)?;
    Ok(MeasurementDelta {
        policy: CalculationPolicy::default(),
        quantity: Quantity::new(current.value().checked_sub(previous.value())?)?,
        previous_accumulated,
        current_accumulated,
        period_value: Money::new(
            current_accumulated
                .value()
                .checked_sub(previous_recognized.value())?,
        )?,
        remaining_quantity: Quantity::new(contracted.value().checked_sub(current.value())?)?,
        remaining_value: Money::new(
            contracted_value
                .value()
                .checked_sub(current_accumulated.value())?,
        )?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GlobalMeasurement {
    pub policy: CalculationPolicy,
    pub previous_accumulated: Money,
    pub current_accumulated: Money,
    pub period_value: Money,
}

pub fn global_measurement(
    stage: Money,
    previous: Percent,
    current: Percent,
) -> Result<GlobalMeasurement, DecimalError> {
    nonnegative(stage.value())?;
    let previous = bounded_percent(previous, 100)?;
    let current = bounded_percent(current, 100)?;
    if current < previous {
        return Err(DecimalError::Decreasing);
    }
    let previous_accumulated = Money::new(
        stage
            .value()
            .checked_mul(previous.checked_div_exact(hundred())?)?
            .truncate(2)?,
    )?;
    let current_accumulated = Money::new(
        stage
            .value()
            .checked_mul(current.checked_div_exact(hundred())?)?
            .truncate(2)?,
    )?;
    Ok(GlobalMeasurement {
        policy: CalculationPolicy::default(),
        previous_accumulated,
        current_accumulated,
        period_value: Money::new(
            current_accumulated
                .value()
                .checked_sub(previous_accumulated.value())?,
        )?,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Adjustment {
    pub policy: CalculationPolicy,
    pub factor: Factor,
    pub amount: Money,
}

/// The caller selects the contractual index cycle and whether a decrease is
/// authorized. Missing indices are errors at the caller, never a zero here.
pub fn adjustment(
    base: Money,
    index_base: IndexValue,
    index_current: IndexValue,
) -> Result<Adjustment, DecimalError> {
    nonnegative(base.value())?;
    let factor = Factor::new(
        index_current
            .value()
            .divide_rounded(index_base.value(), 10)?,
    )?;
    let amount = Money::new(
        base.value()
            .checked_mul(factor.value().checked_sub(ExactDecimal::ONE)?)?
            .truncate(2)?,
    )?;
    Ok(Adjustment {
        policy: CalculationPolicy::default(),
        factor,
        amount,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllocationWeight {
    pub id: String,
    pub position: u32,
    pub weight: ExactDecimal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AllocationPart {
    pub id: String,
    pub position: u32,
    pub value: ExactDecimal,
    pub received_residual: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Allocation {
    pub policy: CalculationPolicy,
    pub total: ExactDecimal,
    pub scale: u32,
    /// Sorted by the stable position and identifier, independent of query order.
    pub parts: Vec<AllocationPart>,
}

/// Exact largest remainders. Each integer residual unit is awarded once, in
/// descending remainder order, then stable position and identifier. Percentages
/// are an informational allocation, not another payable cost to add to money.
pub fn allocate(
    total: ExactDecimal,
    weights: &[AllocationWeight],
    scale: u32,
) -> Result<Allocation, DecimalError> {
    nonnegative(total)?;
    let units = total.scaled_bigint(scale)?;
    if weights.is_empty() || weights.len() > 10_000 {
        return Err(DecimalError::EmptyAllocation);
    }
    let mut ids = BTreeSet::new();
    let common_scale = weights
        .iter()
        .map(|row| row.weight.scale())
        .max()
        .unwrap_or(0);
    let mut denominator = BigInt::zero();
    for row in weights {
        if row.id.is_empty()
            || row.id.len() > 128
            || row.id.chars().any(char::is_control)
            || row.id.trim() != row.id
        {
            return Err(DecimalError::InvalidIdentifier);
        }
        if !ids.insert(row.id.as_str()) {
            return Err(DecimalError::DuplicateIdentifier);
        }
        nonnegative(row.weight)?;
        denominator += row.weight.aligned(common_scale);
    }
    // Check the exact sum once. Pairwise decimal reduction would make a valid
    // allocation depend on the incidental order of its input rows.
    ExactDecimal::from_parts(denominator.clone(), common_scale)?;
    if denominator.is_zero() {
        return Err(DecimalError::ZeroWeight);
    }
    let mut parts = Vec::with_capacity(weights.len());
    let mut used = BigInt::zero();
    for row in weights {
        let numerator = &units * row.weight.aligned(common_scale);
        let floor = &numerator / &denominator;
        let remainder = numerator % &denominator;
        used += &floor;
        parts.push((row, floor, remainder, false));
    }
    let residual = (units - used).to_usize().ok_or(DecimalError::Overflow)?;
    parts.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then(a.0.position.cmp(&b.0.position))
            .then(a.0.id.cmp(&b.0.id))
    });
    // floor quotas imply residual < n. No fallback is allowed to hide a bug.
    if residual >= parts.len() {
        return Err(DecimalError::Precision);
    }
    for part in parts.iter_mut().take(residual) {
        part.1 += 1_u8;
        part.3 = true;
    }
    parts.sort_by(|a, b| a.0.position.cmp(&b.0.position).then(a.0.id.cmp(&b.0.id)));
    let parts = parts
        .into_iter()
        .map(|(row, units, _, received_residual)| {
            Ok(AllocationPart {
                id: row.id.clone(),
                position: row.position,
                value: ExactDecimal::from_parts(units, scale)?,
                received_residual,
            })
        })
        .collect::<Result<Vec<_>, DecimalError>>()?;
    Ok(Allocation {
        policy: CalculationPolicy::default(),
        total,
        scale,
        parts,
    })
}

pub fn allocate_money(
    total: Money,
    weights: &[AllocationWeight],
) -> Result<Allocation, DecimalError> {
    allocate(total.value(), weights, 2)
}

pub fn allocate_percentages(weights: &[AllocationWeight]) -> Result<Allocation, DecimalError> {
    allocate(hundred(), weights, 2)
}
