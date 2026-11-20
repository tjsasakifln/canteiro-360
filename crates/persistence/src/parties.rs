//! Persistência da identidade e de revisões. O serviço abre e autoriza a transação.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct StoredParty {
    pub id: Uuid,
    pub data: String,
    pub version: i64,
}
pub struct PartyWrite<'a> {
    pub tenant: Uuid,
    pub id: Uuid,
    pub actor: Uuid,
    pub expected: Option<i64>,
    pub kind: &'a str,
    pub tax_id: Option<&'a str>,
    pub name: &'a str,
    pub customer: bool,
    pub supplier: bool,
    pub professional: bool,
    pub active: bool,
    pub council: &'a str,
    pub registration: &'a str,
    pub uf: &'a str,
    pub valid_from: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
    pub data: &'a str,
}
#[derive(sqlx::FromRow)]
pub struct StoredRegistryReceipt {
    pub actor_id: Uuid,
    pub fingerprint: Vec<u8>,
    pub result: String,
}
#[derive(sqlx::FromRow)]
pub struct StoredPartyRevision {
    pub version: i64,
    pub before_data: Option<String>,
    pub after_data: String,
    pub actor: String,
    pub recorded_at: DateTime<Utc>,
    pub reason: String,
}
#[derive(sqlx::FromRow)]
pub struct StoredBankVersion {
    pub version: i64,
    pub account: String,
    pub actor: String,
    pub recorded_at: DateTime<Utc>,
    pub reason: String,
}

impl AuthTransaction {
    pub async fn parties_page(
        &mut self,
        tenant: Uuid,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<StoredParty>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,data,version FROM canteiro.parties WHERE tenant_id=$1 AND (strpos(lower(name),lower($2))>0 OR strpos(COALESCE(tax_id,''),upper($2))>0) ORDER BY lower(name),id LIMIT $3 OFFSET $4")
            .bind(tenant).bind(query).bind(limit.clamp(1,100)).bind(offset.max(0)).fetch_all(&mut *self.tx).await?)
    }
    pub async fn parties_count(
        &mut self,
        tenant: Uuid,
        query: &str,
    ) -> Result<i64, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM canteiro.parties WHERE tenant_id=$1 AND (strpos(lower(name),lower($2))>0 OR strpos(COALESCE(tax_id,''),upper($2))>0)").bind(tenant).bind(query).fetch_one(&mut *self.tx).await?)
    }
    pub async fn party_get(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredParty>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT id,data,version FROM canteiro.parties WHERE tenant_id=$1 AND id=$2 FOR UPDATE",
        )
        .bind(tenant)
        .bind(id)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn party_by_tax_id(
        &mut self,
        tenant: Uuid,
        tax_id: &str,
    ) -> Result<Option<StoredParty>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT id,data,version FROM canteiro.parties WHERE tenant_id=$1 AND tax_id=$2",
        )
        .bind(tenant)
        .bind(tax_id)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn party_write(&mut self, v: &PartyWrite<'_>) -> Result<bool, AuthStoreError> {
        let query = if v.expected.is_some() {
            "UPDATE canteiro.parties SET kind=$4,tax_id=$5,name=$6,customer=$7,supplier=$8,professional=$9,active=$10,council=$11,registration=$12,uf=$13,valid_from=$14,valid_until=$15,data=$16,version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$17 AND version<9223372036854775807 AND created_by IS NOT NULL AND $3::uuid IS NOT NULL"
        } else {
            "INSERT INTO canteiro.parties(tenant_id,id,created_by,kind,tax_id,name,customer,supplier,professional,active,council,registration,uf,valid_from,valid_until,data) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16 WHERE $17::bigint IS NULL ON CONFLICT(tenant_id,id) DO NOTHING"
        };
        Ok(sqlx::query(query)
            .bind(v.tenant)
            .bind(v.id)
            .bind(v.actor)
            .bind(v.kind)
            .bind(v.tax_id)
            .bind(v.name)
            .bind(v.customer)
            .bind(v.supplier)
            .bind(v.professional)
            .bind(v.active)
            .bind(v.council)
            .bind(v.registration)
            .bind(v.uf)
            .bind(v.valid_from)
            .bind(v.valid_until)
            .bind(v.data)
            .bind(v.expected)
            .execute(&mut *self.tx)
            .await?
            .rows_affected()
            == 1)
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn party_record_revision(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        version: i64,
        before: Option<&str>,
        after: &str,
        actor: Uuid,
        reason: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.party_history(tenant_id,party_id,version,before_data,after_data,actor_id,reason) VALUES($1,$2,$3,$4,$5,$6,$7)").bind(tenant).bind(id).bind(version).bind(before).bind(after).bind(actor).bind(reason).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn party_history(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Vec<StoredPartyRevision>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT h.version,h.before_data,h.after_data,u.email AS actor,h.recorded_at,h.reason FROM canteiro.party_history h JOIN canteiro_auth.users u ON u.id=h.actor_id WHERE h.tenant_id=$1 AND h.party_id=$2 ORDER BY h.version DESC LIMIT 100").bind(tenant).bind(id).fetch_all(&mut *self.tx).await?)
    }
    pub async fn party_bank_versions(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Vec<StoredBankVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT h.version,h.account,u.email AS actor,h.recorded_at,h.reason FROM canteiro.party_bank_versions h JOIN canteiro_auth.users u ON u.id=h.actor_id WHERE h.tenant_id=$1 AND h.party_id=$2 ORDER BY h.version DESC LIMIT 100").bind(tenant).bind(id).fetch_all(&mut *self.tx).await?)
    }
    pub async fn party_record_bank(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        version: i64,
        account: &str,
        actor: Uuid,
        reason: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.party_bank_versions(tenant_id,party_id,version,account,actor_id,reason) VALUES($1,$2,$3,$4,$5,$6)").bind(tenant).bind(id).bind(version).bind(account).bind(actor).bind(reason).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn registry_receipt(
        &mut self,
        tenant: Uuid,
        act: &str,
        key: Uuid,
    ) -> Result<Option<StoredRegistryReceipt>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT actor_id,fingerprint,result FROM canteiro.registry_operations WHERE tenant_id=$1 AND act=$2 AND operation_key=$3").bind(tenant).bind(act).bind(key).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn registry_record(
        &mut self,
        tenant: Uuid,
        act: &str,
        key: Uuid,
        actor: Uuid,
        fingerprint: &[u8],
        result: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.registry_operations(tenant_id,act,operation_key,actor_id,fingerprint,result) VALUES($1,$2,$3,$4,$5,$6)").bind(tenant).bind(act).bind(key).bind(actor).bind(fingerprint).bind(result).execute(&mut *self.tx).await?;
        Ok(())
    }
}
