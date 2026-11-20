//! Cenários PostgreSQL. O antivírus tem ensaio próprio de protocolo; conteúdo
//! previamente verificado é semeado explicitamente para testar versões e acesso.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use canteiro_contracts::documents::*;
use sha2::{Digest, Sha256};

async fn workspace(f: &Fixture, owner: &Person, label: &str) -> (DocumentScope, Uuid) {
    let (tenant, _) = f.create(owner, label).await;
    let selected = f.select(owner, &tenant).await;
    let work = Uuid::new_v4();
    let grant = Uuid::new_v4();
    let tenant_id = Uuid::parse_str(&tenant).unwrap();
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,created_by) VALUES($1,$2,'work','Obra de teste documental',$3)")
        .bind(tenant_id).bind(work).bind(owner.id).execute(&f.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'manager',ARRAY['view','manage_portfolio'],0,clock_timestamp(),$4)")
        .bind(tenant_id).bind(grant).bind(work).bind(owner.id).execute(&f.admin).await.unwrap();
    (
        DocumentScope {
            organization_id: tenant,
            context_version: selected.context_version,
            scope_id: work.to_string(),
        },
        grant,
    )
}

fn upload(scope: &DocumentScope, key: Uuid, text: &str) -> UploadDocument {
    UploadDocument {
        scope: scope.clone(),
        operation_key: key.to_string(),
        document_id: None,
        expected_version: 0,
        file_name: "registro.txt".into(),
        description: "Registro fictício de recebimento".into(),
        category: "technical".into(),
        content_base64: STANDARD.encode(text),
    }
}

async fn inspected_fixture(
    f: &Fixture,
    owner: &Person,
    scope: &DocumentScope,
    result: &DocumentUploadResult,
) -> (Uuid, i64) {
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    let mut tx = f.service.store.begin().await.unwrap();
    tx.org_set_actor(owner.id).await.unwrap();
    tx.org_set_tenant(tenant).await.unwrap();
    let row = tx
        .document_upload(tenant, Uuid::parse_str(&result.upload_id).unwrap())
        .await
        .unwrap()
        .unwrap();
    let document = row.target_document.unwrap_or_else(Uuid::new_v4);
    let version = row.expected_version + 1;
    tx.accept_document_upload(tenant, &row, document, version, owner.id)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    (document, version)
}

