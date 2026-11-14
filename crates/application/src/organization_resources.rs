//! Fronteiras de empresa para recursos fora das consultas relacionais.
//!
//! Estes tipos definem o contrato dos adaptadores de objetos, cache, busca,
//! processamento e exportação. Não implementam nem simulam esses provedores.
//! Uma referência, chave ou mensagem serializada nunca é uma autorização.
//! O serviço revalida sessão, vínculo e escopo antes de construir a fronteira,
//! em cada operação, e antes de entregar o resultado de um processamento.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::marker::PhantomData;
use uuid::Uuid;

/// Classes fechadas: entradas externas não podem introduzir caminhos livres.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Organization,
    Member,
    Invitation,
    AccessRequest,
    OrganizationUnit,
    Document,
    Export,
    Task,
}

impl ResourceKind {
    fn segment(self) -> &'static str {
        match self {
            Self::Organization => "organizations",
            Self::Member => "members",
            Self::Invitation => "invitations",
            Self::AccessRequest => "access-requests",
            Self::OrganizationUnit => "units",
            Self::Document => "documents",
            Self::Export => "exports",
            Self::Task => "tasks",
        }
    }
}

/// Identifica um recurso, sem provar que o destinatário pode acessá-lo.
/// Toda desserialização precisa passar pela fronteira autorizada atual.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub tenant_id: Uuid,
    pub id: Uuid,
    pub kind: ResourceKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResourceError {
    #[error("Referência indisponível neste contexto.")]
    WrongOrganization,
    #[error("O recurso não pertence a esta consulta.")]
    WrongKind,
    #[error("A referência informada não é válida.")]
    InvalidIdentity,
    #[error("A versão informada não é válida.")]
    InvalidVersion,
    #[error("A consulta ultrapassa o limite permitido.")]
    QueryTooLong,
}

/// Fronteira interna com duração de uma única operação autorizada.
///
/// O construtor fica restrito à aplicação. Chamá-lo somente depois de conferir
/// a identidade, a organização ativa e o vínculo, dentro da transação aplicável.
/// Não guardar em sessão, cache, estado global ou mensagem de fila. Não possui
/// Clone/Serialize/Deserialize. As chaves e os escopos emprestam esta fronteira
/// para não serem carregados como autorização entre operações.
///
/// O empréstimo limita a vida dos valores derivados, mas não substitui a
/// revalidação no servidor nem os filtros/controles dos adaptadores consumidores.
pub struct ResourceBoundary {
    tenant_id: Uuid,
    context_version: i64,
}

impl ResourceBoundary {
    pub(crate) fn new(tenant: Uuid, context_version: i64) -> Result<Self, ResourceError> {
        valid_id(tenant)?;
        valid_version(context_version)?;
        Ok(Self {
            tenant_id: tenant,
            context_version,
        })
    }

    pub fn reference(&self, kind: ResourceKind, id: Uuid) -> Result<ResourceRef, ResourceError> {
        valid_id(id)?;
        Ok(ResourceRef {
            tenant_id: self.tenant_id,
            id,
            kind,
        })
    }

    /// Confere a empresa antes de qualquer leitura de conteúdo do recurso.
    /// A concessão específica de obra/documento continua obrigatória no serviço.
    pub fn validate(&self, reference: &ResourceRef) -> Result<(), ResourceError> {
        if reference.tenant_id != self.tenant_id {
            return Err(ResourceError::WrongOrganization);
        }
        valid_id(reference.id)
    }

    pub fn validate_kind(
        &self,
        reference: &ResourceRef,
        expected: ResourceKind,
    ) -> Result<(), ResourceError> {
        self.validate(reference)?;
        if reference.kind != expected {
            return Err(ResourceError::WrongKind);
        }
        Ok(())
    }

