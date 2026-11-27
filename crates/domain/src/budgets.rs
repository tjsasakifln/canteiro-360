//! Orçamentos mantêm variações, grupos, revisões e calendário reconciliáveis.
use crate::{
    ActorId, CivilDate, EntityId, UtcInstant,
    decimal::{
        Allocation, AllocationWeight, DecimalError, ExactDecimal, Money, Percent, Quantity,
        UnitPrice, allocate_money, allocate_percentages,
    },
    units::MeasureUnit,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetState {
    Draft,
    Review,
    Approved,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetVariation {
    pub id: String,
    pub work_id: EntityId,
    pub scope: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLine {
    pub id: String,
    pub variation_id: String,
    pub group: String,
    pub stage: String,
    pub substage: String,
    pub code: String,
    pub description: String,
    pub unit: MeasureUnit,
    pub quantity: Quantity,
    pub price: UnitPrice,
    pub catalog_id: Option<String>,
    pub catalog_edition: Option<u32>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetRevision {
    pub budget_id: String,
    pub revision: u32,
    pub status: BudgetState,
    pub responsible_id: ActorId,
    pub variations: Vec<BudgetVariation>,
    pub lines: Vec<BudgetLine>,
    pub schedule: Option<BudgetSchedule>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetGroupTotal {
    pub variation_id: String,
    pub group: String,
    pub amount: Money,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetSummary {
    pub groups: Vec<BudgetGroupTotal>,
    pub total: Money,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeriodBasis {
    Fixed30Days,
    CivilMonths,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulePeriod {
    pub id: String,
    pub position: u32,
    pub percentage: Percent,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetSchedule {
    pub starts_on: CivilDate,
    pub deadline_days: u32,
    pub basis: PeriodBasis,
    pub periods: Vec<SchedulePeriod>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditLease {
    pub actor: ActorId,
    pub session: EntityId,
    pub revision: u32,
    pub valid_until: UtcInstant,
}
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BudgetError {
    #[error("A estrutura do orçamento está incompleta ou contém identificações repetidas.")]
    Structure,
    #[error("O prazo ou rateio do cronograma é inválido.")]
    Schedule,
    #[error("A versão mudou ou a reserva de edição expirou. O rascunho deve ser preservado.")]
    Lease,
    #[error("Esta revisão é definitiva e não pode ser alterada diretamente.")]
    Definitive,
    #[error("O item possui referência em composição ou documento definitivo.")]
    ReferencedItem,
    #[error(transparent)]
    Decimal(#[from] DecimalError),
}
pub(crate) fn bounded(s: &str, limit: usize) -> bool {
    !s.trim().is_empty()
        && s.trim() == s
        && s.len() <= limit
        && !s.chars().any(char::is_control)
        && crate::budget_csv::safe(s)
}
impl BudgetLine {
    pub fn total(&self) -> Result<Money, BudgetError> {
        Ok(Money::new(
            self.quantity
                .value()
                .checked_mul(self.price.value())?
                .truncate(2)?,
        )?)
    }
}
impl BudgetRevision {
    pub fn validate(&self) -> Result<(), BudgetError> {
        if !bounded(&self.budget_id, 120)
            || self.revision == 0
            || self.variations.is_empty()
            || self.variations.len() > 500
            || self.lines.len() > 100_000
        {
            return Err(BudgetError::Structure);
        }
        let mut variation_ids = BTreeSet::new();
        let mut real_scopes = BTreeSet::new();
        for v in &self.variations {
            if !bounded(&v.id, 120)
                || !bounded(&v.scope, 500)
                || !variation_ids.insert(v.id.as_str())
                || !real_scopes.insert((v.work_id, v.scope.as_str()))
            {
                return Err(BudgetError::Structure);
            }
        }
        let mut line_ids = BTreeSet::new();
        for l in &self.lines {
            if !bounded(&l.id, 120)
                || !line_ids.insert(l.id.as_str())
                || !variation_ids.contains(l.variation_id.as_str())
                || !bounded(&l.group, 160)
                || !bounded(&l.stage, 160)
                || !bounded(&l.substage, 160)
                || !bounded(&l.code, 80)
                || !bounded(&l.description, 800)
                || l.catalog_id.is_some() != l.catalog_edition.is_some()
                || l.catalog_edition == Some(0)
            {
                return Err(BudgetError::Structure);
            }
        }
        if let Some(schedule) = &self.schedule {
            schedule.validate()?;
        } else if self.status == BudgetState::Approved {
            return Err(BudgetError::Schedule);
        }
        Ok(())
    }
    pub fn summarize(&self) -> Result<BudgetSummary, BudgetError> {
        self.validate()?;
        let mut subtotals: BTreeMap<(String, String), ExactDecimal> = BTreeMap::new();
        let mut total = ExactDecimal::ZERO;
        for line in &self.lines {
            let amount = line.total()?.value();
            total = total.checked_add(amount)?;
            let key = (line.variation_id.clone(), line.group.clone());
            let prev = subtotals.entry(key).or_insert(ExactDecimal::ZERO);
            *prev = prev.checked_add(amount)?;
        }
        let groups = subtotals
            .into_iter()
            .map(|((variation_id, group), amount)| {
                Ok(BudgetGroupTotal {
                    variation_id,
                    group,
                    amount: Money::new(amount)?,
                })
            })
            .collect::<Result<Vec<_>, DecimalError>>()?;
        Ok(BudgetSummary {
            groups,
            total: Money::new(total)?,
        })
    }
    /// Vinculação a uma fonte histórica exige o código, unidade e preço oficiais.
    /// A checagem vale para qualquer snapshot, inclusive importação e edição integral.
    pub fn validate_catalog_references(
        &self,
        published: &BTreeMap<(String, u32, String), (MeasureUnit, UnitPrice)>,
    ) -> Result<(), BudgetError> {
        for line in &self.lines {
            if let (Some(catalog), Some(edition)) = (&line.catalog_id, line.catalog_edition) {
                let key = (catalog.clone(), edition, line.code.clone());
                if published.get(&key) != Some(&(line.unit, line.price)) {
                    return Err(BudgetError::ReferencedItem);
                }
            }
        }
        Ok(())
    }
    /// Uma edição não pode apagar a origem de preço mantendo a identidade do item.
    /// A exclusão explícita é tratada separadamente do comando de alteração.
    pub fn validate_catalog_lineage(&self, previous: &Self) -> Result<(), BudgetError> {
        let new_by_id = self
            .lines
            .iter()
            .map(|line| (line.id.as_str(), line))
            .collect::<BTreeMap<_, _>>();
        for old in &previous.lines {
            if old.catalog_id.is_some()
                && let Some(updated) = new_by_id.get(old.id.as_str())
                && (updated.catalog_id != old.catalog_id
                    || updated.catalog_edition != old.catalog_edition
                    || updated.code != old.code
                    || updated.unit != old.unit
                    || updated.price != old.price)
            {
                return Err(BudgetError::ReferencedItem);
            }
        }
        Ok(())
    }
    /// Gravações integrais não podem ocultar exclusões de serviços ou variações,
    /// inclusive variações vazias que podem ter referências externas.
    /// A remoção exige um comando específico com validação no servidor.
    pub fn validate_no_implicit_removals(&self, previous: &Self) -> Result<(), BudgetError> {
        let current_ids = self
            .lines
            .iter()
            .map(|line| line.id.as_str())
            .collect::<BTreeSet<_>>();
        if previous
            .lines
            .iter()
            .any(|line| !current_ids.contains(line.id.as_str()))
        {
            return Err(BudgetError::ReferencedItem);
        }
        let variation_ids = self
            .variations
            .iter()
            .map(|variation| variation.id.as_str())
            .collect::<BTreeSet<_>>();
        if previous
            .variations
            .iter()
            .any(|variation| !variation_ids.contains(variation.id.as_str()))
        {
            return Err(BudgetError::ReferencedItem);
        }
        Ok(())
    }
    /// Uma gravação integral ou importação não pode deslocar um serviço
    /// existente nem alterar a obra ou o escopo de uma variação antiga.
    /// O deslocamento exige MoveLine, que reconstrói o snapshot no servidor.
    pub fn validate_no_implicit_moves(&self, previous: &Self) -> Result<(), BudgetError> {
        let assignments = self
            .variations
            .iter()
            .map(|variation| {
                (
                    variation.id.as_str(),
                    (variation.work_id, variation.scope.as_str()),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if previous.variations.iter().any(|variation| {
            assignments
                .get(variation.id.as_str())
                .is_some_and(|assignment| {
                    *assignment != (variation.work_id, variation.scope.as_str())
                })
        }) {
            return Err(BudgetError::ReferencedItem);
        }
        let current = self
            .lines
            .iter()
            .map(|line| (line.id.as_str(), line.variation_id.as_str()))
            .collect::<BTreeMap<_, _>>();
        if previous.lines.iter().any(|line| {
            current
                .get(line.id.as_str())
                .is_some_and(|variation| *variation != line.variation_id.as_str())
        }) {
            return Err(BudgetError::ReferencedItem);
        }
        Ok(())
    }
    /// Um salvamento integral ou CSV não pode substituir silenciosamente
    /// o cronograma: a alteração exige comando explícito e confirmação.
    pub fn validate_no_implicit_reschedule(&self, previous: &Self) -> Result<(), BudgetError> {
        if self.schedule != previous.schedule {
            return Err(BudgetError::Schedule);
        }
        Ok(())
    }
    /// Apenas serviços manuais, sem fonte histórica vinculada, podem ser
    /// excluídos por este comando. O orçamento e o cronograma são atômicos.
    pub fn remove_unlinked_line(&mut self, line_id: &str) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        let selected = self
            .lines
            .iter()
            .find(|line| line.id == line_id)
            .ok_or(BudgetError::Structure)?;
        if selected.catalog_id.is_some() || selected.catalog_edition.is_some() {
            return Err(BudgetError::ReferencedItem);
        }
        let mut candidate = self.clone();
        candidate.lines.retain(|line| line.id != line_id);
        candidate.summarize()?;
        *self = candidate;
        Ok(())
    }
    /// Edição de serviço sem perder cronograma e sem substituir preço de fonte
    /// histórica por valor manual mantendo a referência de catálogo.
    /// O serviço de aplicação ainda exige reserva, versão e autorização atuais.
    pub fn edit_line_values(
        &mut self,
        line_id: &str,
        quantity: Quantity,
        price: UnitPrice,
    ) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        let mut candidate = self.clone();
        let line = candidate
            .lines
            .iter_mut()
            .find(|line| line.id == line_id)
            .ok_or(BudgetError::Structure)?;
        if line.catalog_id.is_some() && line.price != price {
            return Err(BudgetError::ReferencedItem);
        }
        line.quantity = quantity;
        line.price = price;
        candidate.summarize()?;
        *self = candidate;
        Ok(())
    }
    /// Copia um serviço preservando preço, unidade e edição de catálogo.
    /// O servidor deve conferir reserva, autorização e versão antes de salvar.
    /// A operação não modifica o orçamento se a cópia for inválida.
    pub fn copy_line(
        &mut self,
        source_id: &str,
        new_id: &str,
        target_variation_id: &str,
    ) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        if !bounded(new_id, 120)
            || !crate::budget_csv::safe(new_id)
            || self.lines.len() >= 100_000
            || self.lines.iter().any(|line| line.id == new_id)
            || !self
                .variations
                .iter()
                .any(|variation| variation.id == target_variation_id)
        {
            return Err(BudgetError::Structure);
        }
        let mut copy = self
            .lines
            .iter()
            .find(|line| line.id == source_id)
            .ok_or(BudgetError::Structure)?
            .clone();
        copy.id = new_id.to_owned();
        copy.variation_id = target_variation_id.to_owned();
        let mut candidate = self.clone();
        candidate.lines.push(copy);
        candidate.summarize()?;
        *self = candidate;
        Ok(())
    }
    /// Recorta e cola um serviço em outra variação sem alterar preço ou origem.
    /// A gravação no servidor continua condicionada a reserva e versão atuais.
    pub fn move_line(
        &mut self,
        line_id: &str,
        target_variation_id: &str,
    ) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        if !self
            .variations
            .iter()
            .any(|variation| variation.id == target_variation_id)
        {
            return Err(BudgetError::Structure);
        }
        let mut candidate = self.clone();
        let line = candidate
            .lines
            .iter_mut()
            .find(|line| line.id == line_id)
            .ok_or(BudgetError::Structure)?;
        line.variation_id = target_variation_id.to_owned();
        candidate.summarize()?;
        *self = candidate;
        Ok(())
    }
    /// Recria o rateio linear com revisão explícita de datas, base e períodos.
    /// O serviço valida novamente reserva, versão e escopo antes do commit.
    pub fn reschedule_linear(
        &mut self,
        starts_on: CivilDate,
        deadline_days: u32,
        basis: PeriodBasis,
        periods: u32,
    ) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        let mut candidate = self.clone();
        candidate.schedule = Some(BudgetSchedule::linear(
            starts_on,
            deadline_days,
            basis,
            periods,
        )?);
        candidate.summarize()?;
        *self = candidate;
        Ok(())
    }
    pub fn confirm_edit(
        &self,
        lease: &EditLease,
        actor: ActorId,
        session: EntityId,
        now: UtcInstant,
    ) -> Result<(), BudgetError> {
        if self.status == BudgetState::Approved {
            return Err(BudgetError::Definitive);
        }
        if lease.actor != actor
            || lease.session != session
            || lease.revision != self.revision
            || lease.valid_until <= now
        {
            return Err(BudgetError::Lease);
        }
        Ok(())
    }
    pub fn remove_line(
        &mut self,
        line_id: &str,
        is_referenced_or_final: bool,
        lease: &EditLease,
        actor: ActorId,
        session: EntityId,
        now: UtcInstant,
    ) -> Result<(), BudgetError> {
        self.confirm_edit(lease, actor, session, now)?;
        if is_referenced_or_final {
            return Err(BudgetError::ReferencedItem);
        }
        // A indicação externa de referências não substitui a verificação
        // da origem histórica presente no próprio orçamento.
        self.remove_unlinked_line(line_id)
    }
}
impl BudgetSchedule {
    /// Limites cronológicos ancorados no início original e no prazo formal.
    /// O último período pode ter menos dias que a cadência nominal.
    pub fn calendar_periods(&self) -> Result<Vec<crate::calendar::CalendarPeriod>, BudgetError> {
        use crate::calendar::{Cadence, CalendarPolicy, execution_end, schedule};

        self.validate()?;
        let policy = match self.basis {
            PeriodBasis::Fixed30Days => CalendarPolicy::FixedDays,
            PeriodBasis::CivilMonths => CalendarPolicy::Civil,
        };
        let end = execution_end(self.starts_on, self.deadline_days)
            .map_err(|_| BudgetError::Schedule)?;
        let periods = schedule(self.starts_on, end, policy, Cadence::Monthly)
            .map_err(|_| BudgetError::Schedule)?;
        if periods.len() != self.periods.len() {
            return Err(BudgetError::Schedule);
        }
        Ok(periods)
    }

    /// Distribui um valor monetário pelos períodos sem perder centavos.
    /// Pode ser aplicado ao valor de um grupo ou ao total do orçamento.
    pub fn financial_allocation(&self, amount: Money) -> Result<Allocation, BudgetError> {
        self.validate()?;
        let weights = self
            .periods
            .iter()
            .map(|period| AllocationWeight {
                id: period.id.clone(),
                position: period.position,
                weight: period.percentage.value(),
            })
            .collect::<Vec<_>>();
        Ok(allocate_money(amount, &weights)?)
    }

    /// Residual entregue na ordem estável dos períodos, não na ordem da consulta SQL.
    pub fn linear(
        starts_on: CivilDate,
        deadline_days: u32,
        basis: PeriodBasis,
        count: u32,
    ) -> Result<Self, BudgetError> {
        if count == 0
            || count > 10000
            || deadline_days == 0
            || !Self::period_count_matches(starts_on, deadline_days, basis, count)
        {
            return Err(BudgetError::Schedule);
        }
        let weights = (0..count)
            .map(|i| AllocationWeight {
                id: format!("periodo-{}", i + 1),
                position: i,
                weight: ExactDecimal::ONE,
            })
            .collect::<Vec<_>>();
        let parts = allocate_percentages(&weights)?;
        let periods = parts
            .parts
            .into_iter()
            .map(|p| {
                Ok(SchedulePeriod {
                    id: p.id,
                    position: p.position,
                    percentage: Percent::new(p.value)?,
                })
            })
            .collect::<Result<Vec<_>, DecimalError>>()?;
        let result = Self {
            starts_on,
            deadline_days,
            basis,
            periods,
        };
        result.validate()?;
        Ok(result)
    }
    fn period_count_matches(
        starts_on: CivilDate,
        deadline_days: u32,
        basis: PeriodBasis,
        count: u32,
    ) -> bool {
        if count == 0 || deadline_days == 0 {
            return false;
        }
        let Ok(end) = crate::calendar::execution_end(starts_on, deadline_days) else {
            return false;
        };
        match basis {
            PeriodBasis::Fixed30Days => count == deadline_days.div_ceil(30),
            PeriodBasis::CivilMonths => {
                // Boundaries use the original anchor, including February clamps.
                let Ok(previous) = crate::calendar::civil_months_after(starts_on, count - 1) else {
                    return false;
                };
                let reaches_end = match crate::calendar::civil_months_after(starts_on, count) {
                    Ok(boundary) => end <= boundary,
                    Err(_) => true, // Last residual may end within the supported date range.
                };
                previous < end && reaches_end
            }
        }
    }
    pub(crate) fn matches_fixed_period_count(&self) -> bool {
        self.basis != PeriodBasis::Fixed30Days || self.matches_period_count()
    }
    pub(crate) fn matches_period_count(&self) -> bool {
        Self::period_count_matches(
            self.starts_on,
            self.deadline_days,
            self.basis,
            self.periods.len() as u32,
        )
    }
    pub fn validate(&self) -> Result<(), BudgetError> {
        if self.deadline_days == 0
            || self.periods.is_empty()
            || self.periods.len() > 10000
            || !self.matches_period_count()
        {
            return Err(BudgetError::Schedule);
        }
        let mut ids = BTreeSet::new();
        let mut positions = BTreeSet::new();
        let mut sum = ExactDecimal::ZERO;
        for period in &self.periods {
            if !bounded(&period.id, 128)
                || !ids.insert(period.id.as_str())
                || !positions.insert(period.position)
            {
                return Err(BudgetError::Schedule);
            }
            sum = sum.checked_add(period.percentage.value())?;
        }
        // Cada posição identifica um período cronológico. Lacunas (ou início
        // diferente de zero) tornam o rateio ambíguo mesmo quando soma 100%.
        if positions.iter().copied().ne(0..self.periods.len() as u32) {
            return Err(BudgetError::Schedule);
        }
        if sum != "100".parse()? {
            return Err(BudgetError::Schedule);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn actor(n: u8) -> ActorId {
        format!("{n:032x}").parse().unwrap()
    }
    fn entity(n: u8) -> EntityId {
        format!("{n:032x}").parse().unwrap()
    }
    fn budget() -> BudgetRevision {
        BudgetRevision {
            budget_id: "orcamento-001".into(),
            revision: 1,
            status: BudgetState::Review,
            responsible_id: actor(1),
            variations: vec![
                BudgetVariation {
                    id: "v-01".into(),
                    work_id: entity(10),
                    scope: "Prédio A".into(),
                },
                BudgetVariation {
                    id: "v-02".into(),
                    work_id: entity(11),
                    scope: "Prédio B".into(),
                },
            ],
            lines: vec![
                BudgetLine {
                    id: "l-1".into(),
                    variation_id: "v-01".into(),
                    group: "Fundações".into(),
                    stage: "Estaca".into(),
                    substage: "Escavação".into(),
                    code: "SIN-A".into(),
                    description: "Serviço exemplo".into(),
                    unit: MeasureUnit::CubicMetre,
                    quantity: "2.000000".parse().unwrap(),
                    price: "100.000000".parse().unwrap(),
                    catalog_id: Some("ref".into()),
                    catalog_edition: Some(1),
                },
                BudgetLine {
                    id: "l-2".into(),
                    variation_id: "v-02".into(),
                    group: "Fundações".into(),
                    stage: "Blocos".into(),
                    substage: "Forma".into(),
                    code: "SIN-B".into(),
                    description: "Serviço exemplo".into(),
                    unit: MeasureUnit::SquareMetre,
                    quantity: "1".parse().unwrap(),
                    price: "30".parse().unwrap(),
                    catalog_id: None,
                    catalog_edition: None,
                },
            ],
            schedule: Some(
                BudgetSchedule::linear(
                    "2026-11-01".parse().unwrap(),
                    90,
                    PeriodBasis::Fixed30Days,
                    3,
                )
                .unwrap(),
            ),
        }
    }
    #[test]
    fn direct_revision_rejects_spreadsheet_formulas_before_persistence() {
        for dangerous in ["=1+1", "@SUM(1)", "\u{202e}manual"] {
            let mut changed = budget();
            changed.lines[1].description = dangerous.into();
            assert_eq!(changed.validate(), Err(BudgetError::Structure));
            changed = budget();
            changed.lines[1].code = dangerous.into();
            assert_eq!(changed.validate(), Err(BudgetError::Structure));
            changed = budget();
            changed.variations[0].scope = dangerous.into();
            assert_eq!(changed.validate(), Err(BudgetError::Structure));
        }
    }
    #[test]
    fn approved_budget_requires_a_valid_schedule() {
        let mut candidate = budget();
        let schedule = candidate.schedule.take().unwrap();
        candidate.validate().unwrap();
        candidate.status = BudgetState::Draft;
        candidate.validate().unwrap();
        candidate.status = BudgetState::Approved;
        assert_eq!(candidate.validate(), Err(BudgetError::Schedule));
        assert_eq!(candidate.summarize(), Err(BudgetError::Schedule));

        candidate.schedule = Some(schedule);
        candidate.validate().unwrap();
        candidate.summarize().unwrap();
        candidate.schedule.as_mut().unwrap().periods.clear();
        assert_eq!(candidate.validate(), Err(BudgetError::Schedule));
    }
    #[test]
    fn totals_reconcile_and_three_periods_allocate_stably() {
        let b = budget();
        let totals = b.summarize().unwrap();
        assert_eq!(totals.total.to_string(), "230.00");
        assert_eq!(totals.groups.len(), 2);
        assert_eq!(
            totals
                .groups
                .iter()
                .map(|g| g.amount.value())
                .try_fold(ExactDecimal::ZERO, |sum, v| sum.checked_add(v))
                .unwrap(),
            totals.total.value()
        );
        let percentages = b.schedule.unwrap().periods;
        assert_eq!(
            percentages
                .iter()
                .map(|p| p.percentage.to_string())
                .collect::<Vec<_>>(),
            vec!["33.34", "33.33", "33.33"]
        );
    }
    #[test]
    fn calendar_periods_preserve_anchor_and_formal_end() {
        let fixed = BudgetSchedule::linear(
            "2026-11-01".parse().unwrap(),
            65,
            PeriodBasis::Fixed30Days,
            3,
        )
        .unwrap();
        let periods = fixed.calendar_periods().unwrap();
        assert_eq!(periods[0].end.to_string(), "2026-12-01");
        assert_eq!(periods[1].end.to_string(), "2026-12-31");
        assert_eq!(periods[2].end.to_string(), "2027-01-05");
        assert_eq!(periods[2].days, 5);
        assert!(periods[2].residual);

        let mut civil = BudgetSchedule::linear(
            "2026-01-31".parse().unwrap(),
            70,
            PeriodBasis::CivilMonths,
            3,
        )
        .unwrap();
        civil.periods.reverse();
        let periods = civil.calendar_periods().unwrap();
        assert_eq!(periods[0].end.to_string(), "2026-02-28");
        assert_eq!(periods[1].start.to_string(), "2026-02-28");
        assert_eq!(periods[1].end.to_string(), "2026-03-31");
        assert_eq!(periods[2].end.to_string(), "2026-04-11");
        assert_eq!(periods[2].days, 11);
        assert!(periods[2].residual);
        civil.periods.pop();
        assert_eq!(civil.calendar_periods(), Err(BudgetError::Schedule));
    }

    #[test]
    fn financial_schedule_preserves_each_group_and_stable_residual_cents() {
        let b = budget();
        let totals = b.summarize().unwrap();
        let mut schedule = b.schedule.unwrap();
        let allocation = schedule.financial_allocation(totals.total).unwrap();
        assert_eq!(
            allocation
                .parts
                .iter()
                .map(|part| Money::new(part.value).unwrap().to_string())
                .collect::<Vec<_>>(),
            vec!["76.68", "76.66", "76.66"]
        );
        for group in totals.groups {
            let allocated = schedule.financial_allocation(group.amount).unwrap();
            assert_eq!(
                allocated
                    .parts
                    .iter()
                    .try_fold(ExactDecimal::ZERO, |sum, part| sum.checked_add(part.value))
                    .unwrap(),
                group.amount.value()
            );
        }
        let cent: Money = "0.01".parse().unwrap();
        let original = schedule.financial_allocation(cent).unwrap();
        assert_eq!(original.parts[0].value, cent.value());
        assert_eq!(original.parts[1].value, ExactDecimal::ZERO);
        schedule.periods.reverse();
        assert_eq!(schedule.financial_allocation(cent).unwrap(), original);
        schedule.periods.clear();
        assert_eq!(
            schedule.financial_allocation(cent),
            Err(BudgetError::Schedule)
        );
    }

    #[test]
    fn every_snapshot_must_match_immutable_catalog_prices() {
        let original = budget();
        let mut official = BTreeMap::new();
        official.insert(
            ("ref".into(), 1, "SIN-A".into()),
            (MeasureUnit::CubicMetre, "100".parse().unwrap()),
        );
        original.validate_catalog_references(&official).unwrap();
        let mut altered = original.clone();
        altered.lines[0].price = "999".parse().unwrap();
        assert_eq!(
            altered.validate_catalog_references(&official),
            Err(BudgetError::ReferencedItem)
        );
        altered = original.clone();
        altered.lines[0].code = "SIN-B".into();
        assert_eq!(
            altered.validate_catalog_references(&official),
            Err(BudgetError::ReferencedItem)
        );
        altered = original.clone();
        altered.lines[0].unit = MeasureUnit::SquareMetre;
        assert_eq!(
            altered.validate_catalog_references(&official),
            Err(BudgetError::ReferencedItem)
        );
        altered = original;
        altered.lines[0].catalog_edition = Some(2);
        assert_eq!(
            altered.validate_catalog_references(&official),
            Err(BudgetError::ReferencedItem)
        );
    }
    #[test]
    fn snapshot_save_cannot_strip_or_replace_existing_catalog_provenance() {
        let old = budget();
        let mut changed = old.clone();
        changed.lines[0].catalog_id = None;
        changed.lines[0].catalog_edition = None;
        changed.lines[0].price = "999".parse().unwrap();
        assert_eq!(
            changed.validate_catalog_lineage(&old),
            Err(BudgetError::ReferencedItem)
        );
        changed = old.clone();
        changed.lines[0].catalog_edition = Some(2);
        assert_eq!(
            changed.validate_catalog_lineage(&old),
            Err(BudgetError::ReferencedItem)
        );
        changed = old.clone();
        changed.lines[0].quantity = "3".parse().unwrap();
        changed.validate_catalog_lineage(&old).unwrap();
        changed.lines[1].price = "45".parse().unwrap();
        changed.validate_catalog_lineage(&old).unwrap();
    }
    #[test]
    fn editing_service_preserves_schedule_and_historical_catalog_price() {
        let mut draft = budget();
        let original = draft.clone();
        assert_eq!(
            draft.edit_line_values("l-1", "3".parse().unwrap(), "200".parse().unwrap()),
            Err(BudgetError::ReferencedItem)
        );
        assert_eq!(draft, original);
        draft
            .edit_line_values("l-1", "3".parse().unwrap(), "100".parse().unwrap())
            .unwrap();
        assert_eq!(draft.summarize().unwrap().total.to_string(), "330.00");
        assert_eq!(draft.schedule, original.schedule);
        draft
            .edit_line_values("l-2", "2".parse().unwrap(), "40".parse().unwrap())
            .unwrap();
        assert_eq!(draft.summarize().unwrap().total.to_string(), "380.00");
        let before_missing = draft.clone();
        assert_eq!(
            draft.edit_line_values("absente", "1".parse().unwrap(), "1".parse().unwrap()),
            Err(BudgetError::Structure)
        );
        assert_eq!(draft, before_missing);
        draft.status = BudgetState::Approved;
        assert_eq!(
            draft.edit_line_values("l-2", "1".parse().unwrap(), "1".parse().unwrap()),
            Err(BudgetError::Definitive)
        );
    }
    #[test]
    fn copying_service_preserves_catalog_provenance_schedule_and_atomicity() {
        let mut draft = budget();
        let original = draft.clone();
        draft.copy_line("l-1", "l-3", "v-02").unwrap();
        assert_eq!(draft.lines.len(), 3);
        assert_eq!(draft.lines[2].id, "l-3");
        assert_eq!(draft.lines[2].variation_id, "v-02");
        assert_eq!(draft.lines[2].catalog_id, original.lines[0].catalog_id);
        assert_eq!(
            draft.lines[2].catalog_edition,
            original.lines[0].catalog_edition
        );
        assert_eq!(draft.lines[2].price, original.lines[0].price);
        assert_eq!(draft.schedule, original.schedule);
        assert_eq!(draft.summarize().unwrap().total.to_string(), "430.00");

        let before_failure = draft.clone();
        for (source, id, variation) in [
            ("l-1", "l-3", "v-01"),
            ("missing", "new", "v-01"),
            ("l-1", "new", "missing"),
            ("l-1", "=1+1", "v-01"),
        ] {
            assert!(draft.copy_line(source, id, variation).is_err());
            assert_eq!(draft, before_failure);
        }
        draft.status = BudgetState::Approved;
        let before_approval = draft.clone();
        assert_eq!(
            draft.copy_line("l-1", "l-4", "v-02"),
            Err(BudgetError::Definitive)
        );
        assert_eq!(draft, before_approval);
    }
    #[test]
    fn moving_service_is_atomic_and_does_not_reprice_or_discard_schedule() {
        let mut draft = budget();
        let original = draft.clone();
        draft.move_line("l-1", "v-02").unwrap();
        assert_eq!(draft.lines[0].variation_id, "v-02");
        assert_eq!(draft.lines[0].price, original.lines[0].price);
        assert_eq!(draft.lines[0].catalog_id, original.lines[0].catalog_id);
        assert_eq!(
            draft.lines[0].catalog_edition,
            original.lines[0].catalog_edition
        );
        assert_eq!(draft.schedule, original.schedule);
        assert_eq!(draft.summarize().unwrap().total.to_string(), "230.00");

        let stable = draft.clone();
        assert_eq!(
            draft.move_line("l-1", "unknown"),
            Err(BudgetError::Structure)
        );
        assert_eq!(
            draft.move_line("unknown", "v-02"),
            Err(BudgetError::Structure)
        );
        assert_eq!(draft, stable);
        draft.status = BudgetState::Approved;
        assert_eq!(draft.move_line("l-1", "v-01"), Err(BudgetError::Definitive));
    }
    #[test]
    fn whole_snapshot_cannot_move_services_without_explicit_command() {
        let original = budget();
        let mut altered = original.clone();
        altered.lines[0].variation_id = "v-02".into();
        assert_eq!(
            altered.validate_no_implicit_moves(&original),
            Err(BudgetError::ReferencedItem)
        );
        // Trocar a obra de uma variação antiga moveria todos os seus serviços
        // sem jamais alterar o identificador de variação de cada item.
        let mut retargeted = original.clone();
        retargeted.variations[0].work_id = entity(12);
        assert_eq!(
            retargeted.validate_no_implicit_moves(&original),
            Err(BudgetError::ReferencedItem)
        );
        // Alterar o escopo mantendo a obra também altera implicitamente a
        // classificação de todos os serviços vinculados à variação.
        let mut relabeled = original.clone();
        relabeled.variations[0].scope = "Outro escopo".into();
        relabeled.validate().unwrap();
        assert_eq!(
            relabeled.validate_no_implicit_moves(&original),
            Err(BudgetError::ReferencedItem)
        );
        // Alterações normais de quantidade e inclusões não são proibidas.
        let mut quantity = original.clone();
        quantity.lines[0].quantity = "4".parse().unwrap();
        quantity.validate_no_implicit_moves(&original).unwrap();
        quantity.copy_line("l-1", "l-3", "v-02").unwrap();
        quantity.validate_no_implicit_moves(&original).unwrap();
        // A operação explícita continua autorizada no domínio, mas um Save
        // forjado com o mesmo resultado não pode substituí-la.
        let mut moved = original.clone();
        moved.move_line("l-1", "v-02").unwrap();
        assert_eq!(moved.lines[0].variation_id, "v-02");
        assert_eq!(
            moved.validate_no_implicit_moves(&original),
            Err(BudgetError::ReferencedItem)
        );
        let mut missing = original.clone();
        missing.lines.retain(|line| line.id != "l-1");
        assert_eq!(
            missing.validate_no_implicit_removals(&original),
            Err(BudgetError::ReferencedItem)
        );
    }
    #[test]
    fn snapshot_cannot_delete_or_replace_an_empty_variation() {
        let mut original = budget();
        original.variations.push(BudgetVariation {
            id: "v-empty".into(),
            work_id: entity(99),
            scope: "Variação sem serviços".into(),
        });
        original.validate().unwrap();

        // A ausência de serviços não prova ausência de vínculos externos.
        let mut deleted = original.clone();
        deleted
            .variations
            .retain(|variation| variation.id != "v-empty");
        deleted.validate().unwrap();
        assert_eq!(
            deleted.validate_no_implicit_removals(&original),
            Err(BudgetError::ReferencedItem)
        );

        let mut replaced = deleted.clone();
        replaced.variations.push(BudgetVariation {
            id: "v-replacement".into(),
            work_id: entity(99),
            scope: "Variação sem serviços".into(),
        });
        replaced.validate().unwrap();
        assert_eq!(
            replaced.validate_no_implicit_removals(&original),
            Err(BudgetError::ReferencedItem)
        );

        let mut added = original.clone();
        added.variations.push(BudgetVariation {
            id: "v-additional".into(),
            work_id: entity(100),
            scope: "Variação adicional".into(),
        });
        added.validate().unwrap();
        added.validate_no_implicit_removals(&original).unwrap();
        assert_eq!(original.variations.len(), 3);
    }
    #[test]
    fn deletion_requires_explicit_command_and_preserves_original_on_failure() {
        let original = budget();
        let mut edited = original.clone();
        edited.lines.retain(|line| line.id != "l-2");
        assert_eq!(
            edited.validate_no_implicit_removals(&original),
            Err(BudgetError::ReferencedItem)
        );
        let mut snapshot = original.clone();
        assert_eq!(
            snapshot.remove_unlinked_line("l-1"),
            Err(BudgetError::ReferencedItem)
        );
        assert_eq!(
            snapshot.remove_unlinked_line("missing"),
            Err(BudgetError::Structure)
        );
        assert_eq!(snapshot, original);
        snapshot.remove_unlinked_line("l-2").unwrap();
        assert_eq!(snapshot.lines.len(), 1);
        assert_eq!(snapshot.lines[0].id, "l-1");
        assert_eq!(snapshot.schedule, original.schedule);
        assert_eq!(snapshot.summarize().unwrap().total.to_string(), "200.00");
        assert_eq!(
            snapshot.validate_no_implicit_removals(&original),
            Err(BudgetError::ReferencedItem)
        );
        let mut approved = original.clone();
        approved.status = BudgetState::Approved;
        assert_eq!(
            approved.remove_unlinked_line("l-2"),
            Err(BudgetError::Definitive)
        );
    }
    #[test]
    fn fixed_calendar_periods_cover_the_deadline_including_partial_last_period() {
        let starts_on: CivilDate = "2026-11-01".parse().unwrap();
        let basis = PeriodBasis::Fixed30Days;
        for (days, count) in [(1, 1), (30, 1), (31, 2), (90, 3), (91, 4)] {
            let schedule = BudgetSchedule::linear(starts_on, days, basis, count).unwrap();
            schedule.validate().unwrap();
            assert_eq!(schedule.periods.len(), count as usize);
            assert_eq!(
                BudgetSchedule::linear(starts_on, days, basis, count + 1),
                Err(BudgetError::Schedule)
            );
        }
        let mut original = budget();
        original.schedule.as_mut().unwrap().deadline_days = 91;
        assert_eq!(original.validate(), Err(BudgetError::Schedule));
    }

    #[test]
    fn civil_periods_use_original_anchor_and_reject_unrepresentable_end_dates() {
        let start = "2024-01-31".parse().unwrap();
        let basis = PeriodBasis::CivilMonths;
        for (days, count) in [(29, 1), (30, 2), (60, 2), (61, 3)] {
            let schedule = BudgetSchedule::linear(start, days, basis, count).unwrap();
            schedule.validate().unwrap();
            assert_eq!(
                BudgetSchedule::linear(start, days, basis, count + 1),
                Err(BudgetError::Schedule)
            );
        }
        let mut schedule = BudgetSchedule::linear(start, 60, basis, 2).unwrap();
        schedule.deadline_days = 61;
        assert_eq!(schedule.validate(), Err(BudgetError::Schedule));

        let last_day = "9999-12-31".parse().unwrap();
        for basis in [PeriodBasis::Fixed30Days, PeriodBasis::CivilMonths] {
            assert_eq!(
                BudgetSchedule::linear(last_day, 1, basis, 1),
                Err(BudgetError::Schedule)
            );
        }
        let start = "9999-12-30".parse().unwrap();
        BudgetSchedule::linear(start, 1, basis, 1).unwrap();
    }

    #[test]
    fn schedule_rejects_position_gaps_without_changing_a_valid_rateio() {
        let original = budget();
        let mut missing_first = original.clone();
        for period in &mut missing_first.schedule.as_mut().unwrap().periods {
            period.position += 1;
        }
        assert_eq!(missing_first.validate(), Err(BudgetError::Schedule));

        let mut distant_period = original.clone();
        distant_period.schedule.as_mut().unwrap().periods[2].position = 99;
        assert_eq!(distant_period.validate(), Err(BudgetError::Schedule));
        assert_eq!(distant_period.summarize(), Err(BudgetError::Schedule));

        // A ordem física dos registros não altera sua posição temporal.
        let mut reordered = original.clone();
        reordered.schedule.as_mut().unwrap().periods.swap(0, 2);
        reordered.validate().unwrap();
        assert_eq!(
            reordered.summarize().unwrap(),
            original.summarize().unwrap()
        );
    }

    #[test]
    fn snapshots_cannot_bypass_explicit_schedule_confirmation() {
        let original = budget();
        let mut removed = original.clone();
        removed.schedule = None;
        assert_eq!(
            removed.validate_no_implicit_reschedule(&original),
            Err(BudgetError::Schedule)
        );

        let mut changed = original.clone();
        changed
            .reschedule_linear(
                "2026-12-01".parse().unwrap(),
                120,
                PeriodBasis::CivilMonths,
                4,
            )
            .unwrap();
        assert_eq!(
            changed.validate_no_implicit_reschedule(&original),
            Err(BudgetError::Schedule)
        );

        let mut ordinary_edit = original.clone();
        ordinary_edit.lines[1].quantity = "3".parse().unwrap();
        ordinary_edit
            .validate_no_implicit_reschedule(&original)
            .unwrap();
    }
    #[test]
    fn rescheduling_is_atomic_keeps_costs_and_rejects_definitive_versions() {
        let mut draft = budget();
        let original = draft.clone();
        assert_eq!(
            draft.reschedule_linear(
                "2026-12-01".parse().unwrap(),
                0,
                PeriodBasis::CivilMonths,
                4,
            ),
            Err(BudgetError::Schedule)
        );
        assert_eq!(draft, original);
        draft
            .reschedule_linear(
                "2026-12-01".parse().unwrap(),
                120,
                PeriodBasis::CivilMonths,
                4,
            )
            .unwrap();
        assert_eq!(draft.lines, original.lines);
        assert_eq!(draft.variations, original.variations);
        assert_eq!(draft.summarize().unwrap(), original.summarize().unwrap());
        let schedule = draft.schedule.as_ref().unwrap();
        assert_eq!(schedule.starts_on.to_string(), "2026-12-01");
        assert_eq!(schedule.periods.len(), 4);
        assert_eq!(
            schedule.periods[0].percentage.value(),
            "25".parse().unwrap()
        );
        let stable = draft.clone();
        assert_eq!(
            draft.reschedule_linear(
                "2026-12-01".parse().unwrap(),
                120,
                PeriodBasis::CivilMonths,
                0,
            ),
            Err(BudgetError::Schedule)
        );
        assert_eq!(draft, stable);
        draft.status = BudgetState::Approved;
        assert_eq!(
            draft.reschedule_linear(
                "2026-12-01".parse().unwrap(),
                120,
                PeriodBasis::CivilMonths,
                4,
            ),
            Err(BudgetError::Definitive)
        );
    }
    #[test]
    fn stale_reservation_and_referenced_item_cannot_delete() {
        let mut b = budget();
        let lease = EditLease {
            actor: actor(1),
            session: entity(8),
            revision: 1,
            valid_until: "2026-11-21T16:00:00Z".parse().unwrap(),
        };
        let now = "2026-11-21T15:00:00Z".parse().unwrap();
        assert_eq!(
            b.remove_line("l-1", true, &lease, actor(1), entity(8), now),
            Err(BudgetError::ReferencedItem)
        );
        assert_eq!(
            b.remove_line("l-1", false, &lease, actor(2), entity(8), now),
            Err(BudgetError::Lease)
        );
        assert_eq!(
            b.remove_line(
                "l-1",
                false,
                &lease,
                actor(1),
                entity(8),
                "2026-11-21T16:00:00Z".parse().unwrap()
            ),
            Err(BudgetError::Lease)
        );
        let original = b.clone();
        assert_eq!(
            b.remove_line("l-1", false, &lease, actor(1), entity(8), now),
            Err(BudgetError::ReferencedItem)
        );
        assert_eq!(b, original);
        b.remove_line("l-2", false, &lease, actor(1), entity(8), now)
            .unwrap();
        assert_eq!(b.lines.len(), 1);
        assert_eq!(b.lines[0].id, "l-1");
        assert_eq!(b.schedule, original.schedule);
    }
}
