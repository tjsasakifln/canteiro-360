//! Issue #19: reimportação integral de 10 mil serviços em PostgreSQL real.
//! O teste exercita o mesmo leitor CSV versionado e a gravação de revisões.
//! Não considera uma prévia ou uma gravação parcial como importação concluída.
use super::*;
use crate::{auth::AuthStore, budget_imports::NewBudgetImport, budgets::BudgetWrite};
use canteiro_domain::{
    ActorId, EntityId,
    budget_csv::{NumberLocale, export_csv, preview_import},
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

    let mut first = AuthStore::new(database.clone()).begin().await.unwrap();
    first.org_set_actor(actor).await.unwrap();
    first.org_set_tenant(tenant).await.unwrap();
    let first_json = serde_json::to_string(&parsed).unwrap();
    assert_eq!(
        first
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: None,
                actor,
                operation_key: Uuid::new_v4(),
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
    first.commit().await.unwrap();

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
    let updated_json = serde_json::to_string(&updated).unwrap();

    // Identidade e arquivo da prévia persistem antes da confirmação.
    // Uma sessão efetiva, e não apenas o ator, é parte da chave do lote.
    let session = Uuid::new_v4();
    let mut identity = AuthStore::new(database.clone()).begin().await.unwrap();
    identity
        .insert_session(session, &[42_u8; 32], Some((actor, 1)), "authenticated", true, 3600)
        .await
        .unwrap();
    identity.commit().await.unwrap();

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
        base_version: 1,
        locale: "pt-BR",
        sha256: &file_hash,
        preview: &updated_json,
        included: 0,
        changed: 1,
        removed: 0,
    };
    assert_eq!(
        stage.budget_import_stage(&candidate).await.unwrap(),
        Some(batch_id)
    );
    // Retomar a mesma prévia não gera outra operação.
    assert_eq!(
        stage.budget_import_stage(&NewBudgetImport {
            id: Uuid::new_v4(),
            ..candidate
        }).await.unwrap(),
        Some(batch_id)
    );
    // Mesmo hash/versão, mas conteúdo diferente: não reaproveitar silenciosamente.
    assert!(
        stage.budget_import_stage(&NewBudgetImport {
            tenant, id: Uuid::new_v4(), scope, actor, session, base_version: 1,
            locale: "pt-BR", sha256: &file_hash, preview: "{\"outro\":true}",
            included: 0, changed: 1, removed: 0,
        }).await.unwrap().is_none()
    );
    // Não existe revisão 2 neste momento: marcar como aplicado deve falhar.
    assert!(!stage
        .budget_import_mark_applied(tenant, scope, actor, session, batch_id, Uuid::new_v4())
        .await
        .unwrap());
    stage.commit().await.unwrap();

    let mut second = AuthStore::new(database.clone()).begin().await.unwrap();
    second.org_set_actor(actor).await.unwrap();
    second.org_set_tenant(tenant).await.unwrap();
    let import_operation = Uuid::new_v4();
    assert_eq!(
        second
            .budget_write(&BudgetWrite {
                tenant,
                scope,
                expected: Some(1),
                actor,
                operation_key: import_operation,
                fingerprint: &fingerprint,
                reason: "Reimportação dos dez mil serviços",
                payload: &updated_json,
                state: "draft",
            })
            .await
            .unwrap()
            .unwrap()
            .version,
        2
    );
    assert!(second
        .budget_import_mark_applied(tenant, scope, actor, session, batch_id, import_operation)
        .await
        .unwrap());
    assert!(!second
        .budget_import_mark_applied(tenant, scope, actor, session, batch_id, import_operation)
        .await
        .unwrap());
    second.commit().await.unwrap();
    let (status, applied_at): (String, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as("SELECT status,applied_at FROM canteiro.budget_import_batches WHERE tenant_id=$1 AND id=$2")
            .bind(tenant)
            .bind(batch_id)
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(status, "applied");
    assert!(applied_at.is_some());
    // Reescrever data de criação ou estado de lote aplicado deve ser recusado.
    assert!(sqlx::query(
        "UPDATE canteiro.budget_import_batches
         SET created_at=created_at+interval '1 second' WHERE tenant_id=$1 AND id=$2"
    ).bind(tenant).bind(batch_id).execute(&fixture.admin).await.is_err());

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
