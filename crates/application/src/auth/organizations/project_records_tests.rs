//! Cenários com PostgreSQL isolado, papéis distintos e documentos sintéticos pós-verificação.
use super::*;
use canteiro_contracts::{obligations::*, parties::*, projects::*, reliability::ReliabilityScope};
use canteiro_domain::EntityId;
use sha2::{Digest, Sha256};

async fn setup(f: &Fixture, owner: &Person) -> ReliabilityScope {
    let (tenant, _) = f.create(owner, "Projetos fictícios Aurora").await;
    let c = f.select(owner, &tenant).await;
    ReliabilityScope {
        organization_id: tenant,
        context_version: c.context_version,
    }
}
async fn professional(f: &Fixture, owner: &Person, until: Option<&str>) -> EntityId {
    let input = PartyInput {
        kind: PartyKind::Person,
        name: "Profissional Fictícia".into(),
        tax_id: None,
        roles: vec![PartyRole::Professional],
        contacts: vec![],
        address: String::new(),
        professional: Some(Professional {
            council: "CREA".into(),
            registration: "123456-7".into(),
            uf: "SC".into(),
            valid_from: Some("2020-01-01".parse().unwrap()),
            valid_until: until.map(|d| d.parse().unwrap()),
        }),
        active: true,
    };
    let r = f
        .service
        .parties_execute(
            &owner.token,
            PartyCommand::Save {
                context: f.current_scope(owner).await,
                key: Uuid::new_v4().to_string(),
                id: None,
                expected_version: None,
                input,
                reason: "Cadastro profissional fictício".into(),
            },
        )
        .await
        .unwrap();
    r.ids[0].parse().unwrap()
}
fn spec(person: Option<EntityId>) -> ProjectSpec {
    ProjectSpec {
        title: "Projeto independente".into(),
        discipline: "Estruturas".into(),
        objective: "Definir solução para ampliação".into(),
        scope: "Escopo original do anteprojeto".into(),
        external_reference: String::new(),
        work_id: None,
        stakeholder_ids: vec![],
        deliveries: vec![ProjectDelivery {
            id: EntityId::from_uuid(Uuid::new_v4()).unwrap(),
            title: "Anteprojeto".into(),
            responsible_id: person,
            acceptance_criteria: "Plantas e memorial conferidos conforme programa".into(),
            due_on: Some("2027-01-20".parse().unwrap()),
            milestone: true,
        }],
    }
}
fn change(
    scope: &ReliabilityScope,
    record: Option<&ProjectRecord>,
    change: ProjectChange,
) -> ProjectCommand {
    ProjectCommand {
        context: scope.clone(),
        operation_key: Uuid::new_v4().to_string(),
        project_id: record.map(|r| r.id.clone()),
        expected_version: record.map(|r| r.version),
        reason: "Conferência independente da revisão".into(),
        change,
    }
}
async fn grant(
    f: &Fixture,
    scope: &ReliabilityScope,
    record: &str,
    owner: &Person,
    person: &Person,
    role: &str,
    actions: &[&str],
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,$5,$6,0,clock_timestamp(),$7)")
        .bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(id).bind(Uuid::parse_str(record).unwrap()).bind(person.id).bind(role).bind(actions.iter().map(|a|a.to_string()).collect::<Vec<_>>()).bind(owner.id).execute(&f.admin).await.unwrap();
    id
}
async fn member(
    f: &Fixture,
    owner: &Person,
    other: &Person,
    scope: &ReliabilityScope,
) -> ReliabilityScope {
    f.add(owner, other, OrganizationRole::Member).await;
    let c = f.select(other, &scope.organization_id).await;
    ReliabilityScope {
        organization_id: scope.organization_id.clone(),
        context_version: c.context_version,
    }
}
fn rejected<T: std::fmt::Debug>(r: Result<T, AuthError>, code: &str) {
    let error = r.unwrap_err();
    assert_eq!(error.code(), code, "{error:?}");
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local"]
async fn project_lifecycle_independent_professional_approval_revision_and_acceptance() {
    let f = Fixture::new(8).await;
    let owner = f.person("project-owner", true).await;
    let reviewer = f.person("project-reviewer", false).await;
    let scope = setup(&f, &owner).await;
    let person = professional(&f, &owner, None).await;
    let no_login: bool =
        sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM canteiro_auth.users WHERE id=$1)")
            .bind(person.as_uuid())
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert!(no_login);
    let create = change(&scope, None, ProjectChange::Save { spec: spec(None) });
    let draft = f
        .service
        .projects_execute(&owner.token, create)
        .await
        .unwrap()
        .project;
    assert!(draft.snapshot.spec.work_id.is_none());
    let reviewer_scope = member(&f, &owner, &reviewer, &scope).await;
    grant(
        &f,
        &scope,
        &draft.id,
        &owner,
        &reviewer,
        "approver",
        &["view", "approve_project_scope"],
    )
    .await;
    grant(
        &f,
        &scope,
        &draft.id,
        &owner,
        &reviewer,
        "technical_responsible",
        &["view", "technical_accept"],
    )
    .await;
    rejected(
        f.service
            .projects_execute(
                &reviewer.token,
                change(&reviewer_scope, Some(&draft), ProjectChange::Approve),
            )
            .await,
        "invalid_input",
    );
    let mut completed = draft.snapshot.spec.clone();
    completed.deliveries[0].responsible_id = Some(person);
    let complete = f
        .service
        .projects_execute(
            &owner.token,
            change(
                &scope,
                Some(&draft),
                ProjectChange::Save { spec: completed },
            ),
        )
        .await
        .unwrap()
        .project;
    let approved = f
        .service
        .projects_execute(
            &reviewer.token,
            change(&reviewer_scope, Some(&complete), ProjectChange::Approve),
        )
        .await
        .unwrap()
        .project;
    assert_eq!(approved.snapshot.status, ProjectStatus::Approved);
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_requests")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let accepted = f
        .service
        .projects_execute(
            &reviewer.token,
            change(
                &reviewer_scope,
                Some(&approved),
                ProjectChange::Accept {
                    delivery_id: approved.snapshot.spec.deliveries[0].id.to_string(),
                },
            ),
        )
        .await
        .unwrap()
        .project;
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_requests")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(accepted.snapshot.spec, approved.snapshot.spec);
    let mut new_scope = approved.snapshot.spec.clone();
    new_scope.scope = "Escopo revisado incluindo fundações".into();
    let revised = f
        .service
        .projects_execute(
            &owner.token,
            change(
                &scope,
                Some(&accepted),
                ProjectChange::Save { spec: new_scope },
            ),
        )
        .await
        .unwrap()
        .project;
    assert_eq!(revised.snapshot.status, ProjectStatus::Draft);
    assert!(revised.snapshot.accepted_deliveries.is_empty());
    let history = f
        .service
        .projects_history(&reviewer.token, &reviewer_scope, &draft.id)
        .await
        .unwrap();
    assert_eq!(history.len(), 5);
    assert_eq!(
        history.iter().find(|v| v.version == 3).unwrap().snapshot,
        approved.snapshot
    );
    assert_eq!(
        history
            .iter()
            .find(|v| v.version == 1)
            .unwrap()
            .snapshot
            .spec
            .scope,
        "Escopo original do anteprojeto"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local"]
async fn project_concurrency_repetition_cross_scope_and_revocation() {
    let f = Fixture::new(8).await;
    let owner = f.person("project-race", true).await;
    let stranger = f.person("project-stranger", false).await;
    let scope = setup(&f, &owner).await;
    let create = change(&scope, None, ProjectChange::Save { spec: spec(None) });
    let (a, b) = tokio::join!(
        f.service.projects_execute(&owner.token, create.clone()),
        f.service.projects_execute(&owner.token, create.clone())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.project.id, b.project.id);
    assert!(a.repeated != b.repeated);
    let record = a.project;
    let mut one = record.snapshot.spec.clone();
    one.scope = "Revisão escolhida pela primeira sessão".into();
    let mut two = record.snapshot.spec.clone();
    two.scope = "Revisão concorrente da segunda sessão".into();
    let (a, b) = tokio::join!(
        f.service.projects_execute(
            &owner.token,
            change(&scope, Some(&record), ProjectChange::Save { spec: one })
        ),
        f.service.projects_execute(
            &owner.token,
            change(&scope, Some(&record), ProjectChange::Save { spec: two })
        )
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let error = if a.is_err() {
        a.unwrap_err()
    } else {
        b.unwrap_err()
    };
    assert_eq!(error.code(), "version_conflict");
    let outsider_scope = member(&f, &owner, &stranger, &scope).await;
    assert!(
        f.service
            .projects_context(&stranger.token)
            .await
            .unwrap()
            .projects
            .is_empty()
    );
    rejected(
        f.service
            .projects_history(&stranger.token, &outsider_scope, &record.id)
            .await,
        "record_unavailable",
    );
    let mut changed = create.clone();
    changed.reason = "Outro conteúdo na mesma tentativa".into();
    rejected(
        f.service.projects_execute(&owner.token, changed).await,
        "idempotency_conflict",
    );
    sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND scope_id=$2").bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(Uuid::parse_str(&record.id).unwrap()).execute(&f.admin).await.unwrap();
    rejected(
        f.service.projects_execute(&owner.token, create).await,
        "record_unavailable",
    );
    f.close().await;
}

// Este helper representa a saída já verificada do scanner. Autenticidade de conselho
// não é afirmada. O scanner e a quarentena são testados na issue 10.
async fn evidence(
    f: &Fixture,
    owner: &Person,
    scope: &ReliabilityScope,
    scope_id: &str,
    label: &str,
) -> EntityId {
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    let document = Uuid::new_v4();
    let mut pdf = lopdf::Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let page=pdf.add_object(lopdf::dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()]});
    pdf.objects.insert(
        pages,
        lopdf::dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let catalog = pdf.add_object(lopdf::dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let info=pdf.add_object(lopdf::dictionary!{"Title"=>lopdf::Object::string_literal(format!("Documento sintético {label}"))});
    pdf.trailer.set("Info", info);
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let mut tx = f.admin.begin().await.unwrap();
    sqlx::query("INSERT INTO canteiro.documents(tenant_id,id,scope_id,active_version,created_by) VALUES($1,$2,$3,1,$4)").bind(tenant).bind(document).bind(Uuid::parse_str(scope_id).unwrap()).bind(owner.id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO canteiro.document_versions(tenant_id,document_id,version,sha256,content,content_type,file_name,description,category,created_by,scanner) VALUES($1,$2,1,$3,$4,'application/pdf','registro.pdf','Documento sintético pós-verificação','technical',$5,'clamav-instream')").bind(tenant).bind(document).bind(hash).bind(bytes).bind(owner.id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    EntityId::from_uuid(document).unwrap()
}
fn obligation(person: EntityId, document: EntityId) -> ObligationSpec {
    ObligationSpec {
        kind: ObligationKind::Art,
        activity: TechnicalActivity::Execution,
        discipline: "Estruturas".into(),
        description: "Responsabilidade pela execução estrutural".into(),
        professional_id: Some(person),
        number: "2620261234567".into(),
        issuer: "CREA-SC".into(),
        valid_from: "2026-11-01".parse().unwrap(),
        valid_until: Some("2027-12-31".parse().unwrap()),
        activity_on: "2027-01-20".parse().unwrap(),
        registered_on: Some("2026-11-10".parse().unwrap()),
        rrt_timing: None,
        document_id: Some(document),
        document_version: Some(1),
        exception: None,
        replaces_id: None,
    }
}
fn obligation_command(
    scope: &ReliabilityScope,
    id: &str,
    spec: ObligationSpec,
) -> ObligationCommand {
    ObligationCommand {
        context: scope.clone(),
        operation_key: Uuid::new_v4().to_string(),
        scope_id: id.into(),
        obligation_id: None,
        expected_version: None,
        reason: "Conferência documental fictícia".into(),
        spec,
        confirm: true,
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local"]
async fn obligation_requires_real_same_scope_evidence_and_effective_professional() {
    let f = Fixture::new(8).await;
    let owner = f.person("obligation-owner", true).await;
    let scope = setup(&f, &owner).await;
    let person = professional(&f, &owner, None).await;
    let expired = professional(&f, &owner, Some("2026-12-31")).await;
    let project = f
        .service
        .projects_execute(
            &owner.token,
            change(&scope, None, ProjectChange::Save { spec: spec(None) }),
        )
        .await
        .unwrap()
        .project;
    let second = f
        .service
        .projects_execute(
            &owner.token,
            change(&scope, None, ProjectChange::Save { spec: spec(None) }),
        )
        .await
        .unwrap()
        .project;
    let doc = evidence(&f, &owner, &scope, &project.id, "original").await;
    let other = evidence(&f, &owner, &scope, &second.id, "outra obra").await;
    rejected(
        f.service
            .obligations_execute(
                &owner.token,
                obligation_command(&scope, &project.id, obligation(expired, doc)),
            )
            .await,
        "invalid_input",
    );
    rejected(
        f.service
            .obligations_execute(
                &owner.token,
                obligation_command(&scope, &project.id, obligation(person, other)),
            )
            .await,
        "invalid_input",
    );
    rejected(
        f.service
            .obligations_execute(
                &owner.token,
                obligation_command(
                    &scope,
                    &project.id,
                    obligation(person, EntityId::from_uuid(Uuid::new_v4()).unwrap()),
                ),
            )
            .await,
        "invalid_input",
    );
    let command = obligation_command(&scope, &project.id, obligation(person, doc));
    let original = f
        .service
        .obligations_execute(&owner.token, command.clone())
        .await
        .unwrap()
        .obligation;
    let repeat = f
        .service
        .obligations_execute(&owner.token, command.clone())
        .await
        .unwrap();
    assert!(repeat.repeated);
    assert_eq!(repeat.obligation.id, original.id);
    let mut edit = command.clone();
    edit.operation_key = Uuid::new_v4().to_string();
    edit.obligation_id = Some(original.id.clone());
    edit.expected_version = Some(1);
    edit.spec.number = "2620267654321".into();
    rejected(
        f.service.obligations_execute(&owner.token, edit).await,
        "invalid_input",
    );
    let new_doc = evidence(&f, &owner, &scope, &project.id, "substituto").await;
    let mut replacement = obligation(person, new_doc);
    replacement.number = "2620267654321".into();
    replacement.replaces_id = Some(original.id.parse().unwrap());
    let replaced = f
        .service
        .obligations_execute(
            &owner.token,
            obligation_command(&scope, &project.id, replacement),
        )
        .await
        .unwrap()
        .obligation;
    let history = f
        .service
        .obligations_history(&owner.token, &scope, &original.id)
        .await
        .unwrap();
    assert_eq!(history[0].spec, original.spec);
    assert_eq!(history[0].document_sha256, original.document_sha256);
    assert_eq!(
        history[0].replaced_by.as_deref(),
        Some(replaced.id.as_str())
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local"]
async fn technical_exception_requires_separate_approver_and_unchanged_proposal() {
    let f = Fixture::new(8).await;
    let owner = f.person("exception-owner", true).await;
    let approver = f.person("exception-approver", false).await;
    let scope = setup(&f, &owner).await;
    let person = professional(&f, &owner, None).await;
    let project = f
        .service
        .projects_execute(
            &owner.token,
            change(&scope, None, ProjectChange::Save { spec: spec(None) }),
        )
        .await
        .unwrap()
        .project;
    let doc = evidence(&f, &owner, &scope, &project.id, "fundamento").await;
    let mut command = obligation_command(&scope, &project.id, obligation(person, doc));
    command.confirm = false;
    command.spec.exception=Some(TechnicalException{source_reference:"Instrumento fictício de designação, cláusula 10".into(),source_url:"https://example.test/fundamento-ficticio".into(),reasoning:"Registro posterior à atividade em razão de emergência oficialmente decretada, encerrada em 10/01/2027; prazo de regularização e documento comprobatório submetidos à análise do responsável.".into()});
    let mut generic = command.clone();
    generic.confirm = true;
    generic.spec.exception.as_mut().unwrap().reasoning="Dispensado pela existência de campo opcional no sistema legado, sem outra base específica para o caso.".into();
    generic.spec.exception.as_mut().unwrap().source_reference = "SICOP".into();
    rejected(
        f.service.obligations_execute(&owner.token, generic).await,
        "invalid_input",
    );
    let draft = f
        .service
        .obligations_execute(&owner.token, command.clone())
        .await
        .unwrap()
        .obligation;
    let approver_scope = member(&f, &owner, &approver, &scope).await;
    grant(
        &f,
        &scope,
        &project.id,
        &owner,
        &approver,
        "approver",
        &["view", "approve_technical_exception"],
    )
    .await;
    command.context = approver_scope.clone();
    command.operation_key = Uuid::new_v4().to_string();
    command.obligation_id = Some(draft.id.clone());
    command.expected_version = Some(1);
    command.confirm = true;
    let mut altered = command.clone();
    altered.spec.description = "Descrição alterada durante aprovação".into();
    rejected(
        f.service
            .obligations_execute(&approver.token, altered)
            .await,
        "invalid_input",
    );
    let confirmed = f
        .service
        .obligations_execute(&approver.token, command)
        .await
        .unwrap()
        .obligation;
    assert!(confirmed.confirmed);
    assert_eq!(confirmed.spec, draft.spec);
    f.close().await;
}
