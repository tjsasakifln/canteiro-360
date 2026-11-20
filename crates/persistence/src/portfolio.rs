use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;
#[derive(sqlx::FromRow)]
pub struct StoredWork {
    pub id: Uuid,
    pub data: String,
    pub version: i64,
}
#[derive(sqlx::FromRow)]
pub struct StoredWorkRevision {
    pub version: i64,
    pub before_data: Option<String>,
    pub after_data: String,
    pub actor: String,
    pub recorded_at: DateTime<Utc>,
    pub reason: String,
}
impl AuthTransaction {
    pub async fn portfolio_list(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredWork>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,data,version FROM canteiro.portfolio_works WHERE tenant_id=$1 ORDER BY lower(title),id").bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn portfolio_get(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredWork>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,data,version FROM canteiro.portfolio_works WHERE tenant_id=$1 AND id=$2 FOR UPDATE").bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn portfolio_write(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        actor: Uuid,
        expected: Option<i64>,
        title: &str,
        status: &str,
        data: &str,
    ) -> Result<bool, AuthStoreError> {
        let query = if expected.is_some() {
            "UPDATE canteiro.portfolio_works SET title=$4,status=$5,data=$6,version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$7 AND version<9223372036854775807 AND $3::uuid IS NOT NULL"
        } else {
            "INSERT INTO canteiro.portfolio_works(tenant_id,id,created_by,title,status,data) SELECT $1,$2,$3,$4,$5,$6 WHERE $7::bigint IS NULL ON CONFLICT(tenant_id,id) DO NOTHING"
        };
        Ok(sqlx::query(query)
            .bind(tenant)
            .bind(id)
            .bind(actor)
            .bind(title)
            .bind(status)
            .bind(data)
            .bind(expected)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
            == 1)
    }
    pub async fn portfolio_links(
        &mut self,
        tenant: Uuid,
        work: Uuid,
        contracts: &[Uuid],
    ) -> Result<(), AuthStoreError> {
        sqlx::query(
            "DELETE FROM canteiro.portfolio_contract_links WHERE tenant_id=$1 AND work_id=$2",
        )
        .bind(tenant)
        .bind(work)
        .execute(&mut *self.tx)
        .await?;
        for contract in contracts {
            sqlx::query("INSERT INTO canteiro.portfolio_contract_links(tenant_id,work_id,contract_id) VALUES($1,$2,$3)").bind(tenant).bind(work).bind(contract).execute(&mut *self.tx).await?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn portfolio_record_revision(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        version: i64,
        before: Option<&str>,
        after: &str,
        actor: Uuid,
        reason: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.portfolio_history(tenant_id,work_id,version,before_data,after_data,actor_id,reason) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(tenant).bind(id).bind(version).bind(before).bind(after).bind(actor).bind(reason).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn portfolio_history(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Vec<StoredWorkRevision>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT h.version,h.before_data,h.after_data,u.email AS actor,h.recorded_at,h.reason FROM canteiro.portfolio_history h JOIN canteiro_auth.users u ON u.id=h.actor_id WHERE h.tenant_id=$1 AND h.work_id=$2 ORDER BY h.version DESC LIMIT 100").bind(tenant).bind(id).fetch_all(&mut *self.tx).await?)
    }
}
