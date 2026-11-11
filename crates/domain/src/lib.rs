//! Tipos e invariantes puros compartilháveis com o navegador.
//! A persistência converte explicitamente estes tipos, sem acoplar o domínio ao SQL.
#![forbid(unsafe_code)]

mod values;

pub use values::{
    ActorId, CivilDate, EntityId, EntityVersion, InvalidValue, OperationId, TenantId, UtcInstant,
};
