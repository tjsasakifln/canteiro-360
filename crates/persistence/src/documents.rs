//! Armazenamento documental privado. O chamador revalida a sessão e o escopo.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct StoredDocument {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub active_version: i64,
}

#[derive(sqlx::FromRow)]
pub struct StoredDocumentVersion {
    pub document_id: Uuid,
    pub scope_id: Uuid,
    pub version: i64,
    pub active_version: i64,
    pub sha256: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub file_name: String,
    pub description: String,
    pub category: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct StoredDocumentMetadata {
    pub document_id: Uuid,
    pub version: i64,
    pub active_version: i64,
    pub sha256: String,
    pub content_type: String,
    pub size: i64,
    pub file_name: String,
    pub description: String,
    pub category: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct StoredDocumentUpload {
    pub id: Uuid,
    pub operation_key: Uuid,
    pub scope_id: Uuid,
    pub actor_id: Uuid,
    pub target_document: Option<Uuid>,
    pub expected_version: i64,
    pub fingerprint: Vec<u8>,
    pub sha256: String,
    pub content: Option<Vec<u8>>,
    pub content_type: String,
    pub file_name: String,
    pub description: String,
    pub category: String,
    pub status: String,
    pub reason: String,
    pub result_document: Option<Uuid>,
    pub result_version: Option<i64>,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct StoredDocumentSignature {
    pub signer: String,
    pub signed_at: DateTime<Utc>,
    pub evidence: String,
    pub recorded_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct StoredDocumentTicket {
    pub document_id: Uuid,
    pub version: i64,
    pub preview: bool,
    pub expires_at: DateTime<Utc>,
}

pub struct NewDocumentUpload<'a> {
    pub tenant: Uuid,
    pub id: Uuid,
    pub operation_key: Uuid,
    pub scope: Uuid,
    pub actor: Uuid,
    pub target: Option<Uuid>,
    pub expected_version: i64,
    pub fingerprint: &'a [u8],
    pub sha256: &'a str,
    pub content: &'a [u8],
    pub content_type: &'a str,
    pub file_name: &'a str,
    pub description: &'a str,
    pub category: &'a str,
}

impl AuthTransaction {
    pub async fn document(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredDocument>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,scope_id,active_version FROM canteiro.documents WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn document_version(
        &mut self,
        tenant: Uuid,
        document: Uuid,
        version: i64,
    ) -> Result<Option<StoredDocumentVersion>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT v.document_id,d.scope_id,v.version,d.active_version,v.sha256,v.content,v.content_type,v.file_name,v.description,v.category,v.created_by,v.created_at FROM canteiro.document_versions v JOIN canteiro.documents d ON d.tenant_id=v.tenant_id AND d.id=v.document_id WHERE v.tenant_id=$1 AND v.document_id=$2 AND v.version=$3")
            .bind(tenant).bind(document).bind(version).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn document_metadata(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        before: Option<DateTime<Utc>>,
    ) -> Result<Vec<StoredDocumentMetadata>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT v.document_id,v.version,d.active_version,v.sha256,v.content_type,octet_length(v.content)::bigint AS size,v.file_name,v.description,v.category,u.email AS author,v.created_at FROM canteiro.document_versions v JOIN canteiro.documents d ON d.tenant_id=v.tenant_id AND d.id=v.document_id JOIN canteiro_auth.users u ON u.id=v.created_by WHERE v.tenant_id=$1 AND d.scope_id=$2 AND ($3::timestamptz IS NULL OR v.created_at<$3) ORDER BY v.created_at DESC,v.document_id,v.version DESC LIMIT 101")
            .bind(tenant).bind(scope).bind(before).fetch_all(&mut *self.tx).await?)
    }

    pub async fn document_upload_by_key(
        &mut self,
        tenant: Uuid,
        key: Uuid,
    ) -> Result<Option<StoredDocumentUpload>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,operation_key,scope_id,actor_id,target_document,expected_version,fingerprint,sha256,content,content_type,file_name,description,category,status,reason,result_document,result_version,created_at FROM canteiro.document_uploads WHERE tenant_id=$1 AND operation_key=$2 FOR UPDATE")
            .bind(tenant).bind(key).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn document_upload(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredDocumentUpload>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,operation_key,scope_id,actor_id,target_document,expected_version,fingerprint,sha256,content,content_type,file_name,description,category,status,reason,result_document,result_version,created_at FROM canteiro.document_uploads WHERE tenant_id=$1 AND id=$2 FOR UPDATE")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn document_pending_uploads(
        &mut self,
        tenant: Uuid,
        scope: Uuid,
        actor: Uuid,
    ) -> Result<Vec<StoredDocumentUpload>, AuthStoreError> {
        // Não carregar bytes dos arquivos para exibir a lista de andamento.
        Ok(sqlx::query_as("SELECT id,operation_key,scope_id,actor_id,target_document,expected_version,fingerprint,sha256,NULL::bytea AS content,content_type,file_name,description,category,status,reason,result_document,result_version,created_at FROM canteiro.document_uploads WHERE tenant_id=$1 AND scope_id=$2 AND actor_id=$3 AND status<>'accepted' ORDER BY created_at DESC,id LIMIT 100")
            .bind(tenant).bind(scope).bind(actor).fetch_all(&mut *self.tx).await?)
    }

    pub async fn document_quarantine_bytes(&mut self, tenant: Uuid) -> Result<i64, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT COALESCE(sum(octet_length(content)),0)::bigint FROM canteiro.document_uploads WHERE tenant_id=$1 AND status='quarantine'")
            .bind(tenant).fetch_one(&mut *self.tx).await?)
    }

