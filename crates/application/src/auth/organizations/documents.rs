//! Documentos privados. Quarentena e autorização são independentes da assinatura.
mod pdf;
mod scanner;
use super::access::{Snapshot, zero};
use super::{
    Actor, AuthError, AuthErrorKind, AuthService, authorize, context_changed, crypto, id, invalid,
    stale,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use canteiro_contracts::{access::AccessAction, documents::*};
use canteiro_domain::documents::{DocumentError, MAX_DOCUMENT_BYTES, inspect_content};
use canteiro_persistence::{
    auth::AuthTransaction,
    documents::{NewDocumentUpload, StoredDocumentUpload},
};
use chrono::{DateTime, Duration, Utc};
use scanner::ScanVerdict;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn unavailable_document() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "document_unavailable",
        "Este documento não está disponível para seu acesso. Confira a empresa e o vínculo com a obra, o contrato, o orçamento ou o projeto.",
    )
}

fn content_error(error: DocumentError) -> AuthError {
    if error == DocumentError::InspectionUnavailable {
        return AuthError::new(
            AuthErrorKind::Unavailable,
            "document_inspection_unavailable",
            "A análise do PDF não ficou disponível nesta tentativa. Preserve o arquivo e tente novamente.",
        );
    }
    let message = match error {
        DocumentError::Size => "Selecione um arquivo não vazio com até 5 MiB.",
        DocumentError::Name => {
            "Use um nome de arquivo simples, sem pastas, com até 180 caracteres."
        }
        DocumentError::Format => {
            "Use PDF sem conteúdo ativo, PNG, TXT ou CSV. A extensão precisa corresponder ao conteúdo."
        }
        DocumentError::Prohibited => {
            "O arquivo contém conteúdo ativo ou não permitido. Obtenha uma cópia segura com o emissor e preserve o original assinado."
        }
        DocumentError::RestrictedPdf => {
            "Este PDF usa recursos ainda não aceitos, como criptografia, objetos comprimidos ou tamanho de fluxo indireto. Preserve o original, especialmente se estiver assinado."
        }
        DocumentError::Complexity => {
            "Este PDF excede os limites de complexidade aceitos. Preserve o original, especialmente se estiver assinado."
        }
        DocumentError::InspectionUnavailable => unreachable!(),
        DocumentError::Structure => {
            "O arquivo está incompleto ou sua estrutura não é válida. Gere uma nova cópia."
        }
    };
    invalid(message)
}

fn edit_permitted(
    snapshot: &Snapshot,
    actor: Uuid,
    scope: Uuid,
    kind: &str,
    at: DateTime<Utc>,
) -> bool {
    let actions: &[AccessAction] = match kind {
        "work" => &[AccessAction::ManagePortfolio, AccessAction::RecordExecution],
        "contract" => &[AccessAction::EditContractRecord],
        "budget" => &[AccessAction::EditBudget],
        "project" => &[AccessAction::EditProject],
        _ => &[],
    };
    actions
        .iter()
        .any(|action| snapshot.permits(actor, scope, *action, zero(), at))
}

struct DocumentAccess {
    tenant: Uuid,
    actor: Actor,
    scope: Uuid,
    kind: String,
    rights: Snapshot,
    member_until: Option<DateTime<Utc>>,
    edit: bool,
}

impl DocumentAccess {
    fn permitted(&self, now: DateTime<Utc>) -> bool {
        self.member_until.is_none_or(|end| end > now)
            && self
                .rights
                .permits(self.actor.id, self.scope, AccessAction::View, zero(), now)
            && (!self.edit
                || edit_permitted(&self.rights, self.actor.id, self.scope, &self.kind, now))
    }
    fn owns_upload(&self, upload: &StoredDocumentUpload) -> Result<(), AuthError> {
        if upload.scope_id != self.scope || upload.actor_id != self.actor.id {
            Err(unavailable_document())
        } else {
            Ok(())
        }
    }
}

