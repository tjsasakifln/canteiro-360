use chrono::{DateTime, Datelike, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use std::{fmt, str::FromStr};
use uuid::Uuid;

/// Falha de validação sem conservar a entrada, documentos ou identificadores pessoais.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum InvalidValue {
    #[error("Identificador inválido.")]
    Identifier,
    #[error("Data inválida. Informe uma data entre os anos 0001 e 9999.")]
    CivilDate,
    #[error("Instante inválido. Use fuso explícito e precisão de até microssegundos.")]
    Instant,
    #[error("Versão inválida. Use um inteiro positivo na faixa admitida.")]
    Version,
    #[error("A versão atingiu o limite admitido.")]
    VersionOverflow,
}

macro_rules! identifier {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Uuid);

        impl $name {
            pub fn from_uuid(value: Uuid) -> Result<Self, InvalidValue> {
                if value.is_nil() || value.as_u128() == u128::MAX {
                    return Err(InvalidValue::Identifier);
                }
                Ok(Self(value))
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl TryFrom<Uuid> for $name {
            type Error = InvalidValue;

            fn try_from(value: Uuid) -> Result<Self, Self::Error> {
                Self::from_uuid(value)
            }
        }

        impl FromStr for $name {
            type Err = InvalidValue;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map_err(|_| InvalidValue::Identifier)
                    .and_then(Self::from_uuid)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.hyphenated().fmt(f)
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

identifier!(
    TenantId,
    "Identificador de organização. Não constitui autorização de acesso."
);
identifier!(
    EntityId,
    "Identificador opaco de entidade, sempre associado a uma organização."
);
identifier!(
    ActorId,
    "Identificador de ator. A autenticação e a concessão de acesso são externas ao tipo."
);
identifier!(
    OperationId,
    "Identificador de operação utilizado para correlação e recuperação."
);

/// Data civil sem horário nem fuso. Não convertê-la em um instante à meia-noite.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CivilDate(NaiveDate);

impl CivilDate {
    pub fn from_naive_date(value: NaiveDate) -> Result<Self, InvalidValue> {
        if !(1..=9999).contains(&value.year()) {
            return Err(InvalidValue::CivilDate);
        }
        Ok(Self(value))
    }

    pub fn as_naive_date(self) -> NaiveDate {
        self.0
    }
}

impl FromStr for CivilDate {
    type Err = InvalidValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 10
            || value.as_bytes()[4] != b'-'
            || value.as_bytes()[7] != b'-'
            || !value.bytes().enumerate().all(|(index, byte)| {
                if index == 4 || index == 7 {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit()
                }
            })
        {
            return Err(InvalidValue::CivilDate);
        }
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| InvalidValue::CivilDate)
            .and_then(Self::from_naive_date)
    }
}

impl fmt::Display for CivilDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.format("%Y-%m-%d").fmt(f)
    }
}

impl Serialize for CivilDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for CivilDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Instante UTC compatível com `TIMESTAMPTZ(6)`, sem truncagem implícita.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UtcInstant(DateTime<Utc>);

impl UtcInstant {
    pub fn from_datetime(value: DateTime<Utc>) -> Result<Self, InvalidValue> {
        let nanoseconds = value.timestamp_subsec_nanos();
        if !(1..=9999).contains(&value.year())
            || nanoseconds >= 1_000_000_000
            || !nanoseconds.is_multiple_of(1_000)
        {
            return Err(InvalidValue::Instant);
        }
        Ok(Self(value))
    }

    pub fn as_datetime(self) -> DateTime<Utc> {
        self.0
    }
}

impl FromStr for UtcInstant {
    type Err = InvalidValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // O parser pode aceitar precisão maior que a representável. Recusar antes
        // de converter impede que dígitos além dos nanossegundos sejam descartados.
        if let Some((_, fraction)) = value.split_once('.') {
            let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
            if !(1..=6).contains(&digits) {
                return Err(InvalidValue::Instant);
            }
        }
        DateTime::parse_from_rfc3339(value)
            .map_err(|_| InvalidValue::Instant)
            .and_then(|instant| Self::from_datetime(instant.with_timezone(&Utc)))
    }
}

impl fmt::Display for UtcInstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.to_rfc3339_opts(SecondsFormat::Micros, true).fmt(f)
    }
}

impl Serialize for UtcInstant {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for UtcInstant {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

/// Versão para controle otimista. Texto no transporte preserva todos os 63 bits positivos.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntityVersion(i64);

impl EntityVersion {
    pub const INITIAL: Self = Self(1);

    pub fn new(value: i64) -> Result<Self, InvalidValue> {
        if value <= 0 {
            return Err(InvalidValue::Version);
        }
        Ok(Self(value))
    }

    pub fn get(self) -> i64 {
        self.0
    }

    pub fn next(self) -> Result<Self, InvalidValue> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(InvalidValue::VersionOverflow)
    }
}

impl FromStr for EntityVersion {
    type Err = InvalidValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || value.starts_with('0')
            || !value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(InvalidValue::Version);
        }
        value
            .parse()
            .map_err(|_| InvalidValue::Version)
            .and_then(Self::new)
    }
}

