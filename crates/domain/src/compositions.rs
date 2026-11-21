//! Composições recursivas sem ciclos. BDI, descontos e incidências têm uma aplicação.
use crate::{
    catalogs::{CatalogError, CatalogLibrary, UnitConversion, require_matching_units},
    decimal::{DecimalError, ExactDecimal, Percent, Quantity, UnitPrice, negotiated_unit_price},
    units::MeasureUnit,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComponentSource {
    Catalog {
        catalog_id: String,
        edition: u32,
        code: String,
    },
    Composition {
        code: String,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentLine {
    pub id: String,
    pub source: ComponentSource,
    pub coefficient: Quantity,
    /// Unidade da parcela resultante, diferente da fonte somente com conversão documentada.
    pub unit: MeasureUnit,
    pub conversion: Option<UnitConversion>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostIncidence {
    pub id: String,
    pub percent: Percent,
    /// A base contém apenas linhas de insumo, nunca outra incidência.
    pub base_lines: Vec<String>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CostMode {
    Formula,
    Informed { cost: UnitPrice, reason: String },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub code: String,
    pub description: String,
    pub unit: MeasureUnit,
    pub cost_mode: CostMode,
    pub lines: Vec<ComponentLine>,
    pub incidences: Vec<CostIncidence>,
    pub bdi: Percent,
    pub discount: Percent,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostMemory {
    pub code: String,
    pub base_cost: UnitPrice,
    pub incidence_cost: UnitPrice,
    pub direct_cost: UnitPrice,
    pub selling_price: UnitPrice,
    pub free_item: bool,
    pub informed: bool,
}
#[derive(Clone, Debug, Default)]
pub struct CompositionBook {
    compositions: BTreeMap<String, Composition>,
}
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompositionError {
    #[error("Identificação, coeficiente ou referência inválida.")]
    Invalid,
    #[error("Composição ou item de catálogo inexistente: {0}.")]
    Missing(String),
    #[error("Composição circular: {0}.")]
    Cycle(String),
    #[error("A incidência deve apontar apenas para linhas existentes, sem repetição.")]
    Incidence,
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Decimal(#[from] DecimalError),
}
fn text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.trim() == s && s.len() <= max && !s.chars().any(char::is_control)
}
fn unit_price(value: ExactDecimal) -> Result<UnitPrice, CompositionError> {
    Ok(UnitPrice::new(value.round_away(6)?)?)
}
impl Composition {
    pub fn validate(&self) -> Result<(), CompositionError> {
        if !text(&self.code, 80)
            || !text(&self.description, 800)
            || self.lines.len() > 10000
            || self.incidences.len() > 1000
        {
            return Err(CompositionError::Invalid);
        }
        if let CostMode::Informed { reason, .. } = &self.cost_mode
            && !text(reason, 500)
        {
            return Err(CompositionError::Invalid);
        }
        let mut ids = BTreeSet::new();
        for line in &self.lines {
            if !text(&line.id, 80) || !ids.insert(line.id.as_str()) {
                return Err(CompositionError::Invalid);
            }
            if let ComponentSource::Composition { code } = &line.source
                && !text(code, 80)
            {
                return Err(CompositionError::Invalid);
            }
        }
        let mut incidence_ids = BTreeSet::new();
        for incidence in &self.incidences {
            if !text(&incidence.id, 80)
                || !incidence_ids.insert(incidence.id.as_str())
                || incidence.base_lines.is_empty()
                || incidence.percent.value().is_negative()
            {
                return Err(CompositionError::Incidence);
            }
            let mut bases = BTreeSet::new();
            for base in &incidence.base_lines {
                if !ids.contains(base.as_str()) || !bases.insert(base) {
                    return Err(CompositionError::Incidence);
                }
            }
        }
        Ok(())
    }
}
impl CompositionBook {
    pub fn insert(&mut self, composition: Composition) -> Result<(), CompositionError> {
        composition.validate()?;
        if self.compositions.contains_key(&composition.code) {
            return Err(CompositionError::Invalid);
        }
        self.compositions
            .insert(composition.code.clone(), composition);
        Ok(())
    }
    pub fn calculate(
        &self,
        code: &str,
        catalogs: &CatalogLibrary,
    ) -> Result<CostMemory, CompositionError> {
        self.walk(code, catalogs, &mut Vec::new())
    }
    fn walk(
        &self,
        code: &str,
        catalogs: &CatalogLibrary,
        path: &mut Vec<String>,
    ) -> Result<CostMemory, CompositionError> {
        if let Some(at) = path.iter().position(|entry| entry == code) {
            let mut cycle = path[at..].to_vec();
            cycle.push(code.into());
            return Err(CompositionError::Cycle(cycle.join(" -> ")));
        }
        let composition = self
            .compositions
            .get(code)
            .ok_or_else(|| CompositionError::Missing(code.into()))?;
        path.push(code.into());
        let result = self.calculate_one(composition, catalogs, path);
        path.pop();
        result
    }
    fn calculate_one(
        &self,
        item: &Composition,
        catalogs: &CatalogLibrary,
        path: &mut Vec<String>,
    ) -> Result<CostMemory, CompositionError> {
        let mut amounts = BTreeMap::new();
        let (base, incidence, informed) = match &item.cost_mode {
            CostMode::Informed { cost, .. } => (*cost, UnitPrice::new(ExactDecimal::ZERO)?, true),
            CostMode::Formula => {
                let mut base = ExactDecimal::ZERO;
                for line in &item.lines {
                    let (price, unit) = match &line.source {
                        ComponentSource::Catalog {
                            catalog_id,
                            edition,
                            code,
                        } => {
                            let source = catalogs
                                .edition(catalog_id, *edition)?
                                .get(code)
                                .ok_or_else(|| CompositionError::Missing(code.clone()))?;
                            (source.price, source.unit)
                        }
                        ComponentSource::Composition { code } => {
                            let source = self
                                .compositions
                                .get(code)
                                .ok_or_else(|| CompositionError::Missing(code.clone()))?;
                            (self.walk(code, catalogs, path)?.direct_cost, source.unit)
                        }
                    };
                    let unit_cost =
                        require_matching_units(unit, line.unit, line.conversion.as_ref(), price)?;
                    let amount = unit_cost.value().checked_mul(line.coefficient.value())?;
                    base = base.checked_add(amount)?;
                    amounts.insert(line.id.clone(), amount);
                }
                let mut incidences = ExactDecimal::ZERO;
                for incidence in &item.incidences {
                    let mut basis = ExactDecimal::ZERO;
                    for id in &incidence.base_lines {
                        basis = basis
                            .checked_add(*amounts.get(id).ok_or(CompositionError::Incidence)?)?;
                    }
                    let part = basis
                        .checked_mul(incidence.percent.value())?
                        .checked_div_exact("100".parse()?)?;
                    incidences = incidences.checked_add(part)?;
                }
                (unit_price(base)?, unit_price(incidences)?, false)
            }
        };
        let direct = unit_price(base.value().checked_add(incidence.value())?)?;
        let selling = negotiated_unit_price(direct, item.bdi, item.discount)?;
        Ok(CostMemory {
            code: item.code.clone(),
            base_cost: base,
            incidence_cost: incidence,
            direct_cost: direct,
            selling_price: selling,
            free_item: selling.value().is_zero(),
            informed,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn c(code: &str) -> Composition {
        Composition {
            code: code.into(),
            description: "Serviço fictício".into(),
            unit: MeasureUnit::SquareMetre,
            cost_mode: CostMode::Informed {
                cost: "100".parse().unwrap(),
                reason: "Custo informado pelo orçamentista".into(),
            },
            lines: vec![],
            incidences: vec![],
            bdi: "20".parse().unwrap(),
            discount: "10".parse().unwrap(),
        }
    }
    #[test]
    fn bdi_and_discount_apply_once_and_free_item_is_explicit() {
        let mut book = CompositionBook::default();
        book.insert(c("A")).unwrap();
        let a = book.calculate("A", &CatalogLibrary::default()).unwrap();
        assert_eq!(a.selling_price.to_string(), "108.000000");
        assert!(a.informed);
        let mut free = c("B");
        free.discount = "100".parse().unwrap();
        book.insert(free).unwrap();
        assert!(
            book.calculate("B", &CatalogLibrary::default())
                .unwrap()
                .free_item
        );
        let mut excessive = c("C");
        excessive.discount = "150".parse().unwrap();
        book.insert(excessive).unwrap();
        assert!(book.calculate("C", &CatalogLibrary::default()).is_err());
        assert!("-1".parse::<Quantity>().is_err());
    }
    #[test]
    fn mutually_recursive_compositions_report_complete_cycle() {
        let mut a = c("A");
        a.cost_mode = CostMode::Formula;
        a.lines.push(ComponentLine {
            id: "a".into(),
            source: ComponentSource::Composition { code: "B".into() },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::SquareMetre,
            conversion: None,
        });
        let mut b = c("B");
        b.cost_mode = CostMode::Formula;
        b.lines.push(ComponentLine {
            id: "b".into(),
            source: ComponentSource::Composition { code: "A".into() },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::SquareMetre,
            conversion: None,
        });
        let mut book = CompositionBook::default();
        book.insert(a).unwrap();
        book.insert(b).unwrap();
        assert_eq!(
            book.calculate("A", &CatalogLibrary::default()),
            Err(CompositionError::Cycle("A -> B -> A".into()))
        );
    }
    #[test]
    fn incidence_uses_only_its_declared_basis() {
        use crate::catalogs::{CatalogEdition, CatalogItem, CostCategory, CostSource};
        let catalog = CatalogEdition::assemble(
            "ref".into(),
            1,
            "2021-04-01".parse().unwrap(),
            CostSource {
                title: "Tabela exemplo".into(),
                organization: "Origem".into(),
                license: "Uso autorizado".into(),
                locality: "SC".into(),
                tax_regime: "Direto".into(),
                competence: "2021-04".into(),
            },
            vec![CatalogItem {
                code: "X".into(),
                description: "Material exemplo".into(),
                category: CostCategory::Material,
                unit: MeasureUnit::Unit,
                price: "100".parse().unwrap(),
            }],
        )
        .unwrap();
        let mut lib = CatalogLibrary::default();
        lib.publish(catalog).unwrap();
        let mut a = c("A");
        a.cost_mode = CostMode::Formula;
        a.unit = MeasureUnit::Unit;
        a.bdi = "0".parse().unwrap();
        a.discount = "0".parse().unwrap();
        a.lines = vec![ComponentLine {
            id: "material".into(),
            source: ComponentSource::Catalog {
                catalog_id: "ref".into(),
                edition: 1,
                code: "X".into(),
            },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::Unit,
            conversion: None,
        }];
        a.incidences = vec![CostIncidence {
            id: "encargo".into(),
            percent: "10".parse().unwrap(),
            base_lines: vec!["material".into()],
        }];
        let mut book = CompositionBook::default();
        book.insert(a).unwrap();
        let result = book.calculate("A", &lib).unwrap();
        assert_eq!(result.base_cost.to_string(), "100.000000");
        assert_eq!(result.incidence_cost.to_string(), "10.000000");
        assert_eq!(result.selling_price.to_string(), "110.000000");
    }
}