fn upload_result(row: &StoredDocumentUpload) -> Result<DocumentUploadResult, AuthError> {
    let status = match row.status.as_str() {
        "quarantine" => UploadStatus::Quarantine,
        "accepted" => UploadStatus::Accepted,
        "rejected" => UploadStatus::Rejected,
        _ => return Err(AuthError::unavailable()),
    };
    let message = match row.reason.as_str() {
        "accepted" => {
            "Arquivo verificado e versão salva. As versões e assinaturas anteriores foram preservadas."
        }
        "prohibited" => {
            "Arquivo recusado pela verificação de segurança. Envie uma nova cópia confiável; os dados do formulário continuam disponíveis."
        }
        "version_conflict" => {
            "Outra versão foi salva durante o envio. Confira a versão ativa e anexe novamente com base nela."
        }
        _ => {
            "Arquivo recebido e protegido em quarentena. A verificação de segurança ainda não terminou. Ele não pode ser baixado nem usado como anexo válido; tente verificar novamente."
        }
    };
    Ok(DocumentUploadResult {
        upload_id: row.id.to_string(),
        status,
        document_id: row.result_document.map(|v| v.to_string()),
        version: row.result_version,
        message: message.into(),
    })
}

impl AuthService {
    async fn document_access(
        &self,
        token: &str,
        scope: &DocumentScope,
        edit: bool,
    ) -> Result<(AuthTransaction, DocumentAccess), AuthError> {
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let tenant = id(&scope.organization_id)?;
        let resource = id(&scope.scope_id)?;
        let selected = tx.org_selection(actor.session).await?;
        if selected.tenant_id != Some(tenant) || selected.generation != scope.context_version {
            return Err(context_changed());
        }
        let (_, member) = authorize(&mut tx, &actor, tenant).await?;
        let rights = Snapshot::load_permissions(&mut tx, tenant).await?;
        let kind = rights
            .scopes
            .iter()
            .find(|s| s.id == resource)
            .map(|s| s.kind.clone())
            .ok_or_else(unavailable_document)?;
        let access = DocumentAccess {
            tenant,
            actor,
            scope: resource,
            kind,
            rights,
            member_until: member.valid_until,
            edit,
        };
        self.document_revalidate(token, &mut tx, &access).await?;
        Ok((tx, access))
    }

    async fn document_revalidate(
        &self,
        token: &str,
        tx: &mut AuthTransaction,
        access: &DocumentAccess,
    ) -> Result<(), AuthError> {
        self.locked(tx, token).await?;
        if !access.permitted(tx.now().await?) {
            return Err(unavailable_document());
        }
        Ok(())
    }

