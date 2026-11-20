use canteiro_domain::{EntityId, obligations::*, projects::*};

fn entity(n: u128) -> EntityId {
    EntityId::from_uuid(uuid::Uuid::from_u128(n)).unwrap()
}
fn project() -> ProjectSpec {
    ProjectSpec {
        title: "Projeto de ampliação".into(),
        discipline: "Estruturas".into(),
        objective: "Definir a ampliação da edificação".into(),
        scope: "Anteprojeto estrutural do novo pavimento".into(),
        external_reference: String::new(),
        work_id: None,
        stakeholder_ids: vec![],
        deliveries: vec![ProjectDelivery {
            id: entity(1),
            title: "Anteprojeto".into(),
            responsible_id: Some(entity(2)),
            acceptance_criteria: "Plantas e memorial compatíveis com o escopo aprovado".into(),
            due_on: Some("2027-01-20".parse().unwrap()),
            milestone: true,
        }],
    }
}
fn obligation() -> ObligationSpec {
    ObligationSpec {
        kind: ObligationKind::Art,
        activity: TechnicalActivity::Budget,
        discipline: "Estruturas".into(),
        description: "Responsabilidade pelo orçamento estrutural".into(),
        professional_id: Some(entity(2)),
        number: "2620261234567".into(),
        issuer: "CREA-SC".into(),
        valid_from: "2026-11-01".parse().unwrap(),
        valid_until: Some("2027-01-31".parse().unwrap()),
        activity_on: "2027-01-10".parse().unwrap(),
        registered_on: Some("2026-11-10".parse().unwrap()),
        rrt_timing: None,
        document_id: Some(entity(3)),
        document_version: Some(1),
        exception: None,
        replaces_id: None,
    }
}

