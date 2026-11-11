use sqlx::{ConnectOptions, postgres::PgConnectOptions};
use std::{fmt, str::FromStr, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseEnvironment {
    Development,
    Homologation,
    Test,
}
impl DatabaseEnvironment {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Homologation => "homologation",
            Self::Test => "test",
        }
    }
}
impl FromStr for DatabaseEnvironment {
    type Err = InvalidDatabaseSettings;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "development" => Ok(Self::Development),
            "homologation" => Ok(Self::Homologation),
            "test" => Ok(Self::Test),
            _ => Err(InvalidDatabaseSettings),
        }
    }
}

/// Configuração exclusiva do servidor. Não implementa Serialize.
#[derive(Clone)]
pub struct DatabaseSettings(PgConnectOptions);
impl DatabaseSettings {
    pub fn from_external_url(value: &str) -> Result<Self, InvalidDatabaseSettings> {
        if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
            return Err(InvalidDatabaseSettings);
        }
        let options = PgConnectOptions::from_str(value).map_err(|_| InvalidDatabaseSettings)?;
        if options.get_database().is_none_or(str::is_empty) || options.get_username().is_empty() {
            return Err(InvalidDatabaseSettings);
        }
        Ok(Self(options.disable_statement_logging()))
    }
    pub(crate) fn options(&self) -> PgConnectOptions {
        self.0.clone()
    }
    pub(crate) fn username(&self) -> &str {
        self.0.get_username()
    }
    pub(crate) fn same_database(&self, other: &Self) -> bool {
        self.0.get_host() == other.0.get_host()
            && self.0.get_port() == other.0.get_port()
            && self.0.get_database() == other.0.get_database()
            && self.0.get_socket() == other.0.get_socket()
    }
}
impl fmt::Debug for DatabaseSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseSettings([redigido])")
    }
}
#[derive(Debug, thiserror::Error)]
#[error("Configuração externa do PostgreSQL inválida; confira as instruções do ambiente.")]
pub struct InvalidDatabaseSettings;

#[derive(Clone, Copy, Debug)]
pub struct TransactionLimits {
    pub(crate) max_connections: u32,
    pub(crate) acquire: Duration,
    pub(crate) statement: Duration,
    pub(crate) lock: Duration,
    pub(crate) transaction: Duration,
}
impl Default for TransactionLimits {
    fn default() -> Self {
        Self {
            max_connections: 10,
            acquire: Duration::from_secs(5),
            statement: Duration::from_secs(5),
            lock: Duration::from_secs(1),
            transaction: Duration::from_secs(15),
        }
    }
}
impl TransactionLimits {
    pub fn new(
        max_connections: u32,
        acquire_ms: u64,
        statement_ms: u64,
        lock_ms: u64,
        transaction_ms: u64,
    ) -> Result<Self, InvalidDatabaseSettings> {
        if !(1..=50).contains(&max_connections)
            || [acquire_ms, statement_ms, lock_ms, transaction_ms]
                .iter()
                .any(|v| !(10..=60_000).contains(v))
            || lock_ms > statement_ms
            || statement_ms > transaction_ms
        {
            return Err(InvalidDatabaseSettings);
        }
        Ok(Self {
            max_connections,
            acquire: Duration::from_millis(acquire_ms),
            statement: Duration::from_millis(statement_ms),
            lock: Duration::from_millis(lock_ms),
            transaction: Duration::from_millis(transaction_ms),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn database_configuration_never_prints_its_input() {
        let marker = ["fixture", "sensitive", "marker"].join("-");
        let value = format!("{}://worker:{marker}@localhost/canteiro_test", "postgres");
        let settings = DatabaseSettings::from_external_url(&value).unwrap();
        assert!(!format!("{settings:?}").contains(&marker));
        let invalid = format!("https://worker:{marker}@invalid");
        let error = DatabaseSettings::from_external_url(&invalid).unwrap_err();
        assert!(!error.to_string().contains(&marker));
        assert!(!format!("{error:?}").contains(&marker));
        assert!(DatabaseSettings::from_external_url("postgres://worker@localhost/").is_err());
    }
    #[test]
    fn limits_and_environments_reject_ambiguous_settings() {
        assert!("production".parse::<DatabaseEnvironment>().is_err());
        assert!(TransactionLimits::new(0, 10, 10, 10, 10).is_err());
        assert!(TransactionLimits::new(1, 10, 10, 20, 10).is_err());
        assert!(TransactionLimits::new(1, 10, 10, 10, 9).is_err());
        assert!(TransactionLimits::new(51, 10, 10, 10, 10).is_err());
    }
}
