//! Contratos públicos. Nunca incluir credenciais nem entidades internas.
mod error;

pub use error::{ErrorCode, ErrorField, PublicError, WriteOutcome};

use serde::{Deserialize, Serialize};

/// Liveness do processo; não representa prontidão do produto ou do banco.
#[derive(Debug, Serialize, Deserialize)]
pub struct Liveness {
    pub status: ProcessStatus,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Running,
}
