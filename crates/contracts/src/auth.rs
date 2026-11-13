//! Contratos públicos de acesso. Credenciais nunca implementam Debug.
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum AuthCommand {
    Register { email: String, password: String },
    RequestVerification { email: String },
    VerifyEmail { token: String, password: String },
    Login { email: String, password: String },
    RequestReset { email: String },
    ResetPassword { token: String, password: String },
    StartMfa { password: String },
    ConfirmMfa { code: String },
    VerifyMfa { code: String },
    RecoverMfa { code: String },
    Logout,
    RevokeSession { session_id: String },
    RevokeAll,
    PrivilegedCheck,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthState {
    Anonymous,
    MfaPending,
    Authenticated,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub id: String,
    pub current: bool,
    pub created_at: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct AuthContext {
    pub state: AuthState,
    pub email: Option<String>,
    pub privileged: bool,
    pub mfa_enabled: bool,
    pub sessions: Vec<SessionSummary>,
}

/// Segredos transitórios aparecem apenas na resposta privada solicitada, nunca
/// em HTML inicial, URL, logs ou armazenamento persistente do navegador.
#[derive(Clone, Deserialize, Serialize)]
pub struct AuthResponse {
    pub context: AuthContext,
    pub csrf: String,
    pub message: String,
    pub mfa_setup: Option<String>,
    pub recovery_codes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthFailure {
    pub code: String,
    pub message: String,
}
