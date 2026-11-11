//! Adaptador SQLx/PostgreSQL. Migrações e acesso transacional começam na issue 2.
//! Não há pool global, conexão automática nem acesso a dados nesta etapa.
use sqlx::postgres::PgConnectOptions;
use std::{fmt, str::FromStr};

/// Configuração exclusivamente do servidor, recebida de um cofre/ambiente externo.
/// O tipo não implementa Serialize e não expõe a credencial em Debug ou erro.
pub struct DatabaseSettings(PgConnectOptions);

impl DatabaseSettings {
    pub fn from_external_url(value: &str) -> Result<Self, InvalidDatabaseSettings> {
        if !value.starts_with("postgres://") && !value.starts_with("postgresql://") {
            return Err(InvalidDatabaseSettings);
        }
        PgConnectOptions::from_str(value)
            .map(Self)
            .map_err(|_| InvalidDatabaseSettings)
    }

    /// A integração da issue 2 usa estas opções sem reconstruir ou registrar a URL.
    pub fn connect_options(&self) -> &PgConnectOptions {
        &self.0
    }
}

impl fmt::Debug for DatabaseSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DatabaseSettings([redigido])")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("Configuração externa do PostgreSQL inválida; confira o valor no ambiente do servidor.")]
pub struct InvalidDatabaseSettings;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_configuration_never_prints_its_input() {
        // Construída em execução: somente um marcador fictício para a verificação.
        let marker = ["fixture", "sensitive", "marker"].join("-");
        let value = format!("{}://worker:{marker}@localhost/canteiro_test", "postgres");
        let settings = DatabaseSettings::from_external_url(&value).unwrap();
        assert!(!format!("{settings:?}").contains(&marker));
        let invalid = format!("https://worker:{marker}@invalid");
        let error = DatabaseSettings::from_external_url(&invalid).unwrap_err();
        assert!(!error.to_string().contains(&marker));
        assert!(!format!("{error:?}").contains(&marker));
    }
}
