use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredObligationVersion {
    pub obligation_id: Uuid,
    pub scope_id: Uuid,
    pub version: i64,
    pub payload: String,
    pub confirmed: bool,
    pub professional_name: Option<String>,
    pub document_sha256: Option<String>,
    pub replaced_by: Option<Uuid>,
    pub actor_id: Uuid,
    pub fingerprint: Vec<u8>,
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}
pub struct ObligationWrite<'a> {
    pub tenant: Uuid,
    pub id: Uuid,
    pub scope: Uuid,
    pub expected: Option<i64>,
    pub payload: &'a str,
    pub confirmed: bool,
    pub professional_id: Option<Uuid>,
    pub professional_name: Option<&'a str>,
    pub document_id: Option<Uuid>,
    pub document_version: Option<i64>,
    pub sha256: Option<&'a str>,
    pub replaces_id: Option<Uuid>,
    pub actor: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: &'a [u8],
    pub reason: &'a str,
}
#[derive(sqlx::FromRow)]
pub struct TechnicalDocumentChoice {
    pub document_id: Uuid,
    pub version: i64,
    pub scope_id: Uuid,
    pub label: String,
    pub sha256: String,
}
impl AuthTransaction {
    pub async fn obligation_list(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredObligationVersion>, AuthStoreError> {
        let q = "SELECT v.obligation_id,v.scope_id,v.version,v.payload,v.confirmed,v.professional_name,v.document_sha256,r.new_id AS replaced_by,v.actor_id,v.fingerprint,v.reason,v.recorded_at FROM canteiro.obligation_versions v LEFT JOIN canteiro.obligation_replacements r ON r.tenant_id=v.tenant_id AND r.previous_id=v.obligation_id JOIN canteiro.technical_obligations o ON o.tenant_id=v.tenant_id AND o.id=v.obligation_id AND o.current_version=v.version WHERE v.tenant_id=$1 AND v.scope_id=ANY($2) ORDER BY v.recorded_at DESC,v.obligation_id";
        Ok(sqlx::query_as(q)
            .bind(tenant)
            .bind(scopes)
            .fetch_all(&mut *self.tx)
            .await?)
    }
    pub async fn obligation_current(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredObligationVersion>, AuthStoreError> {
        let q = "SELECT v.obligation_id,v.scope_id,v.version,v.payload,v.confirmed,v.professional_name,v.document_sha256,r.new_id AS replaced_by,v.actor_id,v.fingerprint,v.reason,v.recorded_at FROM canteiro.obligation_versions v LEFT JOIN canteiro.obligation_replacements r ON r.tenant_id=v.tenant_id AND r.previous_id=v.obligation_id JOIN canteiro.technical_obligations o ON o.tenant_id=v.tenant_id AND o.id=v.obligation_id AND o.current_version=v.version WHERE v.tenant_id=$1 AND v.obligation_id=$2 FOR UPDATE OF o";
        Ok(sqlx::query_as(q)
            .bind(tenant)
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await?)
    }
    pub async fn obligation_operation(
        &mut self,
        tenant: Uuid,
        key: Uuid,
    ) -> Result<Option<StoredObligationVersion>, AuthStoreError> {
        let q = "SELECT v.obligation_id,v.scope_id,v.version,v.payload,v.confirmed,v.professional_name,v.document_sha256,r.new_id AS replaced_by,v.actor_id,v.fingerprint,v.reason,v.recorded_at FROM canteiro.obligation_versions v LEFT JOIN canteiro.obligation_replacements r ON r.tenant_id=v.tenant_id AND r.previous_id=v.obligation_id WHERE v.tenant_id=$1 AND v.operation_key=$2";
        Ok(sqlx::query_as(q)
            .bind(tenant)
            .bind(key)
            .fetch_optional(&mut *self.tx)
            .await?)
    }
    pub async fn obligation_history(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Vec<StoredObligationVersion>, AuthStoreError> {
        let q = "SELECT v.obligation_id,v.scope_id,v.version,v.payload,v.confirmed,v.professional_name,v.document_sha256,r.new_id AS replaced_by,v.actor_id,v.fingerprint,v.reason,v.recorded_at FROM canteiro.obligation_versions v LEFT JOIN canteiro.obligation_replacements r ON r.tenant_id=v.tenant_id AND r.previous_id=v.obligation_id WHERE v.tenant_id=$1 AND v.obligation_id=$2 ORDER BY v.version DESC LIMIT 200";
        Ok(sqlx::query_as(q)
            .bind(tenant)
            .bind(id)
            .fetch_all(&mut *self.tx)
            .await?)
    }
    pub async fn technical_documents(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<TechnicalDocumentChoice>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT v.document_id,v.version,d.scope_id,(v.file_name || ' · versão ' || v.version::text) AS label,v.sha256 FROM canteiro.documents d JOIN canteiro.document_versions v ON v.tenant_id=d.tenant_id AND v.document_id=d.id WHERE d.tenant_id=$1 AND d.scope_id=ANY($2) AND v.category IN ('technical','evidence') ORDER BY v.created_at DESC LIMIT 500")
            .bind(tenant).bind(scopes).fetch_all(&mut *self.tx).await?)
    }
    pub async fn obligation_write(
        &mut self,
        w: &ObligationWrite<'_>,
    ) -> Result<Option<StoredObligationVersion>, AuthStoreError> {
        let version = if let Some(v) = w.expected {
            match v.checked_add(1) {
                Some(n) => n,
                None => return Ok(None),
            }
        } else {
            1
        };
        let count = if let Some(expected) = w.expected {
            sqlx::query("UPDATE canteiro.technical_obligations SET current_version=$4 WHERE tenant_id=$1 AND id=$2 AND current_version=$3 AND scope_id=$5")
                .bind(w.tenant).bind(w.id).bind(expected).bind(version).bind(w.scope).execute(&mut *self.tx).await?.rows_affected()
        } else {
            sqlx::query("INSERT INTO canteiro.technical_obligations(tenant_id,id,scope_id,current_version) VALUES($1,$2,$3,1) ON CONFLICT DO NOTHING")
                .bind(w.tenant).bind(w.id).bind(w.scope).execute(&mut *self.tx).await?.rows_affected()
        };
        if count != 1 {
            return Ok(None);
        }
        sqlx::query("INSERT INTO canteiro.obligation_versions(tenant_id,obligation_id,scope_id,version,payload,confirmed,professional_id,professional_name,document_id,document_version,document_sha256,replaces_id,actor_id,operation_key,fingerprint,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)")
            .bind(w.tenant).bind(w.id).bind(w.scope).bind(version).bind(w.payload).bind(w.confirmed).bind(w.professional_id).bind(w.professional_name).bind(w.document_id).bind(w.document_version).bind(w.sha256).bind(w.replaces_id).bind(w.actor).bind(w.operation_key).bind(w.fingerprint).bind(w.reason).execute(&mut *self.tx).await?;
        if w.confirmed
            && let Some(previous) = w.replaces_id
        {
            sqlx::query("INSERT INTO canteiro.obligation_replacements(tenant_id,previous_id,new_id,new_version) VALUES($1,$2,$3,$4)")
                .bind(w.tenant).bind(previous).bind(w.id).bind(version).execute(&mut *self.tx).await?;
        }
        self.obligation_current(w.tenant, w.id).await
    }
}
