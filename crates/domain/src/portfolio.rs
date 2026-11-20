//! Carteira e distribuição territorial com soma decimal exata.
use crate::{decimal::ExactDecimal, parties::valid_uf};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetOwnership {
    Owned,
    Leased,
    ThirdParty,
}
impl AssetOwnership {
    pub fn label(self) -> &'static str {
        match self {
            Self::Owned => "Próprio",
            Self::Leased => "Locado",
            Self::ThirdParty => "De terceiro",
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    Planned,
    Active,
    Paused,
    Completed,
}
impl WorkStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Planned => "Planejada",
            Self::Active => "Em execução",
            Self::Paused => "Paralisada",
            Self::Completed => "Concluída",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalityAllocation {
    pub municipality: String,
    pub uf: String,
    pub percentage: ExactDecimal,
    pub dimension: ExactDecimal,
}
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkInput {
    pub title: String,
    pub development: String,
    pub asset_name: String,
    pub ownership: AssetOwnership,
    pub cost_center: String,
    pub external_reference: String,
    pub status: WorkStatus,
    pub address: String,
    pub latitude: Option<ExactDecimal>,
    pub longitude: Option<ExactDecimal>,
    pub section: String,
    pub dimension: ExactDecimal,
    pub unit: String,
    pub localities: Vec<LocalityAllocation>,
    #[serde(default)]
    pub contract_ids: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PortfolioError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("O rateio precisa fechar 100%. Diferença: {0} pontos percentuais.")]
    Percentage(ExactDecimal),
    #[error("O rateio precisa fechar a dimensão da obra. Diferença: {0}.")]
    Dimension(ExactDecimal),
}
fn plain(value: &str, maximum: usize) -> bool {
    value.chars().count() <= maximum && !value.chars().any(char::is_control)
}
impl WorkInput {
    pub fn validated(mut self) -> Result<Self, PortfolioError> {
        self.title = self.title.trim().to_owned();
        if self.title.chars().count() < 2 || !plain(&self.title, 160) {
            return Err(PortfolioError::Invalid(
                "Informe um título com 2 a 160 caracteres.",
            ));
        }
        for field in [
            &mut self.development,
            &mut self.asset_name,
            &mut self.cost_center,
            &mut self.external_reference,
            &mut self.section,
        ] {
            *field = field.trim().to_owned();
            if !plain(field, 160) {
                return Err(PortfolioError::Invalid(
                    "Os campos de identificação aceitam até 160 caracteres.",
                ));
            }
        }
        self.address = self.address.trim().to_owned();
        if !plain(&self.address, 500) {
            return Err(PortfolioError::Invalid(
                "O endereço aceita até 500 caracteres.",
            ));
        }
        match (self.latitude, self.longitude) {
            (Some(lat), Some(lon)) => {
                if lat < "-90".parse().unwrap()
                    || lat > "90".parse().unwrap()
                    || lon < "-180".parse().unwrap()
                    || lon > "180".parse().unwrap()
                    || lat.scale() > 8
                    || lon.scale() > 8
                {
                    return Err(PortfolioError::Invalid(
                        "Latitude deve estar entre -90 e 90, longitude entre -180 e 180, com até oito casas decimais.",
                    ));
                }
            }
            (None, None) => {}
            _ => {
                return Err(PortfolioError::Invalid(
                    "Informe latitude e longitude juntas, ou deixe ambas vazias.",
                ));
            }
        }
        if self.dimension.is_negative()
            || self.dimension.scale() > 6
            || self.dimension > "1000000000000".parse().unwrap()
        {
            return Err(PortfolioError::Invalid(
                "Informe uma dimensão não negativa, até um trilhão, com até seis casas decimais.",
            ));
        }
        if !matches!(self.unit.as_str(), "m" | "m2" | "m3" | "un" | "km" | "ha") {
            return Err(PortfolioError::Invalid(
                "Selecione a unidade da dimensão da obra.",
            ));
        }
        if self.localities.is_empty() || self.localities.len() > 100 {
            return Err(PortfolioError::Invalid(
                "Informe entre um e cem municípios no rateio territorial.",
            ));
        }
        let mut names = BTreeSet::new();
        let mut percentage = ExactDecimal::ZERO;
        let mut dimension = ExactDecimal::ZERO;
        let one_hundred: ExactDecimal = "100".parse().unwrap();
        for locality in &mut self.localities {
            locality.municipality = locality.municipality.trim().to_owned();
            locality.uf = locality.uf.trim().to_ascii_uppercase();
            if locality.municipality.chars().count() < 2
                || !plain(&locality.municipality, 120)
                || !valid_uf(&locality.uf)
                || !names.insert((locality.municipality.to_uppercase(), locality.uf.clone()))
            {
                return Err(PortfolioError::Invalid(
                    "Confira município e UF. Cada município deve aparecer somente uma vez.",
                ));
            }
            if locality.percentage.is_negative()
                || locality.percentage > one_hundred
                || locality.percentage.scale() > 6
                || locality.dimension.is_negative()
                || locality.dimension.scale() > 6
            {
                return Err(PortfolioError::Invalid(
                    "Percentuais e dimensões devem ser não negativos e ter até seis casas decimais.",
                ));
            }
            percentage = percentage
                .checked_add(locality.percentage)
                .map_err(|_| PortfolioError::Invalid("O rateio ultrapassou o limite numérico."))?;
            dimension = dimension.checked_add(locality.dimension).map_err(|_| {
                PortfolioError::Invalid("A dimensão ultrapassou o limite numérico.")
            })?;
        }
        if percentage != one_hundred {
            return Err(PortfolioError::Percentage(
                one_hundred
                    .checked_sub(percentage)
                    .map_err(|_| PortfolioError::Invalid("Confira os percentuais."))?,
            ));
        }
        if dimension != self.dimension {
            return Err(PortfolioError::Dimension(
                self.dimension
                    .checked_sub(dimension)
                    .map_err(|_| PortfolioError::Invalid("Confira as dimensões."))?,
            ));
        }
        let ids: BTreeSet<_> = self.contract_ids.iter().collect();
        if ids.len() != self.contract_ids.len() || ids.len() > 100 {
            return Err(PortfolioError::Invalid(
                "Mantenha até cem vínculos de contratos, sem repetições.",
            ));
        }
        for id in &self.contract_ids {
            if id.parse::<crate::EntityId>().is_err() {
                return Err(PortfolioError::Invalid(
                    "Selecione um contrato válido da mesma empresa.",
                ));
            }
        }
        self.contract_ids.sort();
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn work() -> WorkInput {
        WorkInput {
            title: "Obra fictícia".into(),
            development: String::new(),
            asset_name: "Edifício de teste".into(),
            ownership: AssetOwnership::Owned,
            cost_center: String::new(),
            external_reference: String::new(),
            status: WorkStatus::Planned,
            address: String::new(),
            latitude: None,
            longitude: None,
            section: String::new(),
            dimension: "10".parse().unwrap(),
            unit: "km".into(),
            localities: vec![
                LocalityAllocation {
                    municipality: "Florianópolis".into(),
                    uf: "SC".into(),
                    percentage: "33.333333".parse().unwrap(),
                    dimension: "3.333333".parse().unwrap(),
                },
                LocalityAllocation {
                    municipality: "São José".into(),
                    uf: "SC".into(),
                    percentage: "66.666667".parse().unwrap(),
                    dimension: "6.666667".parse().unwrap(),
                },
            ],
            contract_ids: vec![],
        }
    }
    #[test]
    fn allocation_conserves_exact_totals() {
        let v = work().validated().unwrap();
        assert_eq!(v.dimension.to_string(), "10");
        assert_eq!(v.latitude, None);
    }
    #[test]
    fn missing_percentage_is_reported() {
        let mut v = work();
        v.localities[1].percentage = "60".parse().unwrap();
        assert_eq!(
            v.validated().unwrap_err(),
            PortfolioError::Percentage("6.666667".parse().unwrap())
        );
    }
    #[test]
    fn wrong_dimension_is_reported() {
        let mut v = work();
        v.localities[1].dimension = "6".parse().unwrap();
        assert_eq!(
            v.validated().unwrap_err(),
            PortfolioError::Dimension("0.666667".parse().unwrap())
        );
    }
    #[test]
    fn invalid_and_incomplete_coordinates_fail() {
        let mut v = work();
        v.latitude = Some("-90.00000001".parse().unwrap());
        v.longitude = Some("0".parse().unwrap());
        assert!(v.validated().is_err());
        let mut v = work();
        v.latitude = Some(ExactDecimal::ZERO);
        assert!(v.validated().is_err());
    }
    #[test]
    fn duplicate_contracts_do_not_duplicate_work() {
        let mut v = work();
        v.contract_ids = vec!["f1e71794-6c29-4ea1-b630-e8182c9d97d3".into(); 2];
        assert!(v.validated().is_err());
    }
}
