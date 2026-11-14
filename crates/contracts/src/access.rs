//! Configuração de direitos e decisões de alçada. Nenhuma resposta é um token de autorização.
use crate::organizations::{MemberSummary, OrganizationScope, OrganizationSummary};
pub use canteiro_domain::access::{AccessAction, AccessRole, ScopeKind};
use canteiro_domain::decimal::Money;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum AccessCommand {
    CreateScope {
        context: OrganizationScope,
        kind: ScopeKind,
        name: String,
        parent_id: Option<String>,
    },
    Grant {
        context: OrganizationScope,
        scope_id: String,
        user_email: String,
        role: AccessRole,
        actions: Vec<AccessAction>,
        max_amount: Money,
        valid_from: String,
        valid_until: Option<String>,
        required: bool,
    },
    Delegate {
        context: OrganizationScope,
        source_grant_id: String,
        source_version: i64,
        user_email: String,
        actions: Vec<AccessAction>,
        max_amount: Money,
        valid_from: String,
        valid_until: String,
        substitution: bool,
        reason: String,
    },
    RevokeGrant {
        context: OrganizationScope,
        grant_id: String,
        grant_version: i64,
    },
    SetPolicy {
        context: OrganizationScope,
        scope_id: String,
        action: AccessAction,
        threshold: Money,
        approvals_above_threshold: u8,
        segregate_proposer: bool,
        expected_version: Option<i64>,
    },
    Propose {
        context: OrganizationScope,
        scope_id: String,
        action: AccessAction,
        title: String,
        increase: Money,
        decrease: Money,
        conflicted_user_ids: Vec<String>,
    },
    ReviseRequest {
        context: OrganizationScope,
        request_id: String,
        request_version: i64,
        title: String,
        increase: Money,
        decrease: Money,
        conflicted_user_ids: Vec<String>,
    },
    Decide {
        context: OrganizationScope,
        request_id: String,
        request_version: i64,
        grant_id: String,
        grant_version: i64,
        approve: bool,
    },
}

impl AccessCommand {
    /// Atos monetários que aceitam a identidade de operação no transporte.
    pub fn authority_act(&self) -> Option<&'static str> {
        match self {
            Self::Propose { .. } => Some("authority_propose"),
            Self::ReviseRequest { .. } => Some("authority_revise"),
            Self::Decide { .. } => Some("authority_decide"),
            _ => None,
        }
    }

    pub fn context(&self) -> &OrganizationScope {
        match self {
            Self::CreateScope { context, .. }
            | Self::Grant { context, .. }
            | Self::Delegate { context, .. }
            | Self::RevokeGrant { context, .. }
            | Self::SetPolicy { context, .. }
            | Self::Propose { context, .. }
            | Self::ReviseRequest { context, .. }
            | Self::Decide { context, .. } => context,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScopeView {
    pub id: String,
    pub kind: ScopeKind,
    pub name: String,
    pub parent_id: Option<String>,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct GrantView {
    pub id: String,
    pub scope_id: String,
    pub user_id: String,
    pub user_email: String,
    pub role: AccessRole,
    pub actions: Vec<AccessAction>,
    pub max_amount: Money,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub effective: bool,
    pub revoked: bool,
    pub source_grant_id: Option<String>,
    pub substitution: bool,
    pub reason: String,
    pub required: bool,
    pub version: i64,
    pub can_delegate: bool,
    pub can_revoke: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PolicyView {
    pub scope_id: String,
    pub action: AccessAction,
    pub threshold: Money,
    pub approvals_above_threshold: u8,
    pub segregate_proposer: bool,
    pub version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccessExplanation {
    pub code: String,
    pub message: String,
    pub next_step: String,
    pub resolvers: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ApprovalView {
    pub user_email: String,
    pub approved: bool,
    pub decided_at: String,
    pub request_version: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RequestView {
    pub id: String,
    pub scope_id: String,
    pub title: String,
    pub action: AccessAction,
    pub proposer_id: String,
    pub proposer_email: String,
    pub conflicted_user_ids: Vec<String>,
    pub increase: Money,
    pub decrease: Money,
    pub authority_amount: Money,
    pub contains_increase: bool,
    pub status: String,
    pub version: i64,
    pub policy_version: i64,
    pub policy_changed: bool,
    pub required_approvals: u8,
    pub valid_approvals: u8,
    pub approvals: Vec<ApprovalView>,
    pub allowed_grant_ids: Vec<String>,
    pub can_revise: bool,
    pub denial: Option<AccessExplanation>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccessState {
    pub current_user_id: String,
    pub organization: Option<OrganizationSummary>,
    pub context_version: i64,
    pub mfa_verified: bool,
    pub can_manage: bool,
    pub scopes: Vec<ScopeView>,
    pub members: Vec<MemberSummary>,
    pub grants: Vec<GrantView>,
    pub policies: Vec<PolicyView>,
    pub requests: Vec<RequestView>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessResponse {
    pub state: AccessState,
    pub csrf: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<AuthorityReceipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthorityReceipt {
    pub event_id: String,
    pub request_id: String,
    pub version: i64,
    pub status: String,
    pub valid_approvals: u8,
    pub recorded_at: String,
}
