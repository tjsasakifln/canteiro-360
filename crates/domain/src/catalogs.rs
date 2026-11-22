//! Edições de catálogos são imutáveis e suas competências não são a data atual.
use crate::{
    CivilDate,
    decimal::{DecimalError, ExactDecimal, UnitPrice},
    units::MeasureUnit,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostCategory {
    Material,
    Labor,
    Equipment,
    Service,
    Transport,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostSource {
    pub title: String,
    pub organization: String,
    pub license: String,
    pub locality: String,
    pub tax_regime: String,
    /// Competência civil da fonte, não instante da carga.
    pub competence: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogItem {
    pub code: String,
    pub description: String,
    pub category: CostCategory,
    pub unit: MeasureUnit,
    pub price: UnitPrice,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEdition {
    pub catalog_id: String,
    pub edition: u32,
    pub published_on: CivilDate,
    pub source: CostSource,
    pub items: BTreeMap<String, CatalogItem>,
}

/// Diferenças calculadas apenas entre edições publicadas de uma mesma base.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogItemChange {
    pub before: CatalogItem,
    pub after: CatalogItem,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogComparison {
    pub catalog_id: String,
    pub before_edition: u32,
    pub after_edition: u32,
    pub before_source: CostSource,
    pub after_source: CostSource,
    pub added: Vec<CatalogItem>,
    pub removed: Vec<CatalogItem>,
    pub changed: Vec<CatalogItemChange>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitConversion {
    pub from: MeasureUnit,
    pub to: MeasureUnit,
    /// Coeficiente explícito aplicável ao valor; nunca inferido por símbolo.
    pub factor: ExactDecimal,
    pub reference: String,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CatalogError {
    #[error("Identifique a fonte, a licença, a localidade, o regime e a competência YYYY-MM.")]
    Source,
    #[error("Identifique o catálogo e a edição com versão positiva.")]
    Identity,
    #[error("Há item inválido, código duplicado ou preço não permitido.")]
    Item,
    #[error("Uma edição já publicada não pode ser substituída.")]
    ImmutableEdition,
    #[error("Selecione explicitamente uma edição existente.")]
    MissingEdition,
    #[error("Compare edições diferentes de uma mesma base de custos.")]
    IncompatibleComparison,

    #[error("As unidades são incompatíveis. Registre a conversão com referência.")]
    UnitMismatch,
    #[error("Coeficiente ou referência de conversão inválido.")]
    Conversion,
    #[error(transparent)]
    Decimal(#[from] DecimalError),
}
fn label(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.trim() == value
        && value.len() <= max
        && !value.chars().any(char::is_control)
}
impl CostSource {
    pub fn validate(&self) -> Result<(), CatalogError> {
        if !label(&self.title, 160)
            || !label(&self.organization, 160)
            || !label(&self.license, 300)
            || !label(&self.locality, 120)
            || !label(&self.tax_regime, 120)
            || self.competence.len() != 7
            || self.competence.as_bytes().get(4) != Some(&b'-')
            || !self.competence.as_bytes()[..4]
                .iter()
                .all(u8::is_ascii_digit)
            || !self.competence.as_bytes()[5..]
                .iter()
                .all(u8::is_ascii_digit)
            || !self.competence[..4]
                .parse::<u16>()
                .is_ok_and(|year| (1900..=9999).contains(&year))
            || !self.competence[5..]
                .parse::<u8>()
                .is_ok_and(|month| (1..=12).contains(&month))
        {
            return Err(CatalogError::Source);
        }
        Ok(())
    }
}
impl CatalogItem {
    pub fn validate(&self) -> Result<(), CatalogError> {
        if !label(&self.code, 80)
            || !label(&self.description, 800)
            || self.price.value().is_negative()
        {
            return Err(CatalogError::Item);
        }
        Ok(())
    }
}
impl CatalogEdition {
    /// Prévia integral sem persistência; uma linha inválida impede toda a edição.
    pub fn assemble(
        catalog_id: String,
        edition: u32,
        published_on: CivilDate,
        source: CostSource,
        rows: Vec<CatalogItem>,
    ) -> Result<Self, CatalogError> {
        if !label(&catalog_id, 128) || edition == 0 || rows.is_empty() || rows.len() > 50_000 {
            return Err(CatalogError::Identity);
        }
        source.validate()?;
        let mut items = BTreeMap::new();
        for row in rows {
            row.validate()?;
            if items.insert(row.code.clone(), row).is_some() {
                return Err(CatalogError::Item);
            }
        }
        Ok(Self {
            catalog_id,
            edition,
            published_on,
            source,
            items,
        })
    }
    pub fn get(&self, code: &str) -> Option<&CatalogItem> {
        self.items.get(code)
    }
    /// Compara códigos em ordem estável, sem atualizar referências históricas.
    /// Alterações de unidade ou categoria são sempre apresentadas, nunca convertidas
    /// implicitamente em preços equivalentes.
    pub fn compare_to(&self, other: &Self) -> Result<CatalogComparison, CatalogError> {
        if self.catalog_id != other.catalog_id || self.edition == other.edition {
            return Err(CatalogError::IncompatibleComparison);
        }
        let mut result = CatalogComparison {
            catalog_id: self.catalog_id.clone(),
            before_edition: self.edition,
            after_edition: other.edition,
            before_source: self.source.clone(),
            after_source: other.source.clone(),
            added: Vec::new(),
            removed: Vec::new(),
            changed: Vec::new(),
        };
        for (code, before) in &self.items {
            match other.items.get(code) {
                None => result.removed.push(before.clone()),
                Some(after) if before != after => result.changed.push(CatalogItemChange {
                    before: before.clone(),
                    after: after.clone(),
                }),
                Some(_) => {}
            }
        }
        for (code, after) in &other.items {
            if !self.items.contains_key(code) {
                result.added.push(after.clone());
            }
        }
        Ok(result)
    }

    pub fn outdated_on(&self, current_competence: &str) -> bool {
        self.source.competence.as_str() < current_competence
    }
}
#[derive(Clone, Debug, Default)]
pub struct CatalogLibrary {
    editions: BTreeMap<(String, u32), CatalogEdition>,
}
impl CatalogLibrary {
    pub fn publish(&mut self, edition: CatalogEdition) -> Result<(), CatalogError> {
        // Revalidate a deserialized edition, including map/code identity.
        let items: Vec<_> = edition.items.values().cloned().collect();
        let validated = CatalogEdition::assemble(
            edition.catalog_id.clone(),
            edition.edition,
            edition.published_on,
            edition.source.clone(),
            items,
        )?;
        if validated.items != edition.items {
            return Err(CatalogError::Item);
        }
        let key = (validated.catalog_id.clone(), validated.edition);
        if self.editions.contains_key(&key) {
            return Err(CatalogError::ImmutableEdition);
        }
        self.editions.insert(key, validated);
        Ok(())
    }
    pub fn edition(&self, id: &str, version: u32) -> Result<&CatalogEdition, CatalogError> {
        self.editions
            .get(&(id.to_owned(), version))
            .ok_or(CatalogError::MissingEdition)
    }
    pub fn editions_of(&self, id: &str) -> Vec<&CatalogEdition> {
        self.editions
            .iter()
            .filter(|((catalog, _), _)| catalog == id)
            .map(|(_, e)| e)
            .collect()
    }
}
impl UnitConversion {
    pub fn apply(
        &self,
        value: UnitPrice,
        from: MeasureUnit,
        to: MeasureUnit,
    ) -> Result<UnitPrice, CatalogError> {
        if from == to {
            return Ok(value);
        }
        if self.from != from || self.to != to {
            return Err(CatalogError::UnitMismatch);
        }
        if self.factor.is_zero() || self.factor.is_negative() || !label(&self.reference, 240) {
            return Err(CatalogError::Conversion);
        }
        Ok(UnitPrice::new(
            value.value().checked_mul(self.factor)?.round_away(6)?,
        )?)
    }
}
pub fn require_matching_units(
    given: MeasureUnit,
    expected: MeasureUnit,
    conversion: Option<&UnitConversion>,
    price: UnitPrice,
) -> Result<UnitPrice, CatalogError> {
    if given == expected {
        return Ok(price);
    }
    conversion
        .ok_or(CatalogError::UnitMismatch)?
        .apply(price, given, expected)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source(month: &str) -> CostSource {
        CostSource {
            title: "Tabela fictícia".into(),
            organization: "Instituto Exemplo".into(),
            license: "Uso autorizado para demonstração".into(),
            locality: "Florianópolis".into(),
            tax_regime: "Sem desoneração".into(),
            competence: month.into(),
        }
    }
    fn item(code: &str) -> CatalogItem {
        CatalogItem {
            code: code.into(),
            description: "Serviço fictício".into(),
            category: CostCategory::Service,
            unit: MeasureUnit::SquareMetre,
            price: "123.456789".parse().unwrap(),
        }
    }
    #[test]
    fn historical_competence_and_multiple_editions_are_explicit() {
        let mut lib = CatalogLibrary::default();
        let a = CatalogEdition::assemble(
            "ref".into(),
            1,
            "2021-04-01".parse().unwrap(),
            source("2021-04"),
            vec![item("A")],
        )
        .unwrap();
        let b = CatalogEdition::assemble(
            "ref".into(),
            2,
            "2026-11-01".parse().unwrap(),
            source("2026-11"),
            vec![item("A")],
        )
        .unwrap();
        assert!(a.outdated_on("2026-11"));
        lib.publish(a.clone()).unwrap();
        lib.publish(b).unwrap();
        assert_eq!(lib.editions_of("ref").len(), 2);
        assert_eq!(lib.edition("ref", 1).unwrap().source.competence, "2021-04");
        assert_eq!(lib.publish(a), Err(CatalogError::ImmutableEdition));
        assert_eq!(lib.edition("ref", 3), Err(CatalogError::MissingEdition));
    }
    #[test]
    fn compare_immutable_editions_reports_all_types_of_changes() {
        let first = CatalogEdition::assemble(
            "ref".into(),
            1,
            "2021-04-01".parse().unwrap(),
            source("2021-04"),
            vec![item("A"), item("B"), item("C")],
        )
        .unwrap();
        let mut changed = item("A");
        changed.price = "124.000000".parse().unwrap();
        let second = CatalogEdition::assemble(
            "ref".into(),
            2,
            "2026-11-01".parse().unwrap(),
            source("2026-11"),
            vec![changed.clone(), item("B"), item("D")],
        )
        .unwrap();
        let delta = first.compare_to(&second).unwrap();
        assert_eq!((delta.before_edition, delta.after_edition), (1, 2));
        assert_eq!(delta.added[0].code, "D");
        assert_eq!(delta.removed[0].code, "C");
        assert_eq!(delta.changed[0].before.code, "A");
        assert_eq!(delta.changed[0].after.price, changed.price);
        assert_eq!(first.get("A").unwrap().price.to_string(), "123.456789");
        assert_eq!(
            first.compare_to(&first),
            Err(CatalogError::IncompatibleComparison)
        );
        let different = CatalogEdition::assemble(
            "other".into(),
            1,
            "2026-11-01".parse().unwrap(),
            source("2026-11"),
            vec![item("A")],
        )
        .unwrap();
        assert_eq!(
            first.compare_to(&different),
            Err(CatalogError::IncompatibleComparison)
        );
    }
    #[test]
    fn historical_diff_of_ten_thousand_prices_keeps_every_record_and_source() {
        let before_rows: Vec<_> = (0..10_000)
            .map(|index| item(&format!("SERV-{index:05}")))
            .collect();
        let mut after_rows = before_rows.clone();
        after_rows.remove(20);
        after_rows[30].price = "456.000000".parse().unwrap();
        after_rows.push(item("NOVO"));
        let before = CatalogEdition::assemble(
            "grande".into(),
            1,
            "2026-01-01".parse().unwrap(),
            source("2026-01"),
            before_rows,
        )
        .unwrap();
        let after = CatalogEdition::assemble(
            "grande".into(),
            2,
            "2026-11-01".parse().unwrap(),
            source("2026-11"),
            after_rows,
        )
        .unwrap();
        let difference = before.compare_to(&after).unwrap();
        assert_eq!(
            (difference.added.len(), difference.changed.len(), difference.removed.len()),
            (1, 1, 1)
        );
        assert_eq!(difference.added[0].code, "NOVO");
        assert_eq!(difference.removed[0].code, "SERV-00020");
        assert_eq!(difference.changed[0].before.code, "SERV-00031");
        assert_eq!(
            before.get("SERV-00031").unwrap().price.to_string(),
            "123.456789"
        );
        assert_eq!(before.items.len(), 10_000);
        assert_eq!(after.items.len(), 10_000);
        assert_eq!(difference.before_source.competence, "2026-01");
        assert_eq!(difference.after_source.competence, "2026-11");
    }
    #[test]
    fn wrong_units_and_failed_import_do_not_publish_partial_edition() {
        let price = "10".parse().unwrap();
        assert!(matches!(
            require_matching_units(MeasureUnit::Tonne, MeasureUnit::Kilogram, None, price),
            Err(CatalogError::UnitMismatch)
        ));
        let cv = UnitConversion {
            from: MeasureUnit::Tonne,
            to: MeasureUnit::Kilogram,
            factor: "0.001".parse().unwrap(),
            reference: "Conversão declarada".into(),
        };
        assert_eq!(
            require_matching_units(MeasureUnit::Tonne, MeasureUnit::Kilogram, Some(&cv), price)
                .unwrap()
                .to_string(),
            "0.010000"
        );
        let mut lib = CatalogLibrary::default();
        let failure = CatalogEdition::assemble(
            "ref".into(),
            1,
            "2021-04-01".parse().unwrap(),
            source("2021-04"),
            vec![item("A"), item("A")],
        );
        assert_eq!(failure, Err(CatalogError::Item));
        assert!(lib.editions_of("ref").is_empty());
        let v = CatalogEdition::assemble(
            "ref".into(),
            1,
            "2021-04-01".parse().unwrap(),
            source("2021-04"),
            vec![item("A")],
        )
        .unwrap();
        lib.publish(v).unwrap();
    }
}
