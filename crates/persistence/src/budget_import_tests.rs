//! Issue #19: reimportação integral de 10 mil serviços em PostgreSQL real.
//! O teste exercita os leitores CSV e XLSX versionados e a gravação de revisões.
//! Não considera uma prévia ou uma gravação parcial como importação concluída.
use super::*;
use crate::{auth::AuthStore, budget_imports::NewBudgetImport, budgets::BudgetWrite};
use canteiro_domain::{
    ActorId, EntityId,
    budget_csv::{NumberLocale, export_csv, preview_import},
    budget_xlsx,
    budgets::{BudgetLine, BudgetRevision, BudgetState, BudgetVariation},
    units::MeasureUnit,
};

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn ten_thousand_csv_services_reimport_atomically_with_version_conflict() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let scope = Uuid::new_v4();
    let work = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO canteiro_auth.users(id,email,password_hash,verified)
         VALUES($1,$2,'fixture-only',true)",
    )
    .bind(actor)
    .bind(format!("{actor}@example.invalid"))
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash)
         VALUES($1,'Empresa de teste',$2,$3)",
    )
    .bind(tenant)
    .bind(actor)
    .bind([17_u8; 32].as_slice())
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organization_members(tenant_id,user_id,role)
         VALUES($1,$2,'administrator')",
    )
    .bind(tenant)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by)
         VALUES($1,$2,'work','Escopo de teste',NULL,$3)",
    )
    .bind(tenant)
    .bind(scope)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();

    let mut original = BudgetRevision {
        budget_id: scope.to_string(),
        revision: 1,
        status: BudgetState::Draft,
        responsible_id: ActorId::from_uuid(actor).unwrap(),
        variations: vec![BudgetVariation {
            id: "v1".into(),
            work_id: EntityId::from_uuid(work).unwrap(),
            scope: "Serviços de teste".into(),
        }],
        lines: (0..10_000)
            .map(|index| BudgetLine {
                id: format!("servico-{index:05}"),
                variation_id: "v1".into(),
                group: "Obra".into(),
                stage: "Execução".into(),
                substage: "Serviços".into(),
                code: format!("T-{index:05}"),
                description: "Serviço de verificação".into(),
                unit: MeasureUnit::SquareMetre,
                quantity: "1.00".parse().unwrap(),
                price: "2.50".parse().unwrap(),
                catalog_id: None,
                catalog_edition: None,
            })
            .collect(),
        schedule: None,
    };
    original.validate().unwrap();
    let csv = export_csv(&original, NumberLocale::PtBr).unwrap();
    let parsed = preview_import(&csv, NumberLocale::PtBr).unwrap();
    assert_eq!(parsed, original);
    assert_eq!(parsed.lines.len(), 10_000);
    let fingerprint = [17_u8; 32];

    // Uma sessão efetiva, e não apenas o ator, é parte da chave do lote.
    let session = Uuid::new_v4();
    let mut identity = AuthStore::new(database.clone()).begin().await.unwrap();
    identity
        .insert_session(
            session,
            &[42_u8; 32],
            Some((actor, 1)),
            "authenticated",
            true,
            3600,
        )
        .await
        .unwrap();
    identity.commit().await.unwrap();

    let mut first = AuthStore::new(database.clone()).begin().await.unwrap();
    first.org_set_actor(actor).await.unwrap();
    first.org_set_tenant(tenant).await.unwrap();
    let first_json = serde_json::to_string(&parsed).unwrap();
    // A primeira importação também recebe prévia durável, sem revisão de origem.
    let first_batch_id = Uuid::new_v4();
    let first_hash = [98_u8; 32];
    // Simula uma prévia legítima de duas horas atrás, já vencida.
    let expired_batch = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO canteiro.budget_import_batches
         (tenant_id,id,scope_id,actor_id,session_id,format,locale,sha256,preview,
          included_count,changed_count,removed_count,created_at,expires_at)
         VALUES($1,$2,$3,$4,$5,'CANTEIRO-ORCAMENTO-CSV-1','pt-BR',$6,$7::jsonb,
                10000,0,0,clock_timestamp()-interval '3 hours',
                clock_timestamp()-interval '1 hour')",
    )
    .bind(tenant)
    .bind(expired_batch)
    .bind(scope)
    .bind(actor)
    .bind(session)
    .bind(first_hash.as_slice())
    .bind(&first_json)
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert!(
        first
            .budget_import_ready_snapshot(tenant, scope, actor, session, expired_batch)
            .await
            .unwrap()
            .is_none()
    );
    let first_preview = NewBudgetImport {
        tenant,
        id: first_batch_id,
        scope,
        actor,
        session,
        base_version: None,
        format: canteiro_domain::budget_csv::FORMAT_VERSION,
        locale: "pt-BR",
        sha256: &first_hash,
        preview: &first_json,
        included: 10_000,
        changed: 0,
        removed: 0,
    };
    // Prévia incorreta não deve ocupar um lote nem mesmo se o JSON for bem formado.
    // A camada de persistência confere escopo, revisão e estado independentemente da API.
    let bad_hash = [77_u8; 32];
    let mut invalid_revision = parsed.clone();
    invalid_revision.revision = 2;
    let invalid_revision_json = serde_json::to_string(&invalid_revision).unwrap();
    assert!(
        first
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                sha256: &bad_hash,
                preview: &invalid_revision_json,
                ..first_preview
            })
            .await
            .unwrap()
            .is_none()
    );
    let mut invalid_scope = parsed.clone();
    invalid_scope.budget_id = Uuid::new_v4().to_string();
    let invalid_scope_json = serde_json::to_string(&invalid_scope).unwrap();
    assert!(
        first
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                sha256: &[78_u8; 32],
                preview: &invalid_scope_json,
                ..first_preview
            })
            .await
            .unwrap()
            .is_none()
    );
    let mut invalid_state = parsed.clone();
    invalid_state.status = BudgetState::Approved;
    let invalid_state_json = serde_json::to_string(&invalid_state).unwrap();
    assert!(
        first
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                sha256: &[79_u8; 32],
                preview: &invalid_state_json,
                ..first_preview
            })
            .await
            .unwrap()
            .is_none()
    );
    // Uma contagem falsa não pode gerar uma prévia durável nem ocupar a chave.
    assert!(
        first
            .budget_import_stage(&NewBudgetImport {
                included: 9_999,
                ..first_preview
            })
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        first.budget_import_stage(&first_preview).await.unwrap(),
        Some(first_batch_id)
    );
    assert_eq!(
        first
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                ..first_preview
            })
            .await
            .unwrap(),
        Some(first_batch_id)
    );
    assert!(
        !first
            .budget_import_mark_applied(
                tenant,
                scope,
                actor,
                session,
                first_batch_id,
                first_batch_id
            )
            .await
            .unwrap()
    );
    assert_eq!(
        first
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: None,
                actor,
                operation_key: first_batch_id,
                fingerprint: &fingerprint,
                reason: "Primeira importação integral",
                payload: &first_json,
                state: "draft",
            })
            .await
            .unwrap()
            .unwrap()
            .version,
        1
    );
    assert!(
        first
            .budget_import_mark_applied(
                tenant,
                scope,
                actor,
                session,
                first_batch_id,
                first_batch_id
            )
            .await
            .unwrap()
    );
    first.commit().await.unwrap();
    let expired_status: String = sqlx::query_scalar(
        "SELECT status FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(expired_batch)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(expired_status, "cancelled");
    let first_status: String = sqlx::query_scalar(
        "SELECT status FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(first_batch_id)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(first_status, "applied");

    original.revision = 2;
    original.lines[9_999].quantity = "3.00".parse().unwrap();
    let updated_csv = export_csv(&original, NumberLocale::PtBr).unwrap();
    let updated = preview_import(&updated_csv, NumberLocale::PtBr).unwrap();
    assert_eq!(updated.lines.len(), 10_000);
    assert_eq!(
        updated.lines[9_999].quantity,
        original.lines[9_999].quantity
    );
    // Uma célula maliciosa no último serviço invalida o lote inteiro antes da gravação.
    let malformed = updated_csv.replacen("T-09999", "=1+1", 1);
    let diagnostic = preview_import(&malformed, NumberLocale::PtBr).unwrap_err();
    assert_eq!((diagnostic.row, diagnostic.column), (10_004, 13));
    let xlsx = budget_xlsx::export_from_csv(&updated_csv, NumberLocale::PtBr).unwrap();
    let (xlsx_snapshot, locale) = budget_xlsx::preview_import_with_locale(&xlsx).unwrap();
    assert_eq!(locale, NumberLocale::PtBr);
    assert_eq!(xlsx_snapshot, updated);
    let updated_json = serde_json::to_string(&xlsx_snapshot).unwrap();

    // Identidade e arquivo da prévia persistem antes da confirmação.
    let batch_id = Uuid::new_v4();
    let file_hash = [99_u8; 32];
    let mut stage = AuthStore::new(database.clone()).begin().await.unwrap();
    stage.org_set_actor(actor).await.unwrap();
    stage.org_set_tenant(tenant).await.unwrap();
    let candidate = NewBudgetImport {
        tenant,
        id: batch_id,
        scope,
        actor,
        session,
        base_version: Some(1),
        format: budget_xlsx::XLSX_VERSION,
        locale: "pt-BR",
        sha256: &file_hash,
        preview: &updated_json,
        included: 0,
        changed: 1,
        removed: 0,
    };
    // Mesmo com relatório correto e snapshot estruturalmente válido, uma
    // importação direta não pode excluir serviços ou alterar o escopo histórico.
    let mut removed = updated.clone();
    removed.lines.remove(0);
    let mut moved = updated.clone();
    moved.variations[0].scope = "Escopo alterado sem comando explícito".into();
    for prohibited in [removed, moved] {
        prohibited.summarize().unwrap();
        let difference = canteiro_domain::budget_csv::compare(&parsed, &prohibited);
        let payload = serde_json::to_string(&prohibited).unwrap();
        assert!(
            stage
                .budget_import_stage(&NewBudgetImport {
                    id: Uuid::new_v4(),
                    preview: &payload,
                    included: i32::try_from(difference.included).unwrap(),
                    changed: i32::try_from(difference.changed).unwrap(),
                    removed: i32::try_from(difference.removed).unwrap(),
                    ..candidate
                })
                .await
                .unwrap()
                .is_none()
        );
    }
    // A diferença é recalculada sobre a revisão real, não sobre o relatório enviado.
    for (included, changed, removed) in [(1, 0, 0), (0, 0, 0), (0, 1, 1)] {
        assert!(
            stage
                .budget_import_stage(&NewBudgetImport {
                    included,
                    changed,
                    removed,
                    ..candidate
                })
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        stage.budget_import_stage(&candidate).await.unwrap(),
        Some(batch_id)
    );
    // O relatório consulta exatamente o snapshot preparado e não cria outro lote.
    let (base, payload) = stage
        .budget_import_ready_snapshot(tenant, scope, actor, session, batch_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(base, Some(1));
    assert_eq!(
        serde_json::from_str::<BudgetRevision>(&payload).unwrap(),
        updated
    );
    for (wrong_scope, wrong_actor, wrong_session, wrong_batch) in [
        (Uuid::new_v4(), actor, session, batch_id),
        (scope, Uuid::new_v4(), session, batch_id),
        (scope, actor, Uuid::new_v4(), batch_id),
        (scope, actor, session, Uuid::new_v4()),
        (scope, actor, session, first_batch_id),
    ] {
        assert!(
            stage
                .budget_import_ready_snapshot(
                    tenant,
                    wrong_scope,
                    wrong_actor,
                    wrong_session,
                    wrong_batch,
                )
                .await
                .unwrap()
                .is_none()
        );
    }
    // Retomar a mesma prévia não gera outra operação.
    assert_eq!(
        stage
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                ..candidate
            })
            .await
            .unwrap(),
        Some(batch_id)
    );
    // O mesmo hash não pode ser reinterpretado como outro formato.
    assert!(
        stage
            .budget_import_stage(&NewBudgetImport {
                id: Uuid::new_v4(),
                format: canteiro_domain::budget_csv::FORMAT_VERSION,
                ..candidate
            })
            .await
            .unwrap()
            .is_none()
    );
    // Mesmo hash/versão, mas conteúdo diferente: não reaproveitar silenciosamente.
    assert!(
        stage
            .budget_import_stage(&NewBudgetImport {
                tenant,
                id: Uuid::new_v4(),
                scope,
                actor,
                session,
                base_version: Some(1),
                format: budget_xlsx::XLSX_VERSION,
                locale: "pt-BR",
                sha256: &file_hash,
                preview: "{\"outro\":true}",
                included: 0,
                changed: 1,
                removed: 0,
            })
            .await
            .unwrap()
            .is_none()
    );
    // Não existe revisão 2 neste momento: marcar como aplicado deve falhar.
    assert!(
        !stage
            .budget_import_mark_applied(tenant, scope, actor, session, batch_id, Uuid::new_v4())
            .await
            .unwrap()
    );
    stage.commit().await.unwrap();

    // A identidade de execução não pode emitir um recibo fictício.
    let mut direct = AuthStore::new(database.clone()).begin().await.unwrap();
    direct.org_set_actor(actor).await.unwrap();
    direct.org_set_tenant(tenant).await.unwrap();
    let rejected = sqlx::query(
        "UPDATE canteiro.budget_import_batches
         SET status='applied',applied_at=clock_timestamp()
         WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .execute(&mut *direct.tx)
    .await
    .unwrap_err();
    assert_eq!(
        rejected.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    drop(direct);

    let mut direct = AuthStore::new(database.clone()).begin().await.unwrap();
    direct.org_set_actor(actor).await.unwrap();
    direct.org_set_tenant(tenant).await.unwrap();
    let rejected = sqlx::query(
        "INSERT INTO canteiro.budget_import_batches
         (tenant_id,id,scope_id,actor_id,session_id,base_version,format,locale,
          sha256,preview,included_count,changed_count,removed_count,status,applied_at)
         SELECT tenant_id,$3,scope_id,actor_id,session_id,base_version,format,locale,
                $4,preview,included_count,changed_count,removed_count,'applied',clock_timestamp()
         FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .bind(Uuid::new_v4())
    .bind([77_u8; 32].as_slice())
    .execute(&mut *direct.tx)
    .await
    .unwrap_err();
    assert_eq!(
        rejected.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    drop(direct);

    // Um recibo de lote não pode acompanhar outra versão dos mesmos 10 mil serviços.
    // A falha da conferência deve reverter também o incremento da revisão.
    let mut tampered = updated.clone();
    tampered.lines[0].quantity = "9.00".parse().unwrap();
    let tampered_json = serde_json::to_string(&tampered).unwrap();
    let mut mismatched = AuthStore::new(database.clone()).begin().await.unwrap();
    mismatched.org_set_actor(actor).await.unwrap();
    mismatched.org_set_tenant(tenant).await.unwrap();
    let forged_operation = batch_id;
    assert_eq!(
        mismatched
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: Some(1),
                actor,
                operation_key: forged_operation,
                fingerprint: &fingerprint,
                reason: "Prévia adulterada após conferência",
                payload: &tampered_json,
                state: "draft",
            })
            .await
            .unwrap()
            .unwrap()
            .version,
        2
    );
    assert!(
        !mismatched
            .budget_import_mark_applied(tenant, scope, actor, session, batch_id, forged_operation)
            .await
            .unwrap()
    );
    let rejected = sqlx::query(
        "UPDATE canteiro.budget_import_batches
         SET status='applied',applied_at=clock_timestamp()
         WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .execute(&mut *mismatched.tx)
    .await
    .unwrap_err();
    assert_eq!(
        rejected.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    drop(mismatched);
    // A consulta fora da transação demonstra que o lote e a revisão não foram aplicados.
    let persisted_version: i64 = sqlx::query_scalar(
        "SELECT current_version FROM canteiro.budget_records WHERE tenant_id=$1 AND scope_id=$2",
    )
    .bind(tenant)
    .bind(scope)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(persisted_version, 1);
    let revision_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.budget_versions WHERE tenant_id=$1 AND scope_id=$2",
    )
    .bind(tenant)
    .bind(scope)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(revision_count, 1);
    let pending_batch_status: String = sqlx::query_scalar(
        "SELECT status FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(pending_batch_status, "ready");

    // Duas confirmações reais leem a mesma prévia antes de disputar o CAS.
    // Só a transação que gravar a revisão pode consumir o lote.
    let barrier = tokio::sync::Barrier::new(2);
    let confirm = |operation| {
        let database = database.clone();
        let payload = &updated_json;
        let barrier = &barrier;
        async move {
            let mut tx = AuthStore::new(database).begin().await.unwrap();
            tx.org_set_actor(actor).await.unwrap();
            tx.org_set_tenant(tenant).await.unwrap();
            let (base, _) = tx
                .budget_import_ready_snapshot(tenant, scope, actor, session, batch_id)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(base, Some(1));
            barrier.wait().await;
            let written = tx
                .budget_write(&BudgetWrite {
                    tenant,
                    scope,
                    expected: base,
                    actor,
                    operation_key: operation,
                    fingerprint: &fingerprint,
                    reason: "Reimportação concorrente dos dez mil serviços",
                    payload,
                    state: "draft",
                })
                .await
                .unwrap();
            if let Some(version) = written {
                assert_eq!(version.version, 2);
                assert!(
                    tx.budget_import_mark_applied(
                        tenant, scope, actor, session, batch_id, operation
                    )
                    .await
                    .unwrap()
                );
                assert!(
                    !tx.budget_import_mark_applied(
                        tenant, scope, actor, session, batch_id, operation
                    )
                    .await
                    .unwrap()
                );
                tx.commit().await.unwrap();
                true
            } else {
                assert!(
                    !tx.budget_import_mark_applied(
                        tenant, scope, actor, session, batch_id, operation
                    )
                    .await
                    .unwrap()
                );
                tx.commit().await.unwrap();
                false
            }
        }
    };
    let (first_result, second_result) = tokio::join!(confirm(batch_id), confirm(batch_id));
    assert_ne!(first_result, second_result);
    let (status, applied_at): (String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "SELECT status,applied_at FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(status, "applied");
    assert!(applied_at.is_some());
    let format: String = sqlx::query_scalar(
        "SELECT format FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2",
    )
    .bind(tenant)
    .bind(batch_id)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(format, budget_xlsx::XLSX_VERSION);
    // Reescrever data de criação ou estado de lote aplicado deve ser recusado.
    assert!(
        sqlx::query(
            "UPDATE canteiro.budget_import_batches
         SET created_at=created_at+interval '1 second' WHERE tenant_id=$1 AND id=$2"
        )
        .bind(tenant)
        .bind(batch_id)
        .execute(&fixture.admin)
        .await
        .is_err()
    );

    // CAS obsoleto não cria revisão nem sobrescreve a reimportação.
    let mut stale = AuthStore::new(database.clone()).begin().await.unwrap();
    stale.org_set_actor(actor).await.unwrap();
    stale.org_set_tenant(tenant).await.unwrap();
    assert!(
        stale
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: Some(1),
                actor,
                operation_key: Uuid::new_v4(),
                fingerprint: &fingerprint,
                reason: "Conflito de versão esperado",
                payload: &updated_json,
                state: "draft",
            })
            .await
            .unwrap()
            .is_none()
    );
    stale.commit().await.unwrap();

    // Erro após a tentativa de avançar o ponteiro da revisão reverte tudo.
    let mut rejected = AuthStore::new(database.clone()).begin().await.unwrap();
    rejected.org_set_actor(actor).await.unwrap();
    rejected.org_set_tenant(tenant).await.unwrap();
    assert!(
        rejected
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: Some(2),
                actor,
                operation_key: Uuid::new_v4(),
                fingerprint: &fingerprint,
                reason: " ",
                payload: &updated_json,
                state: "draft",
            })
            .await
            .is_err()
    );
    drop(rejected);

    let mut inspect = AuthStore::new(database.clone()).begin().await.unwrap();
    inspect.org_set_actor(actor).await.unwrap();
    inspect.org_set_tenant(tenant).await.unwrap();
    let current = inspect
        .budget_current(tenant, scope)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(current.version, 2);
    let stored: BudgetRevision = serde_json::from_str(&current.payload).unwrap();
    assert_eq!(stored, original);
    let historical = inspect.budget_history(tenant, scope).await.unwrap();
    assert_eq!(historical.len(), 2);
    assert_eq!(
        serde_json::from_str::<BudgetRevision>(&historical[1].payload).unwrap(),
        parsed
    );
    inspect.commit().await.unwrap();
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn historical_catalog_reference_is_not_limited_to_recent_headers() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let tenant = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let scope = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO canteiro_auth.users(id,email,password_hash,verified)
         VALUES($1,$2,'fixture-only',true)",
    )
    .bind(actor)
    .bind(format!("{actor}@example.invalid"))
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash)
         VALUES($1,'Empresa de teste',$2,$3)",
    )
    .bind(tenant)
    .bind(actor)
    .bind([17_u8; 32].as_slice())
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organization_members(tenant_id,user_id,role)
         VALUES($1,$2,'administrator')",
    )
    .bind(tenant)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by)
         VALUES($1,$2,'work','Escopo de teste',NULL,$3)",
    )
    .bind(tenant)
    .bind(scope)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO canteiro.cost_catalogs(tenant_id,catalog_id,scope_id,title,created_by)
         VALUES($1,'historico',$2,'Tabela histórica',$3)",
    )
    .bind(tenant)
    .bind(scope)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.cost_catalog_editions
         (tenant_id,catalog_id,edition,source_title,source_organization,license,
          locality,tax_regime,competence,published_on,expected_items,created_by)
         SELECT $1,'historico',n,'Fonte','Instituto','Licença','SC','Regular',
          DATE '2026-11-01',DATE '2026-11-01',1,$2
         FROM generate_series(1,202) AS n",
    )
    .bind(tenant)
    .bind(actor)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.cost_catalog_items
         (tenant_id,catalog_id,edition,code,description,category,unit,price)
         SELECT $1,'historico',n,'A','Serviço','service','m',n
         FROM generate_series(1,201) AS n",
    )
    .bind(tenant)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE canteiro.cost_catalog_editions SET state='published'
         WHERE tenant_id=$1 AND catalog_id='historico' AND edition<=201",
    )
    .bind(tenant)
    .execute(&fixture.admin)
    .await
    .unwrap();

    let mut tx = AuthStore::new(database.clone()).begin().await.unwrap();
    tx.org_set_actor(actor).await.unwrap();
    tx.org_set_tenant(tenant).await.unwrap();
    let recent = tx.catalog_headers(tenant, &[scope]).await.unwrap();
    assert_eq!(recent.len(), 200);
    assert!(!recent.iter().any(|header| header.edition == 1));
    let old = tx
        .catalog_header(tenant, scope, "historico", 1)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old.edition, 1);
    let previous = tx
        .catalog_previous_header(tenant, scope, "historico", 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(previous.edition, 1);
    assert!(
        tx.catalog_header(tenant, scope, "historico", 202)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        tx.catalog_header(tenant, Uuid::new_v4(), "historico", 1)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        tx.catalog_previous_header(tenant, scope, "historico", 1)
            .await
            .unwrap()
            .is_none()
    );
    tx.commit().await.unwrap();

    // A publicação volumosa usa a mesma transação e os mesmos gatilhos.
    // Uma duplicidade no último bloco precisa desfazer também os anteriores.
    let codes: Vec<_> = (0..10_000).map(|n| format!("I{n:05}")).collect();
    let mut items: Vec<_> = codes
        .iter()
        .map(|code| crate::catalogs::NewCatalogLine {
            code,
            description: "Serviço de ensaio",
            category: "service",
            unit: "m",
            price: "1234.56",
        })
        .collect();
    let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
    let header = crate::catalogs::NewCatalogEdition {
        tenant,
        scope,
        catalog_id: "catalogo-volumoso",
        edition: 1,
        source_title: "Tabela volumosa",
        source_organization: "Instituto",
        license: "Uso interno",
        locality: "SC",
        tax_regime: "Regular",
        competence: date,
        published_on: date,
        actor,
    };
    items[9_999].code = &codes[0];
    let mut rejected = AuthStore::new(database.clone()).begin().await.unwrap();
    rejected.org_set_actor(actor).await.unwrap();
    rejected.org_set_tenant(tenant).await.unwrap();
    assert!(rejected.catalog_publish(&header, &items).await.is_err());
    drop(rejected);

    items[9_999].code = &codes[9_999];
    let mut publish = AuthStore::new(database.clone()).begin().await.unwrap();
    publish.org_set_actor(actor).await.unwrap();
    publish.org_set_tenant(tenant).await.unwrap();
    assert!(
        !publish
            .catalog_edition_exists(tenant, header.catalog_id, 1)
            .await
            .unwrap()
    );
    assert!(publish.catalog_publish(&header, &items).await.unwrap());
    publish.commit().await.unwrap();

    // A identidade do catálogo não pode mudar, mesmo ao publicar outra edição.
    let wrong_scope = crate::catalogs::NewCatalogEdition {
        scope: Uuid::new_v4(),
        edition: 2,
        ..header
    };
    let mut denied = AuthStore::new(database.clone()).begin().await.unwrap();
    denied.org_set_actor(actor).await.unwrap();
    denied.org_set_tenant(tenant).await.unwrap();
    assert!(
        !denied
            .catalog_publish(&wrong_scope, &items[..1])
            .await
            .unwrap()
    );
    denied.commit().await.unwrap();

    let mut inspect = AuthStore::new(database.clone()).begin().await.unwrap();
    inspect.org_set_actor(actor).await.unwrap();
    inspect.org_set_tenant(tenant).await.unwrap();
    let lines = inspect
        .catalog_lines(tenant, scope, header.catalog_id, 1)
        .await
        .unwrap();
    assert_eq!(lines.len(), 10_000);
    assert_eq!(lines[0].code, "I00000");
    assert_eq!(lines[9_999].code, "I09999");
    assert_eq!(lines[9_999].price, "1234.560000");
    inspect.commit().await.unwrap();
    database.close().await;
    fixture.cleanup().await;
}