fn ticket(url: &str) -> &str {
    url.split("/download/")
        .nth(1)
        .unwrap()
        .split('?')
        .next()
        .unwrap()
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn documents_completed_retry_revalidates_after_waiting_for_upload_lock() {
    let f = Fixture::new(6).await;
    let owner = f.person("document-retry-lock", true).await;
    let (scope, grant) = workspace(&f, &owner, "Documentos com prazo").await;
    let command = upload(
        &scope,
        Uuid::new_v4(),
        "Revisão preservada antes da espera.",
    );
    let result = f
        .service
        .document_upload_new(&owner.token, command)
        .await
        .unwrap();
    inspected_fixture(&f, &owner, &scope, &result).await;
    let upload_id = Uuid::parse_str(&result.upload_id).unwrap();
    let mut blocker = f.admin.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM canteiro.document_uploads WHERE id=$1 FOR UPDATE")
        .bind(upload_id)
        .execute(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("UPDATE canteiro.access_grants SET valid_until=clock_timestamp()+interval '650 milliseconds' WHERE id=$1").bind(grant).execute(&f.admin).await.unwrap();
    let service = f.service.clone();
    let token = owner.token.clone();
    let retry = tokio::spawn(async move {
        service
            .document_upload_retry(
                &token,
                RetryDocumentUpload {
                    scope,
                    upload_id: upload_id.to_string(),
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_millis(500), async {
        loop {
            let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%document_uploads%')").bind(pid).fetch_one(&f.admin).await.unwrap();
            if blocked { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("o replay deve aguardar o bloqueio real do upload");
    sqlx::query("SELECT pg_sleep(GREATEST(0,extract(epoch FROM valid_until-clock_timestamp())+0.02)) FROM canteiro.access_grants WHERE id=$1").bind(grant).execute(&f.admin).await.unwrap();
    blocker.commit().await.unwrap();
    error(retry.await.unwrap(), "document_unavailable");
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL isolado e scanner externo não configurado"]
async fn documents_quarantine_is_not_an_attachment_and_replay_revalidates_access() {
    assert!(std::env::var_os("CANTEIRO_DOCUMENT_SCANNER").is_none());
    let f = Fixture::new(8).await;
    let owner = f.person("document-owner", true).await;
    let (scope, grant) = workspace(&f, &owner, "Documentos Aurora").await;
    let key = Uuid::new_v4();
    let command = upload(&scope, key, "Registro fictício original.");
    let (one, two) = tokio::join!(
        f.service.document_upload_new(&owner.token, command.clone()),
        f.service.document_upload_new(&owner.token, command.clone())
    );
    let one = one.unwrap();
    let two = two.unwrap();
    assert_eq!(one.upload_id, two.upload_id);
    assert_eq!(one.status, UploadStatus::Quarantine);
    assert!(one.document_id.is_none());
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    let counts:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM canteiro.document_uploads WHERE tenant_id=$1),(SELECT count(*) FROM canteiro.document_versions WHERE tenant_id=$1)").bind(tenant).fetch_one(&f.admin).await.unwrap();
    assert_eq!(counts, (1, 0));
    let listing = f
        .service
        .documents_list(&owner.token, &scope, None)
        .await
        .unwrap();
    assert!(listing.versions.is_empty());
    assert_eq!(listing.uploads.len(), 1);
    error(
        f.service
            .document_upload_new(&owner.token, upload(&scope, key, "Conteúdo diferente."))
            .await,
        "idempotency_conflict",
    );
    let mut false_extension = upload(&scope, Uuid::new_v4(), "Não é um PDF.");
    false_extension.file_name = "arquivo.pdf".into();
    error(
        f.service
            .document_upload_new(&owner.token, false_extension)
            .await,
        "invalid_input",
    );
    let mut partial = upload(&scope, Uuid::new_v4(), "dado");
    partial.content_base64 = "%%%".into();
    error(
        f.service.document_upload_new(&owner.token, partial).await,
        "invalid_input",
    );
    error(
        f.service
            .document_download_link(
                &owner.token,
                DocumentLinkRequest {
                    scope: scope.clone(),
                    document_id: one.upload_id.clone(),
                    version: 1,
                    preview: false,
                },
            )
            .await,
        "document_unavailable",
    );
    sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND id=$2").bind(tenant).bind(grant).execute(&f.admin).await.unwrap();
    error(
        f.service.document_upload_new(&owner.token, command).await,
        "document_unavailable",
    );
    error(
        f.service
            .document_upload_retry(
                &owner.token,
                RetryDocumentUpload {
                    scope: scope.clone(),
                    upload_id: one.upload_id,
                },
            )
            .await,
        "document_unavailable",
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL isolado e scanner externo não configurado"]
async fn documents_versions_keep_original_bytes_and_signatures_do_not_move() {
    assert!(std::env::var_os("CANTEIRO_DOCUMENT_SCANNER").is_none());
    let f = Fixture::new(8).await;
    let owner = f.person("version-owner", true).await;
    let (scope, _) = workspace(&f, &owner, "Documentos Horizonte").await;
    let command = upload(&scope, Uuid::new_v4(), "Conteúdo da primeira revisão.");
    let result = f
        .service
        .document_upload_new(&owner.token, command.clone())
        .await
        .unwrap();
    let (document, version) = inspected_fixture(&f, &owner, &scope, &result).await;
    assert_eq!(version, 1);
    let digest = format!(
        "{:x}",
        Sha256::digest("Conteúdo da primeira revisão.".as_bytes())
    );
    let signature = RecordDocumentSignature {
        scope: scope.clone(),
        document_id: document.to_string(),
        version: 1,
        sha256: digest.clone(),
        signer: "Profissional de teste".into(),
        signed_at: "2026-01-01T12:00:00.123456Z".into(),
        evidence: "Protocolo fictício da assinatura 001".into(),
    };
    let mut too_precise = signature.clone();
    too_precise.signed_at = "2026-01-01T12:00:00.1234567Z".into();
    assert!(
        f.service
            .document_record_signature(&owner.token, too_precise)
            .await
            .is_err()
    );
    f.service
        .document_record_signature(&owner.token, signature.clone())
        .await
        .unwrap();
    f.service
        .document_record_signature(&owner.token, signature)
        .await
        .unwrap();
    let mut new = upload(&scope, Uuid::new_v4(), "Conteúdo da segunda revisão.");
    new.document_id = Some(document.to_string());
    new.expected_version = 1;
    let result = f
        .service
        .document_upload_new(&owner.token, new.clone())
        .await
        .unwrap();
    let (_, version) = inspected_fixture(&f, &owner, &scope, &result).await;
    assert_eq!(version, 2);
    let list = f
        .service
        .documents_list(&owner.token, &scope, None)
        .await
        .unwrap();
    assert_eq!(list.versions.len(), 2);
    let original = list.versions.iter().find(|v| v.version == 1).unwrap();
    let current = list.versions.iter().find(|v| v.version == 2).unwrap();
    assert!(!original.active);
    assert!(current.active);
    assert_eq!(original.signatures.len(), 1);
    assert_eq!(
        original.signatures[0].signed_at,
        "2026-01-01T12:00:00.123456+00:00"
    );
    assert!(current.signatures.is_empty());
    assert_eq!(original.sha256, digest);
    let link = f
        .service
        .document_download_link(
            &owner.token,
            DocumentLinkRequest {
                scope: scope.clone(),
                document_id: document.to_string(),
                version: 1,
                preview: false,
            },
        )
        .await
        .unwrap();
    let bytes = f
        .service
        .document_download(&owner.token, &scope, ticket(&link.url))
        .await
        .unwrap();
    assert_eq!(bytes.content, "Conteúdo da primeira revisão.".as_bytes());
    assert_eq!(bytes.sha256, digest);
    let replay = f
        .service
        .document_upload_new(&owner.token, command)
        .await
        .unwrap();
    assert_eq!(replay.version, Some(1));
    assert_eq!(replay.document_id, Some(document.to_string()));
    new.operation_key = Uuid::new_v4().to_string();
    error(
        f.service.document_upload_new(&owner.token, new).await,
        "version_conflict",
    );
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    for statement in [
        "UPDATE canteiro.document_versions SET file_name='alterado.txt'",
        "DELETE FROM canteiro.document_versions",
        "TRUNCATE canteiro.document_versions",
        "UPDATE canteiro.document_signatures SET signer='outro'",
        "DELETE FROM canteiro.document_history",
    ] {
        let mut tx = f.admin.begin().await.unwrap();
        sqlx::query("SELECT set_config('role',$1,true),set_config('canteiro.tenant_id',$2,true)")
            .bind(&f.runtime)
            .bind(tenant.to_string())
            .execute(&mut *tx)
            .await
            .unwrap();
        assert!(
            sqlx::raw_sql(statement).execute(&mut *tx).await.is_err(),
            "{statement}"
        );
        tx.rollback().await.unwrap();
    }
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL isolado e scanner externo não configurado"]
async fn documents_ticket_is_bound_to_session_tenant_scope_expiry_and_current_membership() {
    assert!(std::env::var_os("CANTEIRO_DOCUMENT_SCANNER").is_none());
    let f = Fixture::new(8).await;
    let owner = f.person("link-owner", true).await;
    let teammate = f.person("teammate", false).await;
    let (scope, _) = workspace(&f, &owner, "Empresa do anexo").await;
    let pending = f
        .service
        .document_upload_new(
            &owner.token,
            upload(&scope, Uuid::new_v4(), "Arquivo confidencial fictício."),
        )
        .await
        .unwrap();
    let (document, _) = inspected_fixture(&f, &owner, &scope, &pending).await;
    let link = f
        .service
        .document_download_link(
            &owner.token,
            DocumentLinkRequest {
                scope: scope.clone(),
                document_id: document.to_string(),
                version: 1,
                preview: true,
            },
        )
        .await
        .unwrap();
    f.add(&owner, &teammate, OrganizationRole::Member).await;
    let selected = f.select(&teammate, &scope.organization_id).await;
    let other_scope = DocumentScope {
        context_version: selected.context_version,
        ..scope.clone()
    };
    error(
        f.service
            .document_download(&teammate.token, &other_scope, ticket(&link.url))
            .await,
        "document_unavailable",
    );
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'viewer',ARRAY['view'],0,clock_timestamp(),$5)")
        .bind(tenant).bind(Uuid::new_v4()).bind(Uuid::parse_str(&scope.scope_id).unwrap()).bind(teammate.id).bind(owner.id).execute(&f.admin).await.unwrap();
    error(
        f.service
            .document_download(&teammate.token, &other_scope, ticket(&link.url))
            .await,
        "document_unavailable",
    );
    let stranger = f.person("other-tenant", true).await;
    let (other_company, _) = workspace(&f, &stranger, "Outra empresa documental").await;
    error(
        f.service
            .document_download(&stranger.token, &other_company, ticket(&link.url))
            .await,
        "document_unavailable",
    );
    assert!(
        f.service
            .documents_list(&stranger.token, &other_company, None)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    sqlx::query("UPDATE canteiro.document_download_tickets SET expires_at=clock_timestamp()-interval '1 second' WHERE tenant_id=$1").bind(tenant).execute(&f.admin).await.unwrap();
    error(
        f.service
            .document_download(&owner.token, &scope, ticket(&link.url))
            .await,
        "document_unavailable",
    );
    let own_link = f
        .service
        .document_download_link(
            &teammate.token,
            DocumentLinkRequest {
                scope: other_scope.clone(),
                document_id: document.to_string(),
                version: 1,
                preview: false,
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE canteiro.organization_members SET valid_until=clock_timestamp()-interval '1 second' WHERE tenant_id=$1 AND user_id=$2").bind(tenant).bind(teammate.id).execute(&f.admin).await.unwrap();
    assert!(
        f.service
            .document_download(&teammate.token, &other_scope, ticket(&own_link.url))
            .await
            .is_err()
    );
    f.close().await;
}