impl fmt::Display for EntityVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Serialize for EntityVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for EntityVersion {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "a1111111-1111-4111-8111-111111111111";

    #[test]
    fn identifiers_have_distinct_types_and_validate_deserialization() {
        let id = ID.parse::<TenantId>().unwrap();
        assert_eq!(serde_json::to_string(&id).unwrap(), format!("\"{ID}\""));
        assert_eq!(
            serde_json::from_str::<TenantId>(&format!("\"{ID}\"")).unwrap(),
            id
        );
        assert_eq!(
            ID.to_uppercase().parse::<EntityId>().unwrap().to_string(),
            ID
        );
        assert_eq!(ID.parse::<ActorId>().unwrap().as_uuid(), id.as_uuid());
        assert_eq!(
            OperationId::from_uuid(id.as_uuid()).unwrap().to_string(),
            ID
        );
        for invalid in [
            "00000000-0000-0000-0000-000000000000",
            "ffffffff-ffff-ffff-ffff-ffffffffffff",
            "not-an-id",
        ] {
            assert!(invalid.parse::<TenantId>().is_err());
            assert!(invalid.parse::<EntityId>().is_err());
            assert!(invalid.parse::<ActorId>().is_err());
            assert!(serde_json::from_str::<OperationId>(&format!("\"{invalid}\"")).is_err());
        }
        assert!(serde_json::from_str::<TenantId>("42").is_err());
    }

    #[test]
    fn civil_dates_keep_calendar_meaning_and_reject_impossible_or_ambiguous_dates() {
        for date in ["0001-01-01", "2024-02-29", "2026-11-11", "9999-12-31"] {
            let civil = date.parse::<CivilDate>().unwrap();
            let serialized = serde_json::to_string(&civil).unwrap();
            assert_eq!(serialized, format!("\"{date}\""));
            assert_eq!(
                serde_json::from_str::<CivilDate>(&serialized).unwrap(),
                civil
            );
        }
        for date in [
            "0000-01-01",
            "2023-02-29",
            "2026-13-01",
            "2026-1-01",
            "11/11/2026",
            "10000-01-01",
            "2026-11-11T00:00:00Z",
        ] {
            assert!(date.parse::<CivilDate>().is_err(), "{date}");
        }
        assert!(CivilDate::from_naive_date(NaiveDate::from_ymd_opt(0, 1, 1).unwrap()).is_err());
        assert!(serde_json::from_str::<CivilDate>("\"2023-02-29\"").is_err());
    }

    #[test]
    fn instants_normalize_offsets_without_losing_microseconds() {
        let instant = "2026-11-11T23:30:00.123456-03:00"
            .parse::<UtcInstant>()
            .unwrap();
        assert_eq!(instant.to_string(), "2026-11-12T02:30:00.123456Z");
        let serialized = serde_json::to_string(&instant).unwrap();
        assert_eq!(
            serde_json::from_str::<UtcInstant>(&serialized).unwrap(),
            instant
        );
        assert_eq!(
            "2026-11-11T00:00:00Z"
                .parse::<UtcInstant>()
                .unwrap()
                .to_string(),
            "2026-11-11T00:00:00.000000Z"
        );
        for value in [
            "2026-11-11T00:00:00.123456001Z",
            "2026-11-11T00:00:00.1234560001Z",
            "2026-11-11T00:00:00.0000000009Z",
            "2026-11-11T00:00:00.0000000Z",
            "2026-11-11T00:00:00",
            "2016-12-31T23:59:60Z",
            "0001-01-01T00:00:00+01:00",
            "9999-12-31T23:00:00-02:00",
        ] {
            assert!(value.parse::<UtcInstant>().is_err(), "{value}");
        }
        assert!(serde_json::from_str::<UtcInstant>("\"2026-11-11T00:00:00.000000001Z\"").is_err());
    }

    #[test]
    fn entity_versions_preserve_large_values_and_never_wrap() {
        assert_eq!(EntityVersion::INITIAL.next().unwrap().get(), 2);
        let max = EntityVersion::new(i64::MAX).unwrap();
        assert_eq!(max.next(), Err(InvalidValue::VersionOverflow));
        let serialized = serde_json::to_string(&max).unwrap();
        assert_eq!(serialized, "\"9223372036854775807\"");
        assert_eq!(
            serde_json::from_str::<EntityVersion>(&serialized).unwrap(),
            max
        );
        for invalid in ["0", "-1", "01", "+1", "1.0", " 1", "9223372036854775808"] {
            assert!(invalid.parse::<EntityVersion>().is_err(), "{invalid}");
        }
        assert!(EntityVersion::new(0).is_err());
        assert!(EntityVersion::new(-1).is_err());
        assert!(serde_json::from_str::<EntityVersion>("9007199254740993").is_err());
    }
}