    pub async fn insert_document_upload(
        &mut self,
        v: &NewDocumentUpload<'_>,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.document_uploads(tenant_id,id,operation_key,scope_id,actor_id,target_document,expected_version,fingerprint,sha256,content,content_type,file_name,description,category) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
            .bind(v.tenant).bind(v.id).bind(v.operation_key).bind(v.scope).bind(v.actor).bind(v.target).bind(v.expected_version).bind(v.fingerprint).bind(v.sha256).bind(v.content).bind(v.content_type).bind(v.file_name).bind(v.description).bind(v.category)
            .execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn document_scan_unavailable(
        &mut self,
        tenant: Uuid,
        upload: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.document_uploads SET reason='scanner_unavailable' WHERE tenant_id=$1 AND id=$2 AND status='quarantine'")
            .bind(tenant).bind(upload).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn reject_document_upload(
        &mut self,
        tenant: Uuid,
        upload: Uuid,
        reason: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.document_uploads SET status='rejected',reason=$3,content=NULL WHERE tenant_id=$1 AND id=$2 AND status='quarantine'")
            .bind(tenant).bind(upload).bind(reason).execute(&mut *self.tx).await?;
        Ok(())
    }

    /// Deve ser chamada sob bloqueio da organização e após autorização atual.
    pub async fn accept_document_upload(
        &mut self,
        tenant: Uuid,
        upload: &StoredDocumentUpload,
        document: Uuid,
        version: i64,
        actor: Uuid,
    ) -> Result<(), AuthStoreError> {
        if upload.target_document.is_none() {
            sqlx::query("INSERT INTO canteiro.documents(tenant_id,id,scope_id,active_version,created_by) VALUES($1,$2,$3,$4,$5)")
                .bind(tenant).bind(document).bind(upload.scope_id).bind(version).bind(actor).execute(&mut *self.tx).await?;
        } else {
            let affected = sqlx::query("UPDATE canteiro.documents SET active_version=$3 WHERE tenant_id=$1 AND id=$2 AND active_version=$4")
                .bind(tenant).bind(document).bind(version).bind(upload.expected_version).execute(&mut *self.tx).await?.rows_affected();
            if affected != 1 {
                return Err(AuthStoreError);
            }
        }
        sqlx::query("INSERT INTO canteiro.document_versions(tenant_id,document_id,version,sha256,content,content_type,file_name,description,category,created_by,scanner) SELECT tenant_id,$3,$4,sha256,content,content_type,file_name,description,category,$5,'clamav-instream' FROM canteiro.document_uploads WHERE tenant_id=$1 AND id=$2 AND status='quarantine'")
            .bind(tenant).bind(upload.id).bind(document).bind(version).bind(actor).execute(&mut *self.tx).await?;
        self.document_history(
            tenant,
            document,
            version,
            if version > 1 { Some(version - 1) } else { None },
            &upload.sha256,
            actor,
            "version_created",
        )
        .await?;
        sqlx::query("UPDATE canteiro.document_uploads SET status='accepted',reason='accepted',content=NULL,result_document=$3,result_version=$4 WHERE tenant_id=$1 AND id=$2 AND status='quarantine'")
            .bind(tenant).bind(upload.id).bind(document).bind(version).execute(&mut *self.tx).await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn document_history(
        &mut self,
        tenant: Uuid,
        document: Uuid,
        version: i64,
        previous: Option<i64>,
        sha256: &str,
        actor: Uuid,
        event: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.document_history(tenant_id,id,document_id,version,previous_version,sha256,actor_id,event) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(tenant).bind(Uuid::new_v4()).bind(document).bind(version).bind(previous).bind(sha256).bind(actor).bind(event).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn document_signatures(
        &mut self,
        tenant: Uuid,
        document: Uuid,
        version: i64,
    ) -> Result<Vec<StoredDocumentSignature>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT signer,signed_at,evidence,recorded_at FROM canteiro.document_signatures WHERE tenant_id=$1 AND document_id=$2 AND version=$3 ORDER BY recorded_at,signer")
            .bind(tenant).bind(document).bind(version).fetch_all(&mut *self.tx).await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_document_signature(
        &mut self,
        tenant: Uuid,
        document: Uuid,
        version: i64,
        sha256: &str,
        evidence_key: &[u8],
        signer: &str,
        signed_at: DateTime<Utc>,
        evidence: &str,
        actor: Uuid,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("INSERT INTO canteiro.document_signatures(tenant_id,document_id,version,sha256,evidence_key,signer,signed_at,evidence,actor_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT(tenant_id,document_id,version,evidence_key) DO NOTHING")
            .bind(tenant).bind(document).bind(version).bind(sha256).bind(evidence_key).bind(signer).bind(signed_at).bind(evidence).bind(actor).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn issue_document_ticket(
        &mut self,
        tenant: Uuid,
        hash: &[u8],
        document: Uuid,
        version: i64,
        session: Uuid,
        actor: Uuid,
        expires: DateTime<Utc>,
        preview: bool,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("DELETE FROM canteiro.document_download_tickets WHERE tenant_id=$1 AND expires_at<=clock_timestamp()")
            .bind(tenant).execute(&mut *self.tx).await?;
        sqlx::query("INSERT INTO canteiro.document_download_tickets(tenant_id,token_hash,document_id,version,session_id,actor_id,expires_at,preview) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(tenant).bind(hash).bind(document).bind(version).bind(session).bind(actor).bind(expires).bind(preview).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn document_ticket(
        &mut self,
        tenant: Uuid,
        hash: &[u8],
        session: Uuid,
        actor: Uuid,
    ) -> Result<Option<StoredDocumentTicket>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT document_id,version,preview,expires_at FROM canteiro.document_download_tickets WHERE tenant_id=$1 AND token_hash=$2 AND session_id=$3 AND actor_id=$4 AND expires_at>clock_timestamp()")
            .bind(tenant).bind(hash).bind(session).bind(actor).fetch_optional(&mut *self.tx).await?)
    }
}