    pub async fn documents_context(&self, token: &str) -> Result<DocumentsContext, AuthError> {
        self.limit("documents-read", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let selected = tx.org_selection(actor.session).await?;
        let mut result = DocumentsContext {
            csrf: self.csrf(token),
            organization_id: None,
            organization_name: None,
            context_version: selected.generation,
            scopes: vec![],
        };
        if let Some(tenant) = selected.tenant_id {
            let (organization, member) = authorize(&mut tx, &actor, tenant).await?;
            let rights = Snapshot::load_permissions(&mut tx, tenant).await?;
            self.locked(&mut tx, token).await?;
            let now = tx.now().await?;
            if member.valid_until.is_some_and(|until| until <= now) {
                return Err(unavailable_document());
            }
            result.organization_id = Some(tenant.to_string());
            result.organization_name = Some(organization.name);
            result.scopes = rights
                .scopes
                .iter()
                .filter(|s| rights.permits(actor.id, s.id, AccessAction::View, zero(), now))
                .map(|s| DocumentScopeOption {
                    id: s.id.to_string(),
                    title: s.name.clone(),
                    kind: s.kind.clone(),
                    can_upload: edit_permitted(&rights, actor.id, s.id, &s.kind, now),
                })
                .collect();
        }
        tx.commit().await?;
        Ok(result)
    }

    pub async fn documents_list(
        &self,
        token: &str,
        scope: &DocumentScope,
        before: Option<&str>,
    ) -> Result<DocumentsList, AuthError> {
        self.limit("documents-read", token, 120, 60).await?;
        let (mut tx, access) = self.document_access(token, scope, false).await?;
        let before = before
            .map(|v| {
                DateTime::parse_from_rfc3339(v)
                    .map(|v| v.with_timezone(&Utc))
                    .map_err(|_| invalid("Atualize a lista para continuar."))
            })
            .transpose()?;
        let mut rows = tx
            .document_metadata(access.tenant, access.scope, before)
            .await?;
        let truncated = rows.len() > 100;
        if truncated {
            rows.pop();
        }
        let mut versions = vec![];
        for row in rows {
            let signatures = tx
                .document_signatures(access.tenant, row.document_id, row.version)
                .await?
                .into_iter()
                .map(|s| DocumentSignature {
                    signer: s.signer,
                    signed_at: s.signed_at.to_rfc3339(),
                    evidence: s.evidence,
                    recorded_at: s.recorded_at.to_rfc3339(),
                })
                .collect();
            versions.push(DocumentVersion {
                document_id: row.document_id.to_string(),
                version: row.version,
                active: row.version == row.active_version,
                file_name: row.file_name,
                description: row.description,
                category: row.category,
                sha256: row.sha256,
                content_type: row.content_type,
                size: row.size,
                author: row.author,
                created_at: row.created_at.to_rfc3339(),
                signatures,
            });
        }
        let uploads = tx
            .document_pending_uploads(access.tenant, access.scope, access.actor.id)
            .await?
            .iter()
            .map(|u| {
                let result = upload_result(u)?;
                Ok(DocumentUploadSummary {
                    id: u.id.to_string(),
                    file_name: u.file_name.clone(),
                    status: result.status,
                    message: result.message,
                    created_at: u.created_at.to_rfc3339(),
                })
            })
            .collect::<Result<Vec<_>, AuthError>>()?;
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        Ok(DocumentsList {
            versions,
            uploads,
            truncated,
        })
    }

    pub async fn document_upload_new(
        &self,
        token: &str,
        input: UploadDocument,
    ) -> Result<DocumentUploadResult, AuthError> {
        self.limit("documents-upload", token, 30, 60).await?;
        // Autorizar antes de decodificar ou analisar conteúdo controlado pelo cliente.
        // A transação é encerrada para não bloquear a organização durante a análise.
        let (tx, _) = self.document_access(token, &input.scope, true).await?;
        tx.commit().await?;
        if input.content_base64.len() > MAX_DOCUMENT_BYTES.div_ceil(3) * 4 {
            return Err(content_error(DocumentError::Size));
        }
        let bytes = STANDARD.decode(&input.content_base64).map_err(|_| {
            invalid("O envio está incompleto. Selecione o arquivo e tente novamente.")
        })?;
        let format = inspect_content(&input.file_name, &bytes).map_err(content_error)?;
        if format == canteiro_domain::documents::DocumentFormat::Pdf {
            pdf::validate_bounded(&bytes).await.map_err(content_error)?;
        }
        let description = input.description.trim();
        if !(2..=500).contains(&description.chars().count())
            || description.chars().any(char::is_control)
        {
            return Err(invalid(
                "Descreva o documento com 2 a 500 caracteres, sem quebras de linha.",
            ));
        }
        if !matches!(
            input.category.as_str(),
            "technical" | "contractual" | "budget" | "evidence" | "other"
        ) {
            return Err(invalid("Selecione a categoria do documento."));
        }
        let key = id(&input.operation_key)?;
        if key.get_version() != Some(uuid::Version::Random) {
            return Err(invalid("Reabra a página para identificar o envio."));
        }
        let target = input.document_id.as_deref().map(id).transpose()?;
        if target.is_none() && input.expected_version != 0
            || target.is_some() && input.expected_version <= 0
        {
            return Err(invalid("Confira a versão que será substituída."));
        }
        let digest = Sha256::digest(&bytes);
        let sha256 = format!("{digest:x}");
        let fingerprint = Sha256::digest(
            serde_json::to_vec(&(
                target,
                input.expected_version,
                &input.file_name,
                description,
                &input.category,
                &sha256,
            ))
            .map_err(|_| AuthError::unavailable())?,
        )
        .to_vec();
        let (mut tx, access) = self.document_access(token, &input.scope, true).await?;
        let upload_id = if let Some(row) = tx.document_upload_by_key(access.tenant, key).await? {
            access.owns_upload(&row)?;
            if row.fingerprint != fingerprint {
                return Err(super::reliability::idempotency_conflict());
            }
            if row.status != "quarantine" {
                let result = upload_result(&row)?;
                self.document_revalidate(token, &mut tx, &access).await?;
                tx.commit().await?;
                return Ok(result);
            }
            row.id
        } else {
            if let Some(target) = target {
                let doc = tx
                    .document(access.tenant, target)
                    .await?
                    .filter(|d| d.scope_id == access.scope)
                    .ok_or_else(unavailable_document)?;
                if doc.active_version != input.expected_version {
                    return Err(stale());
                }
            }
            if tx.document_quarantine_bytes(access.tenant).await? + bytes.len() as i64
                > 100 * 1024 * 1024
            {
                return Err(invalid(
                    "A empresa tem 100 MiB aguardando verificação. Conclua os envios pendentes antes de enviar novos arquivos.",
                ));
            }
            let upload = Uuid::new_v4();
            tx.insert_document_upload(&NewDocumentUpload {
                tenant: access.tenant,
                id: upload,
                operation_key: key,
                scope: access.scope,
                actor: access.actor.id,
                target,
                expected_version: input.expected_version,
                fingerprint: &fingerprint,
                sha256: &sha256,
                content: &bytes,
                content_type: format.content_type(),
                file_name: &input.file_name,
                description,
                category: &input.category,
            })
            .await?;
            upload
        };
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        self.document_upload_retry_inner(token, &input.scope, upload_id)
            .await
    }

    pub async fn document_upload_retry(
        &self,
        token: &str,
        input: RetryDocumentUpload,
    ) -> Result<DocumentUploadResult, AuthError> {
        self.limit("documents-scan", token, 30, 60).await?;
        self.document_upload_retry_inner(token, &input.scope, id(&input.upload_id)?)
            .await
    }

    async fn document_upload_retry_inner(
        &self,
        token: &str,
        scope: &DocumentScope,
        upload: Uuid,
    ) -> Result<DocumentUploadResult, AuthError> {
        let (mut tx, access) = self.document_access(token, scope, true).await?;
        let row = tx
            .document_upload(access.tenant, upload)
            .await?
            .ok_or_else(unavailable_document)?;
        access.owns_upload(&row)?;
        if row.status != "quarantine" {
            let result = upload_result(&row)?;
            self.document_revalidate(token, &mut tx, &access).await?;
            tx.commit().await?;
            return Ok(result);
        }
        let bytes = row.content.ok_or_else(AuthError::unavailable)?;
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        // Não manter bloqueios da organização enquanto um serviço externo trabalha.
        let verdict = scanner::scan(&bytes).await;
        let (mut tx, access) = self.document_access(token, scope, true).await?;
        let row = tx
            .document_upload(access.tenant, upload)
            .await?
            .ok_or_else(unavailable_document)?;
        access.owns_upload(&row)?;
        if row.status == "quarantine" {
            match verdict {
                ScanVerdict::Unavailable => {
                    tx.document_scan_unavailable(access.tenant, upload).await?
                }
                ScanVerdict::Prohibited => {
                    tx.reject_document_upload(access.tenant, upload, "prohibited")
                        .await?
                }
                ScanVerdict::Clean => {
                    if row.sha256 != format!("{:x}", Sha256::digest(&bytes))
                        || row.content.as_deref() != Some(bytes.as_slice())
                    {
                        return Err(AuthError::unavailable());
                    }
                    let current = if let Some(document) = row.target_document {
                        Some(
                            tx.document(access.tenant, document)
                                .await?
                                .filter(|d| d.scope_id == access.scope)
                                .ok_or_else(unavailable_document)?,
                        )
                    } else {
                        None
                    };
                    if current
                        .as_ref()
                        .is_some_and(|d| d.active_version != row.expected_version)
                    {
                        tx.reject_document_upload(access.tenant, upload, "version_conflict")
                            .await?;
                    } else {
                        let document = row.target_document.unwrap_or_else(Uuid::new_v4);
                        let version = row
                            .expected_version
                            .checked_add(1)
                            .ok_or_else(AuthError::unavailable)?;
                        tx.accept_document_upload(
                            access.tenant,
                            &row,
                            document,
                            version,
                            access.actor.id,
                        )
                        .await?;
                    }
                }
            }
        }
        let result = upload_result(
            &tx.document_upload(access.tenant, upload)
                .await?
                .ok_or_else(AuthError::unavailable)?,
        )?;
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn document_download_link(
        &self,
        token: &str,
        input: DocumentLinkRequest,
    ) -> Result<DocumentLink, AuthError> {
        self.limit("documents-link", token, 60, 60).await?;
        let (mut tx, access) = self.document_access(token, &input.scope, false).await?;
        let document = id(&input.document_id)?;
        let row = tx
            .document_version(access.tenant, document, input.version)
            .await?
            .filter(|v| v.scope_id == access.scope)
            .ok_or_else(unavailable_document)?;
        if input.preview
            && !matches!(
                row.content_type.as_str(),
                "image/png" | "text/plain; charset=utf-8" | "text/csv; charset=utf-8"
            )
        {
            return Err(invalid(
                "A prévia está disponível para imagens PNG e textos. Baixe o PDF para consultar o arquivo verificado.",
            ));
        }
        let ticket = crypto::random_token();
        let expires = tx.now().await? + Duration::minutes(2);
        tx.issue_document_ticket(
            access.tenant,
            &crypto::hash("document-download", &ticket),
            document,
            input.version,
            access.actor.session,
            access.actor.id,
            expires,
            input.preview,
        )
        .await?;
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        Ok(DocumentLink {
            url: format!(
                "/api/documents/download/{ticket}?organization_id={}&context_version={}&scope_id={}",
                input.scope.organization_id, input.scope.context_version, input.scope.scope_id
            ),
            expires_at: expires.to_rfc3339(),
        })
    }

    pub async fn document_download(
        &self,
        token: &str,
        scope: &DocumentScope,
        ticket: &str,
    ) -> Result<DocumentDownload, AuthError> {
        self.limit("documents-download", token, 60, 60).await?;
        if !crypto::token_valid(ticket) {
            return Err(unavailable_document());
        }
        let (mut tx, access) = self.document_access(token, scope, false).await?;
        let ticket = tx
            .document_ticket(
                access.tenant,
                &crypto::hash("document-download", ticket),
                access.actor.session,
                access.actor.id,
            )
            .await?
            .ok_or_else(unavailable_document)?;
        let row = tx
            .document_version(access.tenant, ticket.document_id, ticket.version)
            .await?
            .filter(|v| v.scope_id == access.scope)
            .ok_or_else(unavailable_document)?;
        if format!("{:x}", Sha256::digest(&row.content)) != row.sha256 {
            return Err(AuthError::unavailable());
        }
        self.document_revalidate(token, &mut tx, &access).await?;
        if ticket.expires_at <= tx.now().await? {
            return Err(unavailable_document());
        }
        tx.commit().await?;
        Ok(DocumentDownload {
            content: row.content,
            content_type: row.content_type,
            file_name: row.file_name,
            sha256: row.sha256,
            preview: ticket.preview,
        })
    }

    pub async fn document_record_signature(
        &self,
        token: &str,
        input: RecordDocumentSignature,
    ) -> Result<(), AuthError> {
        self.limit("documents-signature", token, 30, 60).await?;
        let (mut tx, access) = self.document_access(token, &input.scope, true).await?;
        let document = id(&input.document_id)?;
        let row = tx
            .document_version(access.tenant, document, input.version)
            .await?
            .filter(|v| v.scope_id == access.scope)
            .ok_or_else(unavailable_document)?;
        if row.sha256 != input.sha256 {
            return Err(invalid(
                "O resumo do arquivo não corresponde à versão. Atualize a lista antes de registrar a assinatura.",
            ));
        }
        let signer = input.signer.trim();
        let evidence = input.evidence.trim();
        if !(2..=160).contains(&signer.chars().count())
            || !(10..=500).contains(&evidence.chars().count())
            || signer.chars().chain(evidence.chars()).any(char::is_control)
        {
            return Err(invalid(
                "Informe signatário e referência verificável da evidência da assinatura.",
            ));
        }
        let signed = input
            .signed_at
            .parse::<canteiro_domain::UtcInstant>()
            .map_err(|_| {
                invalid("Informe data e horário de assinatura com fuso e até seis casas decimais.")
            })?
            .as_datetime();
        if signed > tx.now().await? {
            return Err(invalid("A assinatura não pode ter data futura."));
        }
        let existing = tx
            .document_signatures(access.tenant, document, input.version)
            .await?;
        if existing.iter().any(|previous| {
            previous.signer == signer
                && previous.evidence == evidence
                && previous.signed_at != signed
        }) {
            return Err(AuthError::new(
                AuthErrorKind::Conflict,
                "signature_conflict",
                "Esta evidência já foi registrada com outra data. Confira o registro anterior; uma retificação precisa de evidência própria.",
            ));
        }
        let evidence_key = Sha256::digest(
            serde_json::to_vec(&(signer, evidence)).map_err(|_| AuthError::unavailable())?,
        )
        .to_vec();
        if tx
            .insert_document_signature(
                access.tenant,
                document,
                input.version,
                &row.sha256,
                &evidence_key,
                signer,
                signed,
                evidence,
                access.actor.id,
            )
            .await?
        {
            tx.document_history(
                access.tenant,
                document,
                input.version,
                None,
                &row.sha256,
                access.actor.id,
                "signature_recorded",
            )
            .await?;
        }
        self.document_revalidate(token, &mut tx, &access).await?;
        tx.commit().await?;
        Ok(())
    }
}

pub struct DocumentDownload {
    pub content: Vec<u8>,
    pub content_type: String,
    pub file_name: String,
    pub sha256: String,
    pub preview: bool,
}
