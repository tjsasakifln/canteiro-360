use crate::reliability::ReliabilityScope;
pub use canteiro_domain::{
    budget_csv::{ImportDifference,NumberLocale},
    budgets::{BudgetRevision,BudgetState,BudgetSummary,BudgetSchedule},
    catalogs::{CatalogEdition,CostSource,CatalogItem},
    compositions::{Composition,CostMemory},
};
use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct EstimatingScope{
    pub id:String,
    pub name:String,
    pub can_edit:bool,
    pub can_review:bool,
    pub can_approve:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CatalogHeader{
    pub scope_id:String,
    pub catalog_id:String,
    pub edition:u32,
    pub source:CostSource,
    pub published_on:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct BudgetRecord{
    pub scope_id:String,
    pub version:i64,
    pub snapshot:BudgetRevision,
    pub actor_id:String,
    pub reason:String,
    pub recorded_at:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CompositionRecord{
    pub scope_id:String,
    pub code:String,
    pub version:i64,
    pub composition:Composition,
    pub actor_id:String,
    pub reason:String,
    pub recorded_at:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct EstimatingContext{
    pub csrf:String,
    pub context:Option<ReliabilityScope>,
    pub organization_name:String,
    pub scopes:Vec<EstimatingScope>,
    pub catalogs:Vec<CatalogHeader>,
    pub budgets:Vec<BudgetRecord>,
    pub compositions:Vec<CompositionRecord>,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogPublishCommand{
    pub context:ReliabilityScope,
    pub scope_id:String,
    pub edition:CatalogEdition,
    pub reason:String,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionWriteCommand{
    pub context:ReliabilityScope,
    pub scope_id:String,
    pub operation_key:String,
    pub expected_version:Option<i64>,
    pub reason:String,
    pub composition:Composition,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetLeaseCommand{
    pub context:ReliabilityScope,
    pub scope_id:String,
    pub lease_key:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct BudgetLeaseReply{
    pub scope_id:String,
    pub revision:u32,
    pub lease_key:String,
    pub valid_until:String,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetCommand{
    pub context:ReliabilityScope,
    pub scope_id:String,
    pub operation_key:String,
    pub expected_version:Option<i64>,
    pub lease_key:Option<String>,
    pub reason:String,
    pub change:BudgetChange,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(tag="action",rename_all="snake_case",deny_unknown_fields)]
pub enum BudgetChange{
    Save{snapshot:BudgetRevision},
    Approve,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct BudgetReply{
    pub budget:BudgetRecord,
    pub total:BudgetSummary,
    pub repeated:bool,
}
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetImportPreviewCommand{
    pub context:ReliabilityScope,
    pub scope_id:String,
    pub locale:NumberLocale,
    pub csv:String,
}
#[derive(Clone,Debug,Serialize)]
pub struct BudgetImportPreviewReply{
    pub snapshot:BudgetRevision,
    pub difference:ImportDifference,
    pub total:BudgetSummary,
    pub base_version:Option<i64>,
    pub format:&'static str,
}
