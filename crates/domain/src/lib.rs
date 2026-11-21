//! Tipos e invariantes puros compartilháveis com o navegador.
//! A persistência converte explicitamente estes tipos, sem acoplar o domínio ao SQL.
#![forbid(unsafe_code)]

mod values;

pub mod access;
pub mod calendar;
pub mod decimal;
pub mod units;

pub use values::{
    ActorId, CivilDate, EntityId, EntityVersion, InvalidValue, OperationId, TenantId, UtcInstant,
};

pub mod documents;
pub mod obligations;
pub mod parties;
pub mod portfolio;
pub mod projects;

pub mod notifications;
pub mod catalogs;
pub mod compositions;
pub mod budgets;
pub mod budget_csv;
