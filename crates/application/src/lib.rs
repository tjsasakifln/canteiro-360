//! Casos de uso. A fundação não oferece comandos de negócio.
use canteiro_contracts::{Liveness, ProcessStatus};

/// Resposta deliberadamente mínima, sem ambiente, credenciais ou infraestrutura.
pub fn liveness() -> Liveness {
    Liveness {
        status: ProcessStatus::Running,
    }
}
