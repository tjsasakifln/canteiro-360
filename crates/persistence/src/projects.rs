use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredProjectVersion {
    pub project_id: Uuid,
    pub version: i64,
    pub payload: String,
    pub actor_id: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: Vec<u8>,
    pub reason: String,
    pub recorded_at: DateTime<Utc>,
}
pub struct ProjectWrite<'a> {
    pub tenant: Uuid,
    pub id: Uuid,
    pub expected: Option<i64>,
    pub payload: &'a str,
    pub actor: Uuid,
    pub operation_key: Uuid,
    pub fingerprint: &'a [u8],
    pub reason: &'a str,
    pub party_ids: &'a [Uuid],
    pub work_id: Option<Uuid>,
}
#[derive(Clone, sqlx::FromRow)]
pub struct ProfessionalReference {
    pub id: Uuid,
    pub name: String,
    pub professional: bool,
    pub active: bool,
    pub council: String,
    pub registration: String,
    pub uf: String,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
}
impl ProfessionalReference {
    pub fn effective_on(&self, on: NaiveDate) -> bool {
        self.professional
            && self.active
            && !self.council.trim().is_empty()
            && !self.registration.trim().is_empty()
            && self.uf.len() == 2
            && self.valid_from.is_some_and(|start| start <= on)
            && self.valid_until.is_none_or(|end| on <= end)
    }
}
impl AuthTransaction {
    pub async fn project_list(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredProjectVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT v.project_id,v.version,v.payload,v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at FROM canteiro.projects p JOIN canteiro.project_versions v ON v.tenant_id=p.tenant_id AND v.project_id=p.id AND v.version=p.current_version WHERE p.tenant_id=$1 AND p.id=ANY($2) ORDER BY v.recorded_at DESC,p.id")
            .bind(tenant).bind(scopes).fetch_all(&mut *self.tx).await?)
    }
    pub async fn project_current(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredProjectVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT v.project_id,v.version,v.payload,v.actor_id,v.operation_key,v.fingerprint,v.reason,v.recorded_at FROM canteiro.projects p JOIN canteiro.project_versions v ON v.tenant_id=p.tenant_id AND v.project_id=p.id AND v.version=p.current_version WHERE p.tenant_id=$1 AND p.id=$2 FOR UPDATE OF p")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn project_history(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Vec<StoredProjectVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT project_id,version,payload,actor_id,operation_key,fingerprint,reason,recorded_at FROM canteiro.project_versions WHERE tenant_id=$1 AND project_id=$2 ORDER BY version DESC LIMIT 200")
            .bind(tenant).bind(id).fetch_all(&mut *self.tx).await?)
    }
    pub async fn project_operation(
        &mut self,
        tenant: Uuid,
        key: Uuid,
    ) -> Result<Option<StoredProjectVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT project_id,version,payload,actor_id,operation_key,fingerprint,reason,recorded_at FROM canteiro.project_versions WHERE tenant_id=$1 AND operation_key=$2")
            .bind(tenant).bind(key).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn project_write(
        &mut self,
        w: &ProjectWrite<'_>,
    ) -> Result<Option<StoredProjectVersion>, AuthStoreError> {
        let version = if let Some(v) = w.expected {
            match v.checked_add(1) {
                Some(next) => next,
                None => return Ok(None),
            }
        } else {
            1
        };
        let changed = if let Some(expected) = w.expected {
            sqlx::query("UPDATE canteiro.projects SET current_version=$4 WHERE tenant_id=$1 AND id=$2 AND current_version=$3")
                .bind(w.tenant).bind(w.id).bind(expected).bind(version).execute(&mut *self.tx).await?.rows_affected()
        } else {
            sqlx::query("INSERT INTO canteiro.projects(tenant_id,id,current_version) VALUES($1,$2,1) ON CONFLICT DO NOTHING")
                .bind(w.tenant).bind(w.id).execute(&mut *self.tx).await?.rows_affected()
        };
        if changed != 1 {
            return Ok(None);
        }
        sqlx::query("INSERT INTO canteiro.project_versions(tenant_id,project_id,version,payload,actor_id,operation_key,fingerprint,reason) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(w.tenant).bind(w.id).bind(version).bind(w.payload).bind(w.actor).bind(w.operation_key).bind(w.fingerprint).bind(w.reason).execute(&mut *self.tx).await?;
        for party in w.party_ids {
            sqlx::query("INSERT INTO canteiro.project_party_links(tenant_id,project_id,version,party_id) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING")
                .bind(w.tenant).bind(w.id).bind(version).bind(party).execute(&mut *self.tx).await?;
        }
        if let Some(work) = w.work_id {
            sqlx::query("INSERT INTO canteiro.project_work_links(tenant_id,project_id,version,work_id) VALUES($1,$2,$3,$4)")
                .bind(w.tenant).bind(w.id).bind(version).bind(work).execute(&mut *self.tx).await?;
        }
        self.project_current(w.tenant, w.id).await
    }
    pub async fn project_party(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<ProfessionalReference>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,name,professional,active,council,registration,uf,valid_from,valid_until FROM canteiro.parties WHERE tenant_id=$1 AND id=$2 FOR SHARE")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn project_choices(
        &mut self,
        tenant: Uuid,
        professionals: bool,
    ) -> Result<Vec<(Uuid, String)>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,name FROM canteiro.parties WHERE tenant_id=$1 AND active AND (NOT $2 OR professional) ORDER BY name,id LIMIT 500")
            .bind(tenant).bind(professionals).fetch_all(&mut *self.tx).await?)
    }
    pub async fn project_work_exists(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM canteiro.portfolio_works WHERE tenant_id=$1 AND id=$2)",
        )
        .bind(tenant)
        .bind(id)
        .fetch_one(&mut *self.tx)
        .await?)
    }
}
