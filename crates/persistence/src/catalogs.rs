//! Fonte de preços por edição publicada, acessível somente com escopo autorizado.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::NaiveDate;
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredCatalogHeader {
    pub scope_id: Uuid,
    pub catalog_id: String,
    pub edition: i32,
    pub source_title: String,
    pub source_organization: String,
    pub license: String,
    pub locality: String,
    pub tax_regime: String,
    pub competence: NaiveDate,
    pub published_on: NaiveDate,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredCatalogLine {
    pub code: String,
    pub description: String,
    pub category: String,
    pub unit: String,
    pub price: String,
}
pub struct NewCatalogEdition<'a> {
    pub tenant: Uuid,
    pub scope: Uuid,
    pub catalog_id: &'a str,
    pub edition: i32,
    pub source_title: &'a str,
    pub source_organization: &'a str,
    pub license: &'a str,
    pub locality: &'a str,
    pub tax_regime: &'a str,
    pub competence: NaiveDate,
    pub published_on: NaiveDate,
    pub actor: Uuid,
}
pub struct NewCatalogLine<'a> {
    pub code: &'a str,
    pub description: &'a str,
    pub category: &'a str,
    pub unit: &'a str,
    pub price: &'a str,
}
impl AuthTransaction {
    pub async fn catalog_headers(
        &mut self,
        tenant: Uuid,
        scopes: &[Uuid],
    ) -> Result<Vec<StoredCatalogHeader>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT c.scope_id,e.catalog_id,e.edition,e.source_title,e.source_organization,
             e.license,e.locality,e.tax_regime,e.competence,e.published_on
             FROM canteiro.cost_catalogs c JOIN canteiro.cost_catalog_editions e
               ON e.tenant_id=c.tenant_id AND e.catalog_id=c.catalog_id
             WHERE c.tenant_id=$1 AND c.scope_id=ANY($2) AND e.state='published'
             ORDER BY e.competence DESC,e.catalog_id,e.edition DESC LIMIT 200",
        )
        .bind(tenant)
        .bind(scopes)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    /// Consulta pontual: não depende do limite da listagem de cabeçalhos.
    pub async fn catalog_header(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        catalog: &str,
        edition: i32,
    ) -> Result<Option<StoredCatalogHeader>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT c.scope_id,e.catalog_id,e.edition,e.source_title,e.source_organization,
             e.license,e.locality,e.tax_regime,e.competence,e.published_on
             FROM canteiro.cost_catalogs c JOIN canteiro.cost_catalog_editions e
               ON e.tenant_id=c.tenant_id AND e.catalog_id=c.catalog_id
             WHERE c.tenant_id=$1 AND c.scope_id=$2 AND e.catalog_id=$3
               AND e.edition=$4 AND e.state='published'",
        )
        .bind(tenant)
        .bind(scope)
        .bind(catalog)
        .bind(edition)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    /// Seleciona a edição anterior por versão, mesmo fora da listagem recente.
    pub async fn catalog_previous_header(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        catalog: &str,
        edition: i32,
    ) -> Result<Option<StoredCatalogHeader>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT c.scope_id,e.catalog_id,e.edition,e.source_title,e.source_organization,
             e.license,e.locality,e.tax_regime,e.competence,e.published_on
             FROM canteiro.cost_catalogs c JOIN canteiro.cost_catalog_editions e
               ON e.tenant_id=c.tenant_id AND e.catalog_id=c.catalog_id
             WHERE c.tenant_id=$1 AND c.scope_id=$2 AND e.catalog_id=$3
               AND e.edition<$4 AND e.state='published'
             ORDER BY e.edition DESC LIMIT 1",
        )
        .bind(tenant)
        .bind(scope)
        .bind(catalog)
        .bind(edition)
        .fetch_optional(&mut *self.tx)
        .await?)
    }
    pub async fn catalog_lines(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        catalog: &str,
        edition: i32,
    ) -> Result<Vec<StoredCatalogLine>, AuthStoreError> {
        Ok(sqlx::query_as(
            "SELECT i.code,i.description,i.category,i.unit,i.price::TEXT AS price
             FROM canteiro.cost_catalog_items i
             JOIN canteiro.cost_catalogs c
               ON c.tenant_id=i.tenant_id AND c.catalog_id=i.catalog_id
             JOIN canteiro.cost_catalog_editions e
               ON e.tenant_id=i.tenant_id AND e.catalog_id=i.catalog_id AND e.edition=i.edition
             WHERE i.tenant_id=$1 AND c.scope_id=$2 AND i.catalog_id=$3 AND i.edition=$4
             AND e.state='published' ORDER BY i.code",
        )
        .bind(tenant)
        .bind(scope)
        .bind(catalog)
        .bind(edition)
        .fetch_all(&mut *self.tx)
        .await?)
    }
    pub async fn catalog_edition_exists(
        &mut self,
        tenant: Uuid,
        catalog: &str,
        edition: i32,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM canteiro.cost_catalog_editions
                WHERE tenant_id=$1 AND catalog_id=$2 AND edition=$3)",
        )
        .bind(tenant)
        .bind(catalog)
        .bind(edition)
        .fetch_one(&mut *self.tx)
        .await?)
    }
    /// O estado published é alcançado somente após a inserção de todas as linhas,
    /// conferida por gatilho. Um erro em qualquer linha desfaz a transação inteira.
    pub async fn catalog_publish(
        &mut self,
        header: &NewCatalogEdition<'_>,
        items: &[NewCatalogLine<'_>],
    ) -> Result<bool, AuthStoreError> {
        sqlx::query(
            "INSERT INTO canteiro.cost_catalogs
            (tenant_id,catalog_id,scope_id,title,created_by)
            VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING",
        )
        .bind(header.tenant)
        .bind(header.catalog_id)
        .bind(header.scope)
        .bind(header.source_title)
        .bind(header.actor)
        .execute(&mut *self.tx)
        .await?;
        // O cabeçalho é imutável para a identidade de execução (SELECT/INSERT).
        // A chave única arbitra inserções concorrentes; não requer privilégio UPDATE.
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT scope_id FROM canteiro.cost_catalogs
             WHERE tenant_id=$1 AND catalog_id=$2",
        )
        .bind(header.tenant)
        .bind(header.catalog_id)
        .fetch_optional(&mut *self.tx)
        .await?;
        if exists != Some(header.scope) {
            return Ok(false);
        }
        let added = sqlx::query(
            "INSERT INTO canteiro.cost_catalog_editions
            (tenant_id,catalog_id,edition,source_title,source_organization,license,
            locality,tax_regime,competence,published_on,expected_items,created_by)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT DO NOTHING",
        )
        .bind(header.tenant)
        .bind(header.catalog_id)
        .bind(header.edition)
        .bind(header.source_title)
        .bind(header.source_organization)
        .bind(header.license)
        .bind(header.locality)
        .bind(header.tax_regime)
        .bind(header.competence)
        .bind(header.published_on)
        .bind(i32::try_from(items.len()).unwrap_or(i32::MAX))
        .bind(header.actor)
        .execute(&mut *self.tx)
        .await?
        .rows_affected();
        if added != 1 {
            return Ok(false);
        }
        // Cada bloco usa uma única ida ao banco, sem confirmação intermediária.
        // Gatilhos, restrições e RLS continuam sendo aplicados a cada linha.
        for chunk in items.chunks(1_000) {
            let codes: Vec<_> = chunk.iter().map(|item| item.code).collect();
            let descriptions: Vec<_> = chunk.iter().map(|item| item.description).collect();
            let categories: Vec<_> = chunk.iter().map(|item| item.category).collect();
            let units: Vec<_> = chunk.iter().map(|item| item.unit).collect();
            let prices: Vec<_> = chunk.iter().map(|item| item.price).collect();
            sqlx::query(
                "INSERT INTO canteiro.cost_catalog_items
                (tenant_id,catalog_id,edition,code,description,category,unit,price)
                SELECT $1,$2,$3,i.code,i.description,i.category,i.unit,i.price::NUMERIC
                FROM unnest($4::TEXT[],$5::TEXT[],$6::TEXT[],$7::TEXT[],$8::TEXT[])
                    AS i(code,description,category,unit,price)",
            )
            .bind(header.tenant)
            .bind(header.catalog_id)
            .bind(header.edition)
            .bind(&codes)
            .bind(&descriptions)
            .bind(&categories)
            .bind(&units)
            .bind(&prices)
            .execute(&mut *self.tx)
            .await?;
        }
        sqlx::query(
            "UPDATE canteiro.cost_catalog_editions SET state='published'
             WHERE tenant_id=$1 AND catalog_id=$2 AND edition=$3",
        )
        .bind(header.tenant)
        .bind(header.catalog_id)
        .bind(header.edition)
        .execute(&mut *self.tx)
        .await?;
        Ok(true)
    }
}