    /// Caminho privado estável por empresa, classe, identidade e revisão.
    /// Não aceita nome de arquivo, URL, caminho relativo ou chave global.
    /// O adaptador nunca expõe esse caminho como URL pública de autorização.
    pub fn object_key(
        &self,
        reference: &ResourceRef,
        data_version: i64,
    ) -> Result<ObjectKey<'_>, ResourceError> {
        self.validate(reference)?;
        valid_version(data_version)?;
        Ok(ObjectKey::new(format!(
            "org-v1/{}/{}/{}/revision/{data_version}",
            self.tenant_id,
            reference.kind.segment(),
            reference.id,
        )))
    }

    /// Separa resultados por empresa, pessoa, contexto, classe e versão.
    /// `actor_id` deve vir do principal autenticado, nunca do corpo do pedido.
    /// `query` deve incluir os filtros, ordem e página em representação estável.
    /// A chave contém apenas o resumo da consulta, sem nomes ou dados informados.
    /// Revalidar acesso antes do cache; uma chave antiga não preserva direitos.
    pub fn cache_key(
        &self,
        kind: ResourceKind,
        actor_id: Uuid,
        query: &str,
        data_version: i64,
    ) -> Result<CacheKey<'_>, ResourceError> {
        valid_id(actor_id)?;
        valid_version(data_version)?;
        if query.len() > 8_192 {
            return Err(ResourceError::QueryTooLong);
        }
        let digest = Sha256::digest(query.as_bytes());
        Ok(CacheKey::new(format!(
            "org-v1/{}/cache/{actor_id}/context/{}/{}/revision/{data_version}/query/{digest:x}",
            self.tenant_id,
            self.context_version,
            kind.segment(),
        )))
    }

    /// Filtro obrigatório antes de buscar, paginar, contar ou agregar.
    /// Filtrar somente os resultados retornados deixaria contagens expostas.
    pub fn search_scope(&self, kind: ResourceKind) -> SearchScope<'_> {
        SearchScope {
            boundary: self,
            kind,
        }
    }

    /// Namespace privado de exportação: empresa, época de contexto, identidade
    /// e versão da organização/conjunto lida no banco na mesma operação.
    /// O nome serializado é um endereço interno, não uma concessão de download.
    /// Revalidar a associação e a versão ao produzir e ao entregar o arquivo.
    pub fn export_key(&self, id: Uuid, data_version: i64) -> Result<ExportKey<'_>, ResourceError> {
        valid_id(id)?;
        valid_version(data_version)?;
        Ok(ExportKey::new(format!(
            "org-v1/{}/exports/context/{}/{id}/revision/{data_version}",
            self.tenant_id, self.context_version,
        )))
    }

    /// Transporta a empresa de origem, sem transportar a autorização atual.
    /// O consumidor reautoriza o solicitante e recria uma fronteira na execução;
    /// a existência de uma mensagem não mantém vínculo removido ou acesso vencido.
    pub fn queue_job<T>(&self, id: Uuid, payload: T) -> Result<QueueEnvelope<T>, ResourceError> {
        valid_id(id)?;
        Ok(QueueEnvelope {
            tenant_id: self.tenant_id,
            id,
            payload,
        })
    }

    /// Recusa a empresa errada antes de entregar o conteúdo ao consumidor.
    /// A autorização de execução deve ter ocorrido antes de criar `self`.
    /// Referências contidas no payload também precisam passar por `validate`;
    /// o envelope não confere nem autoriza os campos de um conteúdo genérico.
    pub fn job_payload<'operation, T>(
        &'operation self,
        envelope: &'operation QueueEnvelope<T>,
    ) -> Result<&'operation T, ResourceError> {
        self.validate(&ResourceRef {
            tenant_id: envelope.tenant_id,
            id: envelope.id,
            kind: ResourceKind::Task,
        })?;
        Ok(&envelope.payload)
    }
}

macro_rules! scoped_key {
    ($name:ident) => {
        pub struct $name<'operation> {
            value: String,
            boundary: PhantomData<&'operation ResourceBoundary>,
        }

        impl<'operation> $name<'operation> {
            fn new(value: String) -> Self {
                Self {
                    value,
                    boundary: PhantomData,
                }
            }

            /// Endereço interno do adaptador, sem significado de autorização.
            pub fn as_str(&self) -> &str {
                &self.value
            }
        }
    };
}

scoped_key!(ObjectKey);
scoped_key!(CacheKey);
scoped_key!(ExportKey);

/// Escopo emprestado de uma operação autorizada, não desserializável do cliente.
pub struct SearchScope<'operation> {
    boundary: &'operation ResourceBoundary,
    kind: ResourceKind,
}

impl SearchScope<'_> {
    pub fn tenant_id(&self) -> Uuid {
        self.boundary.tenant_id
    }

    pub fn kind(&self) -> ResourceKind {
        self.kind
    }

    pub fn prefix(&self) -> String {
        format!(
            "org-v1/{}/{}/",
            self.boundary.tenant_id,
            self.kind.segment()
        )
    }

    /// Defesa adicional no adaptador; não substitui o filtro anterior à consulta.
    pub fn validate(&self, reference: &ResourceRef) -> Result<(), ResourceError> {
        self.boundary.validate_kind(reference, self.kind)
    }
}

