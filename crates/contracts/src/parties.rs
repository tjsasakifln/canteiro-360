//! Cadastros de partes. Dados bancários não integram consultas ou exportações comuns.
use crate::organizations::OrganizationScope;
pub use canteiro_domain::parties::{
    BankAccount, Contact, PartyInput, PartyKind, PartyRole, Professional,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PartyView {
    pub id: String,
    pub version: i64,
    pub input: PartyInput,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct PartiesContext {
    pub csrf: String,
    pub scope: Option<OrganizationScope>,
    pub organization_name: String,
    pub can_manage: bool,
    pub rows: Vec<PartyView>,
    pub total: i64,
    pub query: String,
    pub page: u32,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum PartyCommand {
    Save {
        context: OrganizationScope,
        key: String,
        id: Option<String>,
        expected_version: Option<i64>,
        input: PartyInput,
        reason: String,
    },
    Import {
        context: OrganizationScope,
        key: String,
        rows: Vec<PartyInput>,
    },
    SetBank {
        context: OrganizationScope,
        key: String,
        id: String,
        expected_version: i64,
        account: BankAccount,
        reason: String,
    },
}
impl PartyCommand {
    /// Stable intent for a retry after refreshing the authorized organization context.
    /// The tenant remains part of the intent; the operation key belongs to the receipt.
    pub fn idempotency_payload(&self) -> Self {
        let mut value = self.clone();
        let (context, key) = match &mut value {
            Self::Save { context, key, .. }
            | Self::Import { context, key, .. }
            | Self::SetBank { context, key, .. } => (context, key),
        };
        context.context_version = 0;
        context.organization_version = 0;
        key.clear();
        value
    }
    pub fn context(&self) -> &OrganizationScope {
        match self {
            Self::Save { context, .. }
            | Self::Import { context, .. }
            | Self::SetBank { context, .. } => context,
        }
    }
    pub fn key(&self) -> &str {
        match self {
            Self::Save { key, .. } | Self::Import { key, .. } | Self::SetBank { key, .. } => key,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PartyReceipt {
    pub ids: Vec<String>,
    pub versions: Vec<i64>,
    pub reused: usize,
    pub message: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct BankVersion {
    pub version: i64,
    pub account: BankAccount,
    pub actor: String,
    pub recorded_at: String,
    pub reason: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct BankHistory {
    pub party_id: String,
    pub party_version: i64,
    pub versions: Vec<BankVersion>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PartyRevision {
    pub version: i64,
    pub before: Option<PartyInput>,
    pub after: PartyInput,
    pub actor: String,
    pub recorded_at: String,
    pub reason: String,
}
