use crate::organizations::OrganizationScope;
pub use canteiro_domain::portfolio::{AssetOwnership, LocalityAllocation, WorkInput, WorkStatus};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorkView {
    pub id: String,
    pub version: i64,
    pub input: WorkInput,
    pub can_edit: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WorkScope {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PortfolioContext {
    pub csrf: String,
    pub scope: Option<OrganizationScope>,
    pub organization_name: String,
    pub available_scopes: Vec<WorkScope>,
    pub contracts: Vec<WorkScope>,
    pub rows: Vec<WorkView>,
    pub total: usize,
    pub located: usize,
    pub query: String,
    pub status: Option<WorkStatus>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortfolioCommand {
    pub context: OrganizationScope,
    pub key: String,
    pub work_id: String,
    pub expected_version: Option<i64>,
    pub input: WorkInput,
    pub reason: String,
}
impl PortfolioCommand {
    /// Keep the same intent when only the authorized context has been refreshed.
    pub fn idempotency_payload(&self) -> Self {
        let mut value = self.clone();
        value.context.context_version = 0;
        value.context.organization_version = 0;
        value.key.clear();
        value
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PortfolioReceipt {
    pub id: String,
    pub version: i64,
    pub message: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkRevision {
    pub version: i64,
    pub before: Option<WorkInput>,
    pub after: WorkInput,
    pub actor: String,
    pub recorded_at: String,
    pub reason: String,
}
