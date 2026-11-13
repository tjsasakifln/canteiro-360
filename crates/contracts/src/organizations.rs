//! Contratos de organizações. Códigos transitórios não implementam Debug.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OrganizationRole {
    Administrator,
    Member,
}
impl OrganizationRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Administrator => "administrator",
            Self::Member => "member",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Administrator => "Administrador",
            Self::Member => "Membro",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OrganizationScope {
    pub organization_id: String,
    pub context_version: i64,
    pub organization_version: i64,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum OrganizationCommand {
    Create {
        name: String,
    },
    Select {
        organization_id: String,
        expected_context_version: i64,
    },
    ClearSelection {
        expected_context_version: i64,
    },
    Rename {
        scope: OrganizationScope,
        name: String,
    },
    Invite {
        scope: OrganizationScope,
        email: String,
        role: OrganizationRole,
        valid_for_hours: u32,
        membership_valid_until: Option<String>,
    },
    RevokeInvite {
        scope: OrganizationScope,
        invitation_id: String,
        invitation_version: i64,
    },
    PreviewInvitation {
        token: String,
    },
    AcceptInvitation {
        token: String,
    },
    RotateAccessCode {
        scope: OrganizationScope,
    },
    RequestAccess {
        code: String,
        requested_role: OrganizationRole,
    },
    DecideAccess {
        scope: OrganizationScope,
        request_id: String,
        request_version: i64,
        approve: bool,
    },
    ChangeMember {
        scope: OrganizationScope,
        user_id: String,
        member_version: i64,
        role: OrganizationRole,
        valid_until: Option<String>,
    },
    RemoveMember {
        scope: OrganizationScope,
        user_id: String,
        member_version: i64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrganizationSummary {
    pub id: String,
    pub name: String,
    pub role: OrganizationRole,
    pub version: i64,
    pub valid_until: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemberSummary {
    pub id: String,
    pub email: String,
    pub role: OrganizationRole,
    pub active: bool,
    pub valid_until: Option<String>,
    pub version: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InvitationSummary {
    pub id: String,
    pub email: String,
    pub role: OrganizationRole,
    pub expires_at: String,
    pub membership_valid_until: Option<String>,
    pub status: String,
    pub version: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AccessRequestSummary {
    pub id: String,
    pub email: String,
    pub requested_role: OrganizationRole,
    pub status: String,
    pub version: i64,
    pub created_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrganizationDetail {
    pub organization: OrganizationSummary,
    pub members: Vec<MemberSummary>,
    pub invitations: Vec<InvitationSummary>,
    pub access_requests: Vec<AccessRequestSummary>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OrganizationState {
    pub current_user_id: String,
    pub organizations: Vec<OrganizationSummary>,
    pub active: Option<OrganizationDetail>,
    pub context_version: i64,
    pub mfa_verified: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct InvitationPreview {
    pub organization_name: String,
    pub role: OrganizationRole,
    pub expires_at: String,
    pub membership_valid_until: Option<String>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct OrganizationResponse {
    pub state: OrganizationState,
    pub csrf: String,
    pub message: String,
    pub invitation_code: Option<String>,
    pub access_code: Option<String>,
    pub invitation_preview: Option<InvitationPreview>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MemberSearch {
    pub organization_id: String,
    pub context_version: i64,
    pub query: String,
    pub members: Vec<MemberSummary>,
    pub total: i64,
}
