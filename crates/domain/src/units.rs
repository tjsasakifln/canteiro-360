//! Quantities carry their unit. Conversion requires a separate explicit rule.
use crate::decimal::{DecimalError, Quantity};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum MeasureUnit {
    #[serde(rename = "un")]
    Unit,
    #[serde(rename = "m")]
    Metre,
    #[serde(rename = "m²")]
    SquareMetre,
    #[serde(rename = "m³")]
    CubicMetre,
    #[serde(rename = "kg")]
    Kilogram,
    #[serde(rename = "t")]
    Tonne,
    #[serde(rename = "L")]
    Litre,
    #[serde(rename = "h")]
    Hour,
}

impl MeasureUnit {
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Unit => "un",
            Self::Metre => "m",
            Self::SquareMetre => "m²",
            Self::CubicMetre => "m³",
            Self::Kilogram => "kg",
            Self::Tonne => "t",
            Self::Litre => "L",
            Self::Hour => "h",
        }
    }
}

impl FromStr for MeasureUnit {
    type Err = UnitError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "un" => Ok(Self::Unit),
            "m" => Ok(Self::Metre),
            "m²" => Ok(Self::SquareMetre),
            "m³" => Ok(Self::CubicMetre),
            "kg" => Ok(Self::Kilogram),
            "t" => Ok(Self::Tonne),
            "L" => Ok(Self::Litre),
            "h" => Ok(Self::Hour),
            _ => Err(UnitError::Unknown),
        }
    }
}

impl fmt::Display for MeasureUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum UnitError {
    #[error("Informe uma unidade conhecida para a quantidade.")]
    Unknown,
    #[error(
        "As unidades são diferentes. Converta a quantidade explicitamente antes de combinar os valores."
    )]
    Incompatible,
    #[error(transparent)]
    Decimal(#[from] DecimalError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuantityWithUnit {
    pub value: Quantity,
    pub unit: MeasureUnit,
}

impl QuantityWithUnit {
    pub fn checked_add(self, other: Self) -> Result<Self, UnitError> {
        self.require_same_unit(other)?;
        Ok(Self {
            value: Quantity::new(self.value.value().checked_add(other.value.value())?)?,
            unit: self.unit,
        })
    }

    pub fn checked_sub(self, other: Self) -> Result<Self, UnitError> {
        self.require_same_unit(other)?;
        Ok(Self {
            value: Quantity::new(self.value.value().checked_sub(other.value.value())?)?,
            unit: self.unit,
        })
    }

    fn require_same_unit(self, other: Self) -> Result<(), UnitError> {
        if self.unit == other.unit {
            Ok(())
        } else {
            Err(UnitError::Incompatible)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantities_require_units_and_never_convert_them_implicitly() {
        let first = QuantityWithUnit {
            value: "1.234567".parse().unwrap(),
            unit: MeasureUnit::Metre,
        };
        let second = QuantityWithUnit {
            value: "2.000001".parse().unwrap(),
            unit: MeasureUnit::Metre,
        };
        assert_eq!(
            first.checked_add(second).unwrap().value.to_string(),
            "3.234568"
        );
        assert_eq!(
            first.checked_add(QuantityWithUnit {
                unit: MeasureUnit::SquareMetre,
                ..second
            }),
            Err(UnitError::Incompatible)
        );
        assert!(first.checked_sub(second).is_err());
        assert!(serde_json::from_str::<QuantityWithUnit>(r#"{"value":"1"}"#).is_err());
        assert!(serde_json::from_str::<QuantityWithUnit>(r#"{"value":1,"unit":"m"}"#).is_err());
        assert!(serde_json::from_str::<QuantityWithUnit>(r#"{"value":"1","unit":"m2"}"#).is_err());
        let encoded = serde_json::to_string(&first).unwrap();
        assert_eq!(
            serde_json::from_str::<QuantityWithUnit>(&encoded).unwrap(),
            first
        );
    }
}
