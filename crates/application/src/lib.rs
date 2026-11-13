//! Casos de uso e autenticação. Identidade não substitui autorização de obra.
pub mod auth;
use canteiro_contracts::{Liveness, ProcessStatus};

/// Resposta deliberadamente mínima, sem ambiente, credenciais ou infraestrutura.
pub fn liveness() -> Liveness {
    Liveness {
        status: ProcessStatus::Running,
    }
}