#[test]
fn independent_project_and_commitment_requirements() {
    let mut spec = project();
    assert!(spec.work_id.is_none());
    assert!(ProjectSnapshot::draft(spec.clone()).is_ok());
    spec.deliveries[0].responsible_id = None;
    let draft = ProjectSnapshot::draft(spec.clone()).unwrap();
    assert_eq!(draft.approve(), Err(ProjectError::IncompleteCommitment));
    spec.deliveries[0].responsible_id = Some(entity(2));
    spec.deliveries[0].acceptance_criteria = String::new();
    assert_eq!(
        ProjectSnapshot::draft(spec).unwrap().approve(),
        Err(ProjectError::IncompleteCommitment)
    );
}
#[test]
fn revising_approved_scope_preserves_previous_commitment_and_requires_new_approval() {
    let initial = ProjectSnapshot::draft(project())
        .unwrap()
        .approve()
        .unwrap();
    let original = initial.clone();
    let mut changed = initial.spec.clone();
    changed.scope = "Novo escopo incluindo fundações".into();
    let revision = initial.revise(changed).unwrap();
    assert_eq!(initial, original);
    assert_eq!(revision.status, ProjectStatus::Draft);
    assert_ne!(revision.spec.scope, initial.spec.scope);
    assert_eq!(revision.accept(entity(1)), Err(ProjectError::InvalidState));
}
#[test]
fn delivery_acceptance_only_changes_the_delivery_and_closing_requires_all() {
    let approved = ProjectSnapshot::draft(project())
        .unwrap()
        .approve()
        .unwrap();
    assert_eq!(approved.close(), Err(ProjectError::PendingDelivery));
    let accepted = approved.accept(entity(1)).unwrap();
    assert_eq!(accepted.spec, approved.spec);
    assert_eq!(accepted.accepted_deliveries, vec![entity(1)]);
    assert_eq!(
        accepted.accept(entity(1)),
        Err(ProjectError::InvalidDelivery)
    );
    assert_eq!(accepted.close().unwrap().status, ProjectStatus::Closed);
    assert_eq!(
        accepted.accept(entity(9)),
        Err(ProjectError::InvalidDelivery)
    );
}
#[test]
fn duplicate_deliveries_and_unknown_fields_do_not_enter_the_project() {
    let mut spec = project();
    spec.deliveries.push(spec.deliveries[0].clone());
    assert_eq!(spec.validate(false), Err(ProjectError::InvalidList));
    let mut value = serde_json::to_value(project()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("paid".into(), true.into());
    assert!(serde_json::from_value::<ProjectSpec>(value).is_err());
}
#[test]
fn budget_art_does_not_cover_execution_or_another_discipline() {
    let art = obligation();
    assert!(art.validate(true).is_ok());
    let date = "2027-01-10".parse().unwrap();
    assert!(art.covers(TechnicalActivity::Budget, "Estruturas", date));
    assert!(!art.covers(TechnicalActivity::Execution, "Estruturas", date));
    assert!(!art.covers(TechnicalActivity::Budget, "Elétrica", date));
    assert!(!art.covers(
        TechnicalActivity::Budget,
        "Estruturas",
        "2027-02-01".parse().unwrap()
    ));
}
#[test]
fn placeholders_and_unlinked_files_cannot_complete_an_obligation() {
    let mut art = obligation();
    for number in ["000000", "Pendente", "ART de teste", "A EMITIR"] {
        art.number = number.into();
        assert_eq!(art.validate(true), Err(ObligationError::EvidenceRequired));
    }
    art = obligation();
    art.document_id = None;
    art.document_version = None;
    assert_eq!(art.validate(true), Err(ObligationError::EvidenceRequired));
    assert!(art.validate(false).is_ok());
}
#[test]
fn council_and_registration_timing_depend_on_the_document_nature() {
    let mut art = obligation();
    art.issuer = "CAU-SC".into();
    assert_eq!(art.validate(true), Err(ObligationError::WrongCouncil));
    art = obligation();
    art.registered_on = Some("2027-01-11".parse().unwrap());
    assert_eq!(art.validate(true), Err(ObligationError::RegistrationTiming));
    art.kind = ObligationKind::Rrt;
    art.issuer = "CAU-SC".into();
    art.activity = TechnicalActivity::Design;
    assert_eq!(art.validate(true), Err(ObligationError::RrtEventRequired));
    art.rrt_timing = Some(RrtTiming {
        framework: RrtFramework::Project,
        event: RrtEvent::FinalDelivery,
        event_on: "2027-01-12".parse().unwrap(),
    });
    assert!(art.validate(true).is_ok());
    art.rrt_timing.as_mut().unwrap().event_on = "2027-01-10".parse().unwrap();
    assert_eq!(art.validate(true), Err(ObligationError::RegistrationTiming));
    art.activity = TechnicalActivity::Execution;
    assert_eq!(art.validate(true), Err(ObligationError::RegistrationTiming));
}
#[test]
fn other_rrt_activities_need_both_thirty_day_limit_and_completion_event() {
    let mut rrt = obligation();
    rrt.kind = ObligationKind::Rrt;
    rrt.issuer = "CAU-SC".into();
    rrt.activity = TechnicalActivity::Inspection;
    rrt.valid_until = None;
    rrt.rrt_timing = Some(RrtTiming {
        framework: RrtFramework::OtherActivities,
        event: RrtEvent::OtherActivityCompletion,
        event_on: "2027-03-10".parse().unwrap(),
    });
    rrt.registered_on = Some("2027-02-09".parse().unwrap());
    assert!(rrt.validate(true).is_ok());
    rrt.registered_on = Some("2027-02-10".parse().unwrap());
    assert_eq!(rrt.validate(true), Err(ObligationError::RegistrationTiming));
    rrt.registered_on = Some("2027-01-20".parse().unwrap());
    rrt.rrt_timing.as_mut().unwrap().event_on = "2027-01-19".parse().unwrap();
    assert_eq!(rrt.validate(true), Err(ObligationError::RegistrationTiming));
    rrt.rrt_timing = None;
    assert_eq!(rrt.validate(true), Err(ObligationError::RrtEventRequired));
}
#[test]
fn rrt_framework_cannot_be_switched_to_escape_the_applicable_deadline() {
    let mut rrt = obligation();
    rrt.kind = ObligationKind::Rrt;
    rrt.issuer = "CAU-SC".into();
    rrt.rrt_timing = Some(RrtTiming {
        framework: RrtFramework::Project,
        event: RrtEvent::FinalDelivery,
        event_on: "2027-01-25".parse().unwrap(),
    });
    // Orçamento integra o grupo 1, itens 1.7.3 e 1.10.3 da Resolução CAU 21.
    assert!(rrt.validate(true).is_ok());
    rrt.activity = TechnicalActivity::Inspection;
    assert_eq!(rrt.validate(true), Err(ObligationError::RrtEventRequired));
    let timing = rrt.rrt_timing.as_mut().unwrap();
    timing.framework = RrtFramework::OtherActivities;
    timing.event = RrtEvent::OtherActivityCompletion;
    assert!(rrt.validate(true).is_ok());
    rrt.activity = TechnicalActivity::Budget;
    assert_eq!(rrt.validate(true), Err(ObligationError::RrtEventRequired));
    rrt.activity = TechnicalActivity::Other;
    rrt.rrt_timing.as_mut().unwrap().event = RrtEvent::FinalDelivery;
    assert_eq!(rrt.validate(true), Err(ObligationError::RrtEventRequired));
    rrt.rrt_timing.as_mut().unwrap().framework = RrtFramework::ProjectCoordination;
    assert!(rrt.validate(true).is_ok());
}
#[test]
fn exception_requires_case_specific_basis_and_a_safe_source_even_in_draft() {
    let mut art = obligation();
    art.exception = Some(TechnicalException {
        source_reference: "SICOP".into(),
        source_url: "https://example.test".into(),
        reasoning: "Dispensado".into(),
    });
    assert_eq!(art.validate(true), Err(ObligationError::InvalidException));
    art.exception.as_mut().unwrap().source_url = ["java", "script", ":alert(1)"].concat();
    assert_eq!(art.validate(false), Err(ObligationError::InvalidException));
}