/// Mensagem persistível, sem Debug e sem acesso público direto ao conteúdo.
/// Pode ser forjada no transporte: somente uma fronteira reautorizada pode abrir.
/// O conteúdo deve carregar ator, operação e versão necessários ao caso de uso.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QueueEnvelope<T> {
    tenant_id: Uuid,
    id: Uuid,
    payload: T,
}

impl<T> QueueEnvelope<T> {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn id(&self) -> Uuid {
        self.id
    }
}

fn valid_id(id: Uuid) -> Result<(), ResourceError> {
    if id == Uuid::nil() || id.as_bytes() == &[255; 16] {
        return Err(ResourceError::InvalidIdentity);
    }
    Ok(())
}

fn valid_version(version: i64) -> Result<(), ResourceError> {
    if version < 1 {
        return Err(ResourceError::InvalidVersion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn id(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    #[test]
    fn identical_resource_ids_remain_separate_between_organizations() {
        let a = ResourceBoundary::new(id(1), 1).unwrap();
        let b = ResourceBoundary::new(id(2), 1).unwrap();
        let first = a.reference(ResourceKind::Document, id(9)).unwrap();
        let other = b.reference(ResourceKind::Document, id(9)).unwrap();
        assert_ne!(
            a.object_key(&first, 1).unwrap().as_str(),
            b.object_key(&other, 1).unwrap().as_str()
        );
        assert_eq!(a.validate(&other), Err(ResourceError::WrongOrganization));
        assert!(matches!(
            a.object_key(&other, 1),
            Err(ResourceError::WrongOrganization)
        ));
        let transported: ResourceRef =
            serde_json::from_str(&serde_json::to_string(&other).unwrap()).unwrap();
        assert_eq!(
            a.validate(&transported),
            Err(ResourceError::WrongOrganization)
        );
    }

    #[test]
    fn search_scope_rejects_cross_tenant_and_cross_kind_before_content() {
        let a = ResourceBoundary::new(id(1), 1).unwrap();
        let scope = a.search_scope(ResourceKind::Member);
        let same = a.reference(ResourceKind::Member, id(7)).unwrap();
        let wrong_tenant = ResourceRef {
            tenant_id: id(2),
            ..same
        };
        let wrong_kind = ResourceRef {
            kind: ResourceKind::Invitation,
            ..same
        };
        assert_eq!(scope.tenant_id(), id(1));
        assert_eq!(scope.kind(), ResourceKind::Member);
        assert!(scope.prefix().ends_with("/members/"));
        assert_eq!(scope.validate(&same), Ok(()));
        assert_eq!(
            scope.validate(&wrong_tenant),
            Err(ResourceError::WrongOrganization)
        );
        assert_eq!(scope.validate(&wrong_kind), Err(ResourceError::WrongKind));
    }

    #[test]
    fn cache_separates_tenant_actor_context_query_kind_and_data_version() {
        let a = ResourceBoundary::new(id(1), 3).unwrap();
        let b = ResourceBoundary::new(id(2), 3).unwrap();
        let next = ResourceBoundary::new(id(1), 4).unwrap();
        let key = a
            .cache_key(ResourceKind::Member, id(7), "page=1", 2)
            .unwrap();
        for other in [
            b.cache_key(ResourceKind::Member, id(7), "page=1", 2)
                .unwrap(),
            a.cache_key(ResourceKind::Member, id(8), "page=1", 2)
                .unwrap(),
            next.cache_key(ResourceKind::Member, id(7), "page=1", 2)
                .unwrap(),
            a.cache_key(ResourceKind::Member, id(7), "page=2", 2)
                .unwrap(),
            a.cache_key(ResourceKind::Invitation, id(7), "page=1", 2)
                .unwrap(),
            a.cache_key(ResourceKind::Member, id(7), "page=1", 3)
                .unwrap(),
        ] {
            assert_ne!(key.as_str(), other.as_str());
        }
        assert_eq!(
            key.as_str(),
            a.cache_key(ResourceKind::Member, id(7), "page=1", 2)
                .unwrap()
                .as_str()
        );
        let sensitive = a
            .cache_key(
                ResourceKind::Member,
                id(7),
                "../../someone@example.invalid?token=secret",
                2,
            )
            .unwrap();
        assert!(!sensitive.as_str().contains(".."));
        assert!(!sensitive.as_str().contains("example"));
        assert!(!sensitive.as_str().contains("secret"));
    }

    #[test]
    fn stale_context_cannot_reuse_export_namespace_but_object_revision_is_stable() {
        let a = ResourceBoundary::new(id(1), 3).unwrap();
        let next = ResourceBoundary::new(id(1), 4).unwrap();
        let b = ResourceBoundary::new(id(2), 3).unwrap();
        let resource = a.reference(ResourceKind::Document, id(9)).unwrap();
        let key = a.export_key(id(9), 5).unwrap();
        for other in [
            next.export_key(id(9), 5).unwrap(),
            b.export_key(id(9), 5).unwrap(),
            a.export_key(id(9), 6).unwrap(),
        ] {
            assert_ne!(key.as_str(), other.as_str());
        }
        assert_eq!(
            a.object_key(&resource, 5).unwrap().as_str(),
            next.object_key(&resource, 5).unwrap().as_str()
        );
        assert_ne!(
            a.object_key(&resource, 5).unwrap().as_str(),
            a.object_key(&resource, 6).unwrap().as_str()
        );
    }

    #[test]
    fn queue_rejects_wrong_company_before_reading_payload_and_survives_transport() {
        let a = ResourceBoundary::new(id(1), 3).unwrap();
        let b = ResourceBoundary::new(id(2), 3).unwrap();
        let calls = Cell::new(0);
        let job = a.queue_job(id(9), &calls).unwrap();
        assert!(matches!(
            b.job_payload(&job),
            Err(ResourceError::WrongOrganization)
        ));
        assert_eq!(calls.get(), 0);
        a.job_payload(&job).unwrap().set(1);
        assert_eq!(calls.get(), 1);

        let persistent = a.queue_job(id(10), "private-data").unwrap();
        let encoded = serde_json::to_string(&persistent).unwrap();
        let decoded: QueueEnvelope<String> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.tenant_id(), id(1));
        assert_eq!(decoded.id(), id(10));
        assert!(matches!(
            b.job_payload(&decoded),
            Err(ResourceError::WrongOrganization)
        ));
        assert_eq!(a.job_payload(&decoded).unwrap(), "private-data");
        // Uma nova operação autorizada da mesma empresa pode retomar a tarefa.
        // A mensagem não mantém a antiga sessão nem sua época como autorização.
        let current = ResourceBoundary::new(id(1), 4).unwrap();
        assert_eq!(current.job_payload(&decoded).unwrap(), "private-data");
    }

    #[test]
    fn malformed_identifiers_versions_and_transport_cannot_make_namespaces() {
        assert!(matches!(
            ResourceBoundary::new(Uuid::nil(), 1),
            Err(ResourceError::InvalidIdentity)
        ));
        assert!(matches!(
            ResourceBoundary::new(Uuid::from_u128(u128::MAX), 1),
            Err(ResourceError::InvalidIdentity)
        ));
        assert!(matches!(
            ResourceBoundary::new(id(1), 0),
            Err(ResourceError::InvalidVersion)
        ));
        let a = ResourceBoundary::new(id(1), 1).unwrap();
        assert!(matches!(
            a.reference(ResourceKind::Document, Uuid::nil()),
            Err(ResourceError::InvalidIdentity)
        ));
        assert!(matches!(
            a.export_key(id(9), -1),
            Err(ResourceError::InvalidVersion)
        ));
        assert!(matches!(
            a.queue_job(Uuid::nil(), "value"),
            Err(ResourceError::InvalidIdentity)
        ));
        assert!(matches!(
            a.cache_key(ResourceKind::Member, id(7), &"x".repeat(8_193), 1),
            Err(ResourceError::QueryTooLong)
        ));
        assert!(serde_json::from_str::<ResourceRef>(r#"{"tenant_id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000009","kind":"../../private"}"#).is_err());
        assert!(serde_json::from_str::<QueueEnvelope<String>>(r#"{"tenant_id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000009","payload":"data","authorized":true}"#).is_err());
        let invalid: QueueEnvelope<String> = serde_json::from_str(r#"{"tenant_id":"00000000-0000-0000-0000-000000000001","id":"00000000-0000-0000-0000-000000000000","payload":"data"}"#).unwrap();
        assert!(matches!(
            a.job_payload(&invalid),
            Err(ResourceError::InvalidIdentity)
        ));
    }
}
