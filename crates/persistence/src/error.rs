use canteiro_domain::OperationId;
use std::fmt;

/// Categorias controladas. Nunca guardar o erro SQL bruto, valores, SQL ou credenciais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistenceErrorKind {
    InvalidInput,
    NotFound,
    Conflict,
    VersionConflict,
    Unavailable,
    Timeout,
    Internal,
    OutcomeUnknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistenceError {
    kind: PersistenceErrorKind,
    operation_id: OperationId,
}

impl PersistenceError {
    pub(crate) fn new(kind: PersistenceErrorKind, operation_id: OperationId) -> Self {
        // Somente categoria e ID opaco. Sem tenant, ator, parâmetros, SQL, URL ou erro bruto.
        tracing::warn!(failure = ?kind, operation_id = %operation_id, "Falha de persistência");
        Self { kind, operation_id }
    }
    pub fn kind(&self) -> PersistenceErrorKind {
        self.kind
    }
    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    pub(crate) fn from_sql(error: &sqlx::Error, operation_id: OperationId) -> Self {
        use PersistenceErrorKind::*;
        let kind = match error {
            sqlx::Error::PoolTimedOut => Timeout,
            sqlx::Error::PoolClosed | sqlx::Error::Io(_) | sqlx::Error::Tls(_) => Unavailable,
            sqlx::Error::RowNotFound => NotFound,
            sqlx::Error::Database(db) => match db.code().as_deref() {
                Some("57014" | "55P03" | "25P03" | "25P04") => Timeout,
                Some("23505" | "23503" | "40001" | "40P01") => Conflict,
                Some("23514" | "23502" | "22001" | "22003" | "22007" | "22008") => InvalidInput,
                Some("53300" | "57P01" | "57P02" | "57P03" | "08000" | "08003" | "08006") => {
                    Unavailable
                }
                _ => Internal,
            },
            _ => Internal,
        };
        Self::new(kind, operation_id)
    }
}
impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Falha de persistência {:?}; operação {}.",
            self.kind, self.operation_id
        )
    }
}
impl std::error::Error for PersistenceError {}
pub(crate) fn operation_id() -> OperationId {
    OperationId::from_uuid(uuid::Uuid::new_v4()).expect("UUID v4 gerado não é nulo nem máximo")
}
