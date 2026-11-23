use crate::reliability::ReliabilityScope;
use canteiro_domain::CivilDate;
pub use canteiro_domain::{
    budget_csv::{ImportDifference, ImportLineChanges, NumberLocale},
    budgets::{BudgetRevision, BudgetSchedule, BudgetState, BudgetSummary},
    catalogs::{CatalogComparison, CatalogEdition, CatalogItem, CatalogItemChange, CostSource},
    compositions::{Composition, CostMemory},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EstimatingScope {
    pub id: String,
    pub name: String,
    pub can_edit: bool,
    pub can_review: bool,
    pub can_approve: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogHeader {
    pub scope_id: String,
    pub catalog_id: String,
    pub edition: u32,
    pub source: CostSource,
    pub published_on: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetRecord {
    pub scope_id: String,
    pub version: i64,
    pub snapshot: BudgetRevision,
    /// Identificador persistido da operação, para rastrear o recibo sem expor dados internos.
    #[serde(default)]
    pub operation_key: String,
    pub actor_id: String,
    pub reason: String,
    pub recorded_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompositionRecord {
    pub scope_id: String,
    pub code: String,
    pub version: i64,
    pub composition: Composition,
    pub actor_id: String,
    pub reason: String,
    pub recorded_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EstimatingContext {
    pub csrf: String,
    pub context: Option<ReliabilityScope>,
    pub organization_name: String,
    pub scopes: Vec<EstimatingScope>,
    pub catalogs: Vec<CatalogHeader>,
    pub budgets: Vec<BudgetRecord>,
    pub compositions: Vec<CompositionRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCsvImportCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub catalog_id: String,
    pub edition: u32,
    pub published_on: String,
    pub source: CostSource,
    pub reason: String,
    pub csv: String,
    pub locale: NumberLocale,
}

#[derive(Clone, Debug, Serialize)]
pub struct CatalogCsvPreviewReply {
    pub header: CatalogHeader,
    pub items_total: usize,
    pub sample: Vec<CatalogItem>,
    pub previous_edition: Option<u32>,
    pub included: usize,
    pub changed: usize,
    pub removed: usize,
    /// Amostras limitadas; as contagens consideram todas as linhas importadas.
    pub added_sample: Vec<CatalogItem>,
    pub changed_sample: Vec<CatalogItemChange>,
    pub removed_sample: Vec<CatalogItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPublishCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub edition: CatalogEdition,
    pub reason: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionWriteCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub operation_key: String,
    pub expected_version: Option<i64>,
    pub reason: String,
    pub composition: Composition,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLeaseCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub lease_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetLeaseReply {
    pub scope_id: String,
    pub revision: u32,
    pub lease_key: String,
    pub valid_until: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub operation_key: String,
    pub expected_version: Option<i64>,
    pub lease_key: Option<String>,
    pub reason: String,
    pub change: BudgetChange,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum BudgetChange {
    Save {
        snapshot: BudgetRevision,
    },
    RemoveLine {
        line_id: String,
    },
    /// Copia o serviço existente sem aceitar preço ou referência fornecidos pelo cliente.
    CopyLine {
        source_id: String,
        new_id: String,
        target_variation_id: String,
    },
    /// Move um serviço existente, preservando identidade, preço e fonte histórica.
    MoveLine {
        line_id: String,
        target_variation_id: String,
    },
    /// Altera somente o cronograma no servidor, sem aceitar serviços ou preços no pedido.
    RescheduleLinear {
        starts_on: CivilDate,
        deadline_days: u32,
        basis: canteiro_domain::budgets::PeriodBasis,
        periods: u32,
    },
    Approve,
}
#[derive(Clone, Debug, Serialize)]
pub struct BudgetReply {
    pub budget: BudgetRecord,
    pub total: BudgetSummary,
    pub repeated: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetImportPreviewCommand {
    pub context: ReliabilityScope,
    pub scope_id: String,
    pub locale: NumberLocale,
    pub csv: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct BudgetImportPreviewReply {
    pub snapshot: BudgetRevision,
    pub difference: ImportDifference,
    pub change_details: ImportLineChanges,
    pub total: BudgetSummary,
    pub base_version: Option<i64>,
    /// Identificador da previsualização persistida, quando já existe revisão-base.
    pub batch_id: Option<String>,
    pub format: &'static str,
}
