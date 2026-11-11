use leptos::config::{Env, LeptosOptions};
use serde::Deserialize;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub environment: Environment,
    pub bind_addr: SocketAddr,
    pub site_root: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Homologation,
}

#[derive(Debug, thiserror::Error)]
#[error("Configuração inválida em {0}; confira as instruções de desenvolvimento.")]
pub struct ConfigError(&'static str);

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Só consulta valores públicos conhecidos. Credenciais não atravessam esta fronteira.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let source = match lookup("CANTEIRO_ENV").as_deref().unwrap_or("development") {
            "development" => include_str!("../../../config/development.toml"),
            "homologation" => include_str!("../../../config/homologation.toml"),
            _ => return Err(ConfigError("CANTEIRO_ENV")),
        };
        let mut config: Self = toml::from_str(source).map_err(|_| ConfigError("config/*.toml"))?;
        if let Some(value) = lookup("CANTEIRO_BIND_ADDR") {
            config.bind_addr = value
                .parse()
                .map_err(|_| ConfigError("CANTEIRO_BIND_ADDR"))?;
        }
        if let Some(value) = lookup("CANTEIRO_SITE_ROOT") {
            if value.is_empty() || value.contains(['\0', '\n', '\r']) {
                return Err(ConfigError("CANTEIRO_SITE_ROOT"));
            }
            config.site_root = value.into();
        }
        Ok(config)
    }

    pub fn leptos_options(&self) -> LeptosOptions {
        LeptosOptions::builder()
            .output_name("canteiro-web")
            .site_root(self.site_root.to_string_lossy().into_owned())
            .site_pkg_dir("pkg")
            .site_addr(self.bind_addr)
            // Dev também usa o artefato reproduzível, sem servidor de recarga ou Node.
            .env(Env::PROD)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environments_are_explicit_and_unknown_values_fail_closed() {
        let dev = Config::from_lookup(|_| None).unwrap();
        assert!(dev.bind_addr.ip().is_loopback());
        let hml = Config::from_lookup(|key| (key == "CANTEIRO_ENV").then(|| "homologation".into()))
            .unwrap();
        assert_eq!(hml.environment, Environment::Homologation);
        assert!(
            Config::from_lookup(|key| (key == "CANTEIRO_ENV").then(|| "production".into()))
                .is_err()
        );
        let err = Config::from_lookup(|key| {
            (key == "CANTEIRO_BIND_ADDR").then(|| "private-input-marker".into())
        })
        .unwrap_err();
        assert!(!err.to_string().contains("private-input-marker"));
    }

    #[test]
    fn configuration_does_not_read_external_credentials() {
        Config::from_lookup(|key| {
            assert!(!key.contains("DATABASE") && !key.contains("SECRET") && !key.contains("TOKEN"));
            None
        })
        .unwrap();
    }
}
