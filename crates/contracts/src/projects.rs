use crate::reliability::ReliabilityScope;
pub use canteiro_domain::projects::{ProjectDelivery, ProjectSnapshot, ProjectSpec, ProjectStatus};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCommand {
    pub context: ReliabilityScope,
    pub operation_key: String,
    pub project_id: Option<String>,
    pub expected_version: Option<i64>,
    pub reason: String,
    pub change: ProjectChange,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProjectChange {
    Save { spec: ProjectSpec },
    SetDelivery { delivery: ProjectDelivery },
    RemoveDelivery { delivery_id: String },
    Approve,
    Start,
    Accept { delivery_id: String },
    Close,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecordChoice {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectRecord {
    pub id: String,
    pub version: i64,
    pub snapshot: ProjectSnapshot,
    pub recorded_at: String,
    pub actor_id: String,
    pub reason: String,
    pub can_edit: bool,
    pub can_approve: bool,
    pub can_accept: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectsContext {
    pub csrf: String,
    pub context: Option<ReliabilityScope>,
    pub organization_name: String,
    pub can_create: bool,
    pub projects: Vec<ProjectRecord>,
    pub professionals: Vec<RecordChoice>,
    pub parties: Vec<RecordChoice>,
    pub works: Vec<RecordChoice>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectReply {
    pub project: ProjectRecord,
    pub repeated: bool,
}
