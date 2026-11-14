//! PostgreSQL transacional. Nenhum pool ou executor bruto atravessa esta fronteira.
//! O chamador fornece contexto já autorizado no servidor; isto não autentica usuários.
pub mod access;
pub mod auth;
mod error;
mod migration;
pub mod organizations;
mod pool;
mod settings;
mod unit_of_work;

pub use error::{PersistenceError, PersistenceErrorKind};
pub use migration::{MIGRATOR, migrate};
pub use pool::Database;
pub use settings::{
    DatabaseEnvironment, DatabaseSettings, InvalidDatabaseSettings, TransactionLimits,
};
pub use unit_of_work::{Committed, NewUnit, TransactionContext, UnitOfWork, UnitRecord};

#[cfg(test)]
mod database_tests;
