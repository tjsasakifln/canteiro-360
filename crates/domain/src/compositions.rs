//! Composições recursivas sem ciclos. BDI, descontos e incidências têm uma aplicação.
use crate::{
    catalogs::{
        CatalogError, CatalogLibrary, CostCategory, UnitConversion, require_matching_units,
    },
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
    /// Memória discriminada da revisão; valores de insumos não são preços de venda.
    pub line_details: Vec<CostLineMemory>,
    pub incidence_details: Vec<CostIncidenceMemory>,
    pub category_totals: Vec<CostCategoryMemory>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CostLineMemory {
    pub id: String,
    pub source: ComponentSource,
    pub category: CostCategory,
    pub coefficient: Quantity,
    pub unit: MeasureUnit,
    pub unit_cost: UnitPrice,
    pub amount: UnitPrice,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CostIncidenceMemory {
    pub id: String,
    pub base_lines: Vec<String>,
    pub percent: Percent,
    pub basis: UnitPrice,
    pub amount: UnitPrice,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CostCategoryMemory {
    pub category: CostCategory,
    pub amount: UnitPrice,
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
            match &line.source {
                ComponentSource::Composition { code } if !text(code, 80) => {
                    return Err(CompositionError::Invalid);
                }
                ComponentSource::Catalog {
                    catalog_id,
                    edition,
                    code,
                } if !text(catalog_id, 128) || !text(code, 128) || *edition == 0 => {
                    return Err(CompositionError::Invalid);
                }
                _ => {}
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
    /// Valida a árvore de referências mesmo quando o custo é informado.
    /// A travessia iterativa impede estouro de pilha em cadeias profundas.
    pub fn validate_composition_links(&self, code: &str) -> Result<(), CompositionError> {
        let mut status = BTreeMap::<String, u8>::new();
        let mut path = Vec::<String>::new();
        let mut pending = vec![(code.to_owned(), false)];
        while let Some((current, closing)) = pending.pop() {
            if closing {
                status.insert(current, 2);
                path.pop();
                continue;
            }
            match status.get(&current) {
                Some(&2) => continue,
                Some(&1) => {
                    let start = path.iter().position(|entry| entry == &current).unwrap_or(0);
                    let mut cycle = path[start..].to_vec();
                    cycle.push(current);
                    return Err(CompositionError::Cycle(cycle.join(" -> ")));
                }
                _ => {}
            }
            let item = self
                .compositions
                .get(&current)
                .ok_or_else(|| CompositionError::Missing(current.clone()))?;
            let mut children = Vec::new();
            for line in &item.lines {
                if let ComponentSource::Composition { code: dependency } = &line.source {
                    let source = self
                        .compositions
                        .get(dependency)
                        .ok_or_else(|| CompositionError::Missing(dependency.clone()))?;
                    require_matching_units(
                        source.unit,
                        line.unit,
                        line.conversion.as_ref(),
                        UnitPrice::new(ExactDecimal::ZERO)?,
                    )?;
                    children.push(dependency.clone());
                }
            }
            path.push(current.clone());
            status.insert(current.clone(), 1);
            pending.push((current, true));
            for dependency in children.into_iter().rev() {
                pending.push((dependency, false));
            }
        }
        Ok(())
    }
    /// Mudanças em uma composição também precisam preservar as unidades dos
    /// vínculos que outras composições mantêm com ela, inclusive de custo informado.
    /// Uma revisão do insumo não deve invalidar serviços já cadastrados.
    pub fn validate_dependents_of(&self, code: &str) -> Result<(), CompositionError> {
        let source = self
            .compositions
            .get(code)
            .ok_or_else(|| CompositionError::Missing(code.into()))?;
        for composition in self.compositions.values() {
            for line in &composition.lines {
                if let ComponentSource::Composition { code: dependency } = &line.source
                    && dependency == code
                {
                    require_matching_units(
                        source.unit,
                        line.unit,
                        line.conversion.as_ref(),
                        UnitPrice::new(ExactDecimal::ZERO)?,
                    )?;
                }
            }
        }
        Ok(())
    }
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
        // Evaluate each reachable formula once, in dependency order. An
        // explicit stack avoids call-stack overflow and repeated expansion of
        // shared subcompositions. Only the root needs its full cost memory.
        let mut costs = BTreeMap::<String, UnitPrice>::new();
        let mut active = BTreeSet::new();
        let mut path = Vec::<String>::new();
        let mut pending = vec![(code.to_owned(), false)];
        while let Some((current, closing)) = pending.pop() {
            let item = self
                .compositions
                .get(&current)
                .ok_or_else(|| CompositionError::Missing(current.clone()))?;
            if closing {
                let memory = self.calculate_one(item, catalogs, &costs)?;
                if current == code {
                    return Ok(memory);
                }
                costs.insert(current.clone(), memory.direct_cost);
                active.remove(&current);
                path.pop();
                continue;
            }
            if costs.contains_key(&current) {
                continue;
            }
            if !active.insert(current.clone()) {
                let start = path.iter().position(|entry| entry == &current).unwrap_or(0);
                let mut cycle = path[start..].to_vec();
                cycle.push(current);
                return Err(CompositionError::Cycle(cycle.join(" -> ")));
            }
            path.push(current.clone());
            pending.push((current, true));
            if matches!(&item.cost_mode, CostMode::Formula) {
                for line in item.lines.iter().rev() {
                    if let ComponentSource::Composition { code } = &line.source {
                        pending.push((code.clone(), false));
                    }
                }
            }
        }
        Err(CompositionError::Missing(code.into()))
    }
    fn calculate_one(
        &self,
        item: &Composition,
        catalogs: &CatalogLibrary,
        costs: &BTreeMap<String, UnitPrice>,
    ) -> Result<CostMemory, CompositionError> {
        let mut amounts = BTreeMap::new();
        let mut line_details = Vec::new();
        let mut incidence_details = Vec::new();
        let mut category_amounts = [ExactDecimal::ZERO; 5];
        let (base, incidence, informed) = match &item.cost_mode {
            CostMode::Informed { cost, .. } => (*cost, UnitPrice::new(ExactDecimal::ZERO)?, true),
            CostMode::Formula => {
                let mut base = ExactDecimal::ZERO;
                for line in &item.lines {
                    let (price, unit, category) = match &line.source {
                        ComponentSource::Catalog {
                            catalog_id,
                            edition,
                            code,
                        } => {
                            let source = catalogs
                                .edition(catalog_id, *edition)?
                                .get(code)
                                .ok_or_else(|| CompositionError::Missing(code.clone()))?;
                            (source.price, source.unit, source.category)
                        }
                        ComponentSource::Composition { code } => {
                            let source = self
                                .compositions
                                .get(code)
                                .ok_or_else(|| CompositionError::Missing(code.clone()))?;
                            (
                                *costs
                                    .get(code)
                                    .ok_or_else(|| CompositionError::Missing(code.clone()))?,
                                source.unit,
                                CostCategory::Service,
                            )
                        }
                    };
                    let unit_cost =
                        require_matching_units(unit, line.unit, line.conversion.as_ref(), price)?;
                    let amount = unit_cost.value().checked_mul(line.coefficient.value())?;
                    base = base.checked_add(amount)?;
                    let index = match category {
                        CostCategory::Material => 0,
                        CostCategory::Labor => 1,
                        CostCategory::Equipment => 2,
                        CostCategory::Service => 3,
                        CostCategory::Transport => 4,
                    };
                    category_amounts[index] = category_amounts[index].checked_add(amount)?;
                    line_details.push(CostLineMemory {
                        id: line.id.clone(),
                        source: line.source.clone(),
                        category,
                        coefficient: line.coefficient,
                        unit: line.unit,
                        unit_cost,
                        amount: unit_price(amount)?,
                    });
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
                    incidence_details.push(CostIncidenceMemory {
                        id: incidence.id.clone(),
                        base_lines: incidence.base_lines.clone(),
                        percent: incidence.percent,
                        basis: unit_price(basis)?,
                        amount: unit_price(part)?,
                    });
                }
                (unit_price(base)?, unit_price(incidences)?, false)
            }
        };
        let direct = unit_price(base.value().checked_add(incidence.value())?)?;
        let selling = negotiated_unit_price(direct, item.bdi, item.discount)?;
        let category_totals = if informed {
            Vec::new()
        } else {
            [
                CostCategory::Material,
                CostCategory::Labor,
                CostCategory::Equipment,
                CostCategory::Service,
                CostCategory::Transport,
            ]
            .into_iter()
            .zip(category_amounts)
            .map(|(category, amount)| {
                Ok(CostCategoryMemory {
                    category,
                    amount: unit_price(amount)?,
                })
            })
            .collect::<Result<Vec<_>, CompositionError>>()?
        };
        Ok(CostMemory {
            code: item.code.clone(),
            base_cost: base,
            incidence_cost: incidence,
            direct_cost: direct,
            selling_price: selling,
            free_item: selling.value().is_zero(),
            informed,
            line_details,
            incidence_details,
            category_totals,
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
    fn informed_compositions_still_require_existing_acyclic_compatible_dependencies() {
        let link = |source: &str| ComponentLine {
            id: "base".into(),
            source: ComponentSource::Composition {
                code: source.into(),
            },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::SquareMetre,
            conversion: None,
        };
        let mut a = c("A");
        a.lines.push(link("B"));
        let b = c("B");
        let mut valid = CompositionBook::default();
        valid.insert(a.clone()).unwrap();
        valid.insert(b.clone()).unwrap();
        assert!(valid.validate_composition_links("A").is_ok());
        assert!(
            valid
                .calculate("A", &CatalogLibrary::default())
                .unwrap()
                .informed
        );

        let mut missing = CompositionBook::default();
        missing.insert(a.clone()).unwrap();
        assert_eq!(
            missing.validate_composition_links("A"),
            Err(CompositionError::Missing("B".into()))
        );

        let mut circular = CompositionBook::default();
        let mut recursive = b.clone();
        recursive.lines.push(link("A"));
        circular.insert(a.clone()).unwrap();
        circular.insert(recursive).unwrap();
        assert_eq!(
            circular.validate_composition_links("A"),
            Err(CompositionError::Cycle("A -> B -> A".into()))
        );

        let mut incompatible = CompositionBook::default();
        let mut b_wrong_unit = b;
        b_wrong_unit.unit = MeasureUnit::Kilogram;
        incompatible.insert(a).unwrap();
        incompatible.insert(b_wrong_unit).unwrap();
        assert!(matches!(
            incompatible.validate_composition_links("A"),
            Err(CompositionError::Catalog(_))
        ));
    }

    #[test]
    fn editing_a_source_unit_must_preserve_existing_dependent_links() {
        let mut dependent = c("SERVICO");
        dependent.lines.push(ComponentLine {
            id: "parte".into(),
            source: ComponentSource::Composition {
                code: "INSUMO".into(),
            },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::SquareMetre,
            conversion: None,
        });
        let mut revised = c("INSUMO");
        revised.unit = MeasureUnit::Kilogram;
        let mut book = CompositionBook::default();
        book.insert(dependent).unwrap();
        book.insert(revised).unwrap();
        // Verificar apenas os vínculos de saída da revisão não detectaria
        // uma referência de entrada tornada inválida por troca de unidade.
        assert!(book.validate_composition_links("INSUMO").is_ok());
        assert!(matches!(
            book.validate_dependents_of("INSUMO"),
            Err(CompositionError::Catalog(_))
        ));

        let mut compatible = c("INSUMO");
        compatible.unit = MeasureUnit::SquareMetre;
        let mut valid = CompositionBook::default();
        valid.insert(c("OUTRA")).unwrap();
        valid.insert(compatible).unwrap();
        assert!(valid.validate_dependents_of("INSUMO").is_ok());
        assert_eq!(
            valid.validate_dependents_of("AUSENTE"),
            Err(CompositionError::Missing("AUSENTE".into()))
        );
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
    fn informed_cost_still_requires_well_formed_catalog_references() {
        let mut item = c("A");
        item.lines.push(ComponentLine {
            id: "insumo".into(),
            source: ComponentSource::Catalog {
                catalog_id: "fonte".into(),
                edition: 1,
                code: "COD-1".into(),
            },
            coefficient: "1".parse().unwrap(),
            unit: MeasureUnit::Unit,
            conversion: None,
        });
        assert!(item.validate().is_ok());
        item.lines[0].source = ComponentSource::Catalog {
            catalog_id: "".into(),
            edition: 1,
            code: "COD-1".into(),
        };
        assert_eq!(item.validate(), Err(CompositionError::Invalid));
        item.lines[0].source = ComponentSource::Catalog {
            catalog_id: "fonte".into(),
            edition: 0,
            code: "COD-1".into(),
        };
        assert_eq!(item.validate(), Err(CompositionError::Invalid));
        item.lines[0].source = ComponentSource::Catalog {
            catalog_id: "fonte".into(),
            edition: 1,
            code: " ".into(),
        };
        assert_eq!(item.validate(), Err(CompositionError::Invalid));
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
    fn deep_compositions_do_not_use_the_call_stack() {
        let mut book = CompositionBook::default();
        book.insert(c("BASE")).unwrap();
        let mut dependency = "BASE".to_owned();
        for index in 0..10_000 {
            let code = format!("C{index}");
            let mut item = c(&code);
            item.cost_mode = CostMode::Formula;
            item.lines.push(ComponentLine {
                id: "base".into(),
                source: ComponentSource::Composition { code: dependency },
                coefficient: "1".parse().unwrap(),
                unit: MeasureUnit::SquareMetre,
                conversion: None,
            });
            book.insert(item).unwrap();
            dependency = code;
        }
        let result = book
            .calculate(&dependency, &CatalogLibrary::default())
            .unwrap();
        assert_eq!(result.direct_cost.to_string(), "100.000000");
        assert_eq!(result.selling_price.to_string(), "108.000000");
    }

    #[test]
    fn shared_compositions_are_evaluated_without_exponential_expansion() {
        let mut book = CompositionBook::default();
        book.insert(c("BASE")).unwrap();
        let mut dependency = "BASE".to_owned();
        for index in 0..40 {
            let code = format!("C{index}");
            let mut item = c(&code);
            item.cost_mode = CostMode::Formula;
            for id in ["A", "B"] {
                item.lines.push(ComponentLine {
                    id: id.into(),
                    source: ComponentSource::Composition {
                        code: dependency.clone(),
                    },
                    coefficient: "0.5".parse().unwrap(),
                    unit: MeasureUnit::SquareMetre,
                    conversion: None,
                });
            }
            book.insert(item).unwrap();
            dependency = code;
        }
        let result = book
            .calculate(&dependency, &CatalogLibrary::default())
            .unwrap();
        assert_eq!(result.direct_cost.to_string(), "100.000000");
        assert_eq!(result.selling_price.to_string(), "108.000000");
        assert_eq!(result.line_details.len(), 2);
        assert_eq!(result.line_details[0].amount.to_string(), "50.000000");
        assert_eq!(result.line_details[1].amount.to_string(), "50.000000");
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
        assert_eq!(result.line_details.len(), 1);
        assert_eq!(result.line_details[0].category, CostCategory::Material);
        assert_eq!(result.line_details[0].amount.to_string(), "100.000000");
        assert_eq!(result.incidence_details.len(), 1);
        assert_eq!(result.incidence_details[0].basis.to_string(), "100.000000");
        assert_eq!(result.incidence_details[0].amount.to_string(), "10.000000");
        assert_eq!(result.category_totals[0].category, CostCategory::Material);
        assert_eq!(result.category_totals[0].amount.to_string(), "100.000000");
        assert_eq!(result.category_totals[2].category, CostCategory::Equipment);
        assert_eq!(result.category_totals[4].category, CostCategory::Transport);
        assert_eq!(result.category_totals[4].amount.to_string(), "0.000000");
    }
}
