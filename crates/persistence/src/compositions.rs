//! Versões de composições conservam a descrição e o autor de cada alteração.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;
#[derive(Clone, sqlx::FromRow)]
pub struct StoredCompositionVersion {
    pub scope_id: Uuid,
    pub code: String,
    pub version: i64,
    pub payload: String,
    pub actor_id: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: Vec<u8>,
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}
pub struct CompositionWrite<'a> {
    pub tenant: Uuid,
    pub scope: Uuid,
    pub code: &'a str,
    pub expected: Option<i64>,
    pub actor: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: &'a [u8],
    pub reason: &'a str,
    pub payload: &'a str,
    pub bindings: &'a [(String, u32)],
}
impl AuthTransaction {
    /// Serializa alterações do grafo inteiro, não apenas da composição editada.
    /// O bloqueio dura até o fim da transação e precede a leitura das versões.
    pub async fn composition_lock_scope(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(
                hashtextextended('composition:' || $1::UUID::TEXT || ':' || $2::UUID::TEXT, 0)
             )",
        )
        .bind(tenant)
        .bind(scope)
        .execute(&mut *self.tx)
        .await?;
        Ok(())
    }

    /// A consulta de tela é limitada; a validação do grafo precisa de todas
    /// as composições do escopo, inclusive referências além da primeira página.
    pub async fn composition_graph(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
    ) -> Result<Vec<StoredCompositionVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.code,v.version,v.payload::TEXT AS payload,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at
             FROM canteiro.cost_compositions c JOIN canteiro.cost_composition_versions v
              ON v.tenant_id=c.tenant_id AND v.scope_id=c.scope_id
              AND v.code=c.code AND v.version=c.current_version
             WHERE c.tenant_id=$1 AND c.scope_id=$2 ORDER BY v.code",
        )
        .bind(tenant)
        .bind(scope)
        .fetch_all(&mut *self.tx)
        .await?)
    }

    /// Busca por identidade, sem o limite da listagem e sem bloquear a linha.
    pub async fn composition_lookup(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        code: &str,
    ) -> Result<Option<StoredCompositionVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.code,v.version,v.payload::TEXT AS payload,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at
             FROM canteiro.cost_compositions c JOIN canteiro.cost_composition_versions v
              ON v.tenant_id=c.tenant_id AND v.scope_id=c.scope_id
              AND v.code=c.code AND v.version=c.current_version
             WHERE c.tenant_id=$1 AND c.scope_id=$2 AND c.code=$3",
        )
        .bind(tenant)
        .bind(scope)
        .bind(code)
        .fetch_optional(&mut *self.tx)
        .await?)
    }

    pub async fn composition_current(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        code: &str,
    ) -> Result<Option<StoredCompositionVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.code,v.version,v.payload::TEXT AS payload,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at
             FROM canteiro.cost_compositions c JOIN canteiro.cost_composition_versions v
              ON v.tenant_id=c.tenant_id AND v.scope_id=c.scope_id
              AND v.code=c.code AND v.version=c.current_version
             WHERE c.tenant_id=$1 AND c.scope_id=$2 AND c.code=$3 FOR UPDATE OF c",
        )
        .bind(tenant)
        .bind(scope)
        .bind(code)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn composition_list(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredCompositionVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT v.scope_id,v.code,v.version,v.payload::TEXT AS payload,
             v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at
             FROM canteiro.cost_compositions c JOIN canteiro.cost_composition_versions v
              ON v.tenant_id=c.tenant_id AND v.scope_id=c.scope_id
              AND v.code=c.code AND v.version=c.current_version
             WHERE c.tenant_id=$1 AND c.scope_id=ANY($2)
             ORDER BY v.scope_id,v.code LIMIT 500",
        )
        .bind(tenant)
        .bind(scopes)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    pub async fn composition_operation(
        &mut self,
        tenant: Uuid,
        key: Uuid,
    ) -> Result<Option<StoredCompositionVersion>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT scope_id,code,version,payload::TEXT AS payload,
             actor_id,operation_key,fingerprint,reason,recorded_at
             FROM canteiro.cost_composition_versions
             WHERE tenant_id=$1 AND operation_key=$2",
        )
        .bind(tenant)
        .bind(key)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn composition_catalog_published(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        catalog: &str,
        edition: u32,
    ) -> Result<bool, AuthStoreError> {
        let version = i32::try_from(edition).unwrap_or(i32::MAX);
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM canteiro.cost_catalog_editions e
              JOIN canteiro.cost_catalogs c
              ON c.tenant_id=e.tenant_id AND c.catalog_id=e.catalog_id
             WHERE e.tenant_id=$1 AND c.scope_id=$2 AND e.catalog_id=$3
               AND e.edition=$4 AND e.state='published')",
        )
        .bind(tenant)
        .bind(scope)
        .bind(catalog)
        .bind(version)
        .fetch_one(&mut *self.tx)
        .await?)
    }
    pub async fn composition_write(
        &mut self,
        w: &CompositionWrite<'_>,
    ) -> Result<Option<StoredCompositionVersion>, AuthStoreError> {
        let next = if let Some(v) = w.expected {
            match v.checked_add(1) {
                Some(n) => n,
                None => return Ok(None),
            }
        } else {
            1
        };
        let count = if let Some(expected) = w.expected {
            sqlx::query(
                "UPDATE canteiro.cost_compositions SET current_version=$5
                 WHERE tenant_id=$1 AND scope_id=$2 AND code=$3 AND current_version=$4",
            )
            .bind(w.tenant)
            .bind(w.scope)
            .bind(w.code)
            .bind(expected)
            .bind(next)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
        } else {
            sqlx::query(
                "INSERT INTO canteiro.cost_compositions
                 (tenant_id,scope_id,code,current_version) VALUES($1,$2,$3,1)
                 ON CONFLICT DO NOTHING",
            )
            .bind(w.tenant)
            .bind(w.scope)
            .bind(w.code)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
        };
        if count != 1 {
            return Ok(None);
        }
        sqlx::query(
            "INSERT INTO canteiro.cost_composition_versions
             (tenant_id,scope_id,code,version,payload,policy,actor_id,
              operation_key,fingerprint,reason)
             VALUES($1,$2,$3,$4,$5::JSONB,'C360-DECIMAL-1',$6,$7,$8,$9)",
        )
        .bind(w.tenant)
        .bind(w.scope)
        .bind(w.code)
        .bind(next)
        .bind(w.payload)
        .bind(w.actor)
        .bind(w.operation_key)
        .bind(w.fingerprint)
        .bind(w.reason)
        .execute(&mut *self.tx)
        .await?;
        for (catalog, edition) in w.bindings {
            sqlx::query(
                "INSERT INTO canteiro.cost_composition_catalog_bindings(
                 tenant_id,scope_id,composition_code,composition_version,catalog_id,catalog_edition)
                 VALUES($1,$2,$3,$4,$5,$6)",
            )
            .bind(w.tenant)
            .bind(w.scope)
            .bind(w.code)
            .bind(next)
            .bind(catalog)
            .bind(i32::try_from(*edition).unwrap_or(i32::MAX))
            .execute(&mut *self.tx)
            .await?;
        }
        self.composition_current(w.tenant, w.scope, w.code).await
    }
}
