use crate::{projects::RecordChoice, reliability::ReliabilityScope};
pub use canteiro_domain::obligations::{
    ObligationKind, ObligationSpec, RrtEvent, RrtFramework, RrtTiming, TechnicalActivity,
    TechnicalException,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationCommand {
    pub context: ReliabilityScope,
    pub operation_key: String,
    pub scope_id: String,
    pub obligation_id: Option<String>,
    pub expected_version: Option<i64>,
    pub reason: String,
    pub spec: ObligationSpec,
    pub confirm: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ObligationRecord {
    pub id: String,
    pub scope_id: String,
    pub version: i64,
    pub spec: ObligationSpec,
    pub confirmed: bool,
    pub document_sha256: Option<String>,
    pub professional_name: Option<String>,
    pub replaced_by: Option<String>,
    pub actor_id: String,
    pub recorded_at: String,
    pub reason: String,
    pub can_edit: bool,
    pub can_exception: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TechnicalScope {
    pub id: String,
    pub name: String,
    pub can_edit: bool,
    pub can_exception: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DocumentChoice {
    pub document_id: String,
    pub version: i64,
    pub scope_id: String,
    pub label: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ObligationsContext {
    pub csrf: String,
    pub context: Option<ReliabilityScope>,
    pub organization_name: String,
    pub today: String,
    pub scopes: Vec<TechnicalScope>,
    pub obligations: Vec<ObligationRecord>,
    pub professionals: Vec<RecordChoice>,
    pub documents: Vec<DocumentChoice>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ObligationReply {
    pub obligation: ObligationRecord,
    pub repeated: bool,
}
