//! Edição concorrente e trilha de referência contratual.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReliabilityScope {
    pub organization_id: String,
    pub context_version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReliabilityContext {
    pub csrf: String,
    pub actor_id: String,
    pub actor_label: String,
    pub organization: Option<ReliabilityOrganization>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ReliabilityOrganization {
    pub scope: ReliabilityScope,
    pub name: String,
    pub contracts: Vec<ContractSummary>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractSummary {
    pub id: String,
    pub title: String,
    pub version: i64,
    pub can_edit: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ContractDetail {
    pub contract: ContractSummary,
    pub lease_until: Option<String>,
    pub history: Vec<HistoryEntry>,
    pub next_after: Option<i64>,
    pub notifications: Vec<HistoryNotification>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub sequence: i64,
    pub event_id: String,
    pub actor_id: String,
    pub actor_label: String,
    pub version: i64,
    pub origin: String,
    pub action: String,
    pub before: Option<String>,
    pub after: String,
    pub reason: String,
    pub occurred_at: String,
    pub recorded_at: String,
    pub late: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HistoryNotification {
    pub event_id: String,
    pub version: i64,
    pub recorded_at: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum ReliabilityCommand {
    Acquire {
        scope: ReliabilityScope,
        contract_id: String,
        lease_id: String,
    },
    Renew {
        scope: ReliabilityScope,
        contract_id: String,
        lease_id: String,
    },
    Release {
        scope: ReliabilityScope,
        contract_id: String,
        lease_id: String,
    },
    Save {
        scope: ReliabilityScope,
        contract_id: String,
        lease_id: String,
        key: String,
        expected_version: i64,
        title: String,
        reason: String,
        occurred_at: Option<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SaveReceipt {
    pub event_id: String,
    pub contract_id: String,
    pub title: String,
    pub version: i64,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ReliabilityReply {
    Lease { expires_at: Option<String> },
    Saved { receipt: SaveReceipt },
    VersionConflict { current: ContractSummary },
}
