//! Contratos públicos de arquivos; conteúdo de quarentena nunca sai nestes tipos.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DocumentScope {
    pub organization_id: String,
    pub context_version: i64,
    pub scope_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentScopeOption {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub can_upload: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentsContext {
    pub csrf: String,
    pub organization_id: Option<String>,
    pub organization_name: Option<String>,
    pub context_version: i64,
    pub scopes: Vec<DocumentScopeOption>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UploadDocument {
    pub scope: DocumentScope,
    pub operation_key: String,
    pub document_id: Option<String>,
    pub expected_version: i64,
    pub file_name: String,
    pub description: String,
    pub category: String,
    pub content_base64: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetryDocumentUpload {
    pub scope: DocumentScope,
    pub upload_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    Quarantine,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentUploadResult {
    pub upload_id: String,
    pub status: UploadStatus,
    pub document_id: Option<String>,
    pub version: Option<i64>,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentSignature {
    pub signer: String,
    pub signed_at: String,
    pub evidence: String,
    pub recorded_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentVersion {
    pub document_id: String,
    pub version: i64,
    pub active: bool,
    pub file_name: String,
    pub description: String,
    pub category: String,
    pub sha256: String,
    pub content_type: String,
    pub size: i64,
    pub author: String,
    pub created_at: String,
    pub signatures: Vec<DocumentSignature>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentUploadSummary {
    pub id: String,
    pub file_name: String,
    pub status: UploadStatus,
    pub message: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentsList {
    pub versions: Vec<DocumentVersion>,
    pub uploads: Vec<DocumentUploadSummary>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentLinkRequest {
    pub scope: DocumentScope,
    pub document_id: String,
    pub version: i64,
    #[serde(default)]
    pub preview: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DocumentLink {
    pub url: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RecordDocumentSignature {
    pub scope: DocumentScope,
    pub document_id: String,
    pub version: i64,
    pub sha256: String,
    pub signer: String,
    pub signed_at: String,
    pub evidence: String,
}
