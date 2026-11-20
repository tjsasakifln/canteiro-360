//! Percursos transacionais com identidades e organizações reais de uma base isolada.
use super::*;
use canteiro_contracts::access::{
    AccessAction as Action, AccessCommand, AccessResponse, AccessRole as Role, GrantView,
    RequestView, ScopeKind,
};
use canteiro_domain::decimal::Money;

const DELEGATION_REASON: &str = "Ausência temporária do titular";

fn money(value: &str) -> Money {
    value.parse().unwrap()
}

fn refused<T>(result: Result<T, AuthError>) -> AuthError {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("a operação deveria ser recusada"),
    };
    assert!(
        matches!(
            error.kind(),
            AuthErrorKind::Forbidden | AuthErrorKind::Conflict | AuthErrorKind::InvalidInput
        ),
        "recusa precisa ter motivo utilizável, sem falha interna: {error:?}"
    );
    error
}

impl Fixture {
    async fn access(&self, person: &Person) -> AccessResponse {
        self.service.access_context(&person.token).await.unwrap()
    }

    async fn access_command(
        &self,
        person: &Person,
        command: AccessCommand,
    ) -> Result<AccessResponse, AuthError> {
        self.service.access_execute(&person.token, command).await
    }

    async fn join_access_company(&self, owner: &Person, person: &Person, company: &str) {
        self.add(owner, person, OrganizationRole::Member).await;
        self.select(person, company).await;
    }

    async fn access_scope(&self, owner: &Person, name: &str, parent: Option<&str>) -> String {
        let response = self
            .access_command(
                owner,
                AccessCommand::CreateScope {
                    context: self.current_scope(owner).await,
                    kind: if parent.is_some() {
                        ScopeKind::Contract
                    } else {
                        ScopeKind::Work
                    },
                    name: name.into(),
                    parent_id: parent.map(str::to_owned),
                },
            )
            .await
            .unwrap();
        response
            .state
            .scopes
            .iter()
            .find(|scope| scope.name == name)
            .unwrap()
            .id
            .clone()
    }

    async fn give_access(
        &self,
        owner: &Person,
        person: &Person,
        scope_id: &str,
        role: Role,
        maximum: &str,
        required: bool,
    ) -> GrantView {
        let state = self.access(owner).await.state;
        let kind = state
            .scopes
            .iter()
            .find(|scope| scope.id == scope_id)
            .expect("fixture scope exists")
            .kind;
        let previous: Vec<_> = state.grants.into_iter().map(|grant| grant.id).collect();
        let response = self
            .access_command(
                owner,
                AccessCommand::Grant {
                    context: self.current_scope(owner).await,
                    scope_id: scope_id.into(),
                    user_email: person.email.clone(),
                    role,
                    actions: Action::ALL
                        .into_iter()
                        .filter(|action| role.permits(*action) && action.applies_to(kind))
                        .collect(),
                    max_amount: money(maximum),
                    valid_from: "2000-01-01T00:00:00Z".into(),
                    valid_until: if required {
                        None
                    } else {
                        Some("2099-01-01T00:00:00Z".into())
                    },
                    required,
                },
            )
            .await
            .unwrap();
        response
            .state
            .grants
            .iter()
            .find(|grant| !previous.contains(&grant.id))
            .unwrap()
            .clone()
    }

    async fn access_policy(
        &self,
        owner: &Person,
        scope_id: &str,
        threshold: &str,
        approvals: u8,
        expected: Option<i64>,
    ) -> AccessResponse {
        self.access_command(
            owner,
            AccessCommand::SetPolicy {
                context: self.current_scope(owner).await,
                scope_id: scope_id.into(),
                action: Action::ApproveChange,
                threshold: money(threshold),
                approvals_above_threshold: approvals,
                segregate_proposer: true,
                expected_version: expected,
            },
        )
        .await
        .unwrap()
    }

    async fn propose_access(
        &self,
        person: &Person,
        scope_id: &str,
        title: &str,
        increase: &str,
        decrease: &str,
    ) -> RequestView {
        let response = self
            .access_command(
                person,
                AccessCommand::Propose {
                    context: self.current_scope(person).await,
                    scope_id: scope_id.into(),
                    action: Action::ApproveChange,
                    title: title.into(),
                    increase: money(increase),
                    decrease: money(decrease),
                    conflicted_user_ids: vec![],
                },
            )
            .await
            .unwrap();
        response
            .state
            .requests
            .iter()
            .find(|request| request.title == title)
            .unwrap()
            .clone()
    }

    async fn request_access_view(&self, person: &Person, request_id: &str) -> RequestView {
        self.access(person)
            .await
            .state
            .requests
            .into_iter()
            .find(|request| request.id == request_id)
            .unwrap()
    }

    async fn decide_access(
        &self,
        person: &Person,
        request: &RequestView,
        grant: &GrantView,
    ) -> Result<AccessResponse, AuthError> {
        self.access_command(
            person,
            AccessCommand::Decide {
                context: self.current_scope(person).await,
                request_id: request.id.clone(),
                request_version: request.version,
                grant_id: grant.id.clone(),
                grant_version: grant.version,
                approve: true,
            },
        )
        .await
    }

    async fn delegate_access(
        &self,
        source_person: &Person,
        target: &Person,
        source: &GrantView,
    ) -> GrantView {
        let response = self
            .access_command(
                source_person,
                AccessCommand::Delegate {
                    context: self.current_scope(source_person).await,
                    source_grant_id: source.id.clone(),
                    source_version: source.version,
                    user_email: target.email.clone(),
                    actions: vec![Action::View, Action::ApproveChange],
                    max_amount: source.max_amount,
                    valid_from: source.valid_from.clone(),
                    valid_until: source
                        .valid_until
                        .clone()
                        .unwrap_or_else(|| "2099-01-01T00:00:00Z".into()),
                    substitution: true,
                    reason: DELEGATION_REASON.into(),
                },
            )
            .await
            .unwrap();
        response
            .state
            .grants
            .iter()
            .find(|grant| {
                grant.user_id == target.id.to_string()
                    && grant.source_grant_id.as_deref() == Some(&source.id)
            })
            .unwrap()
            .clone()
    }

    async fn access_rows(&self) -> (i64, i64, i64, i64, i64) {
        sqlx::query_as("SELECT (SELECT count(*) FROM canteiro.access_grants),(SELECT count(*) FROM canteiro.access_requests),(SELECT count(*) FROM canteiro.access_request_versions),(SELECT count(*) FROM canteiro.access_approvals),(SELECT count(*) FROM canteiro.organization_audit)")
            .fetch_one(&self.admin).await.unwrap()
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_roles_exact_scope_and_organization_do_not_allow_direct_command_escalation() {
    let f = Fixture::new(4).await;
    let owner = f.person("access-owner", true).await;
    let reader = f.person("access-reader", true).await;
    let inspector = f.person("access-inspector", false).await;
    let other_owner = f.person("other-access-owner", true).await;
    let (company, _) = f.create(&owner, "Empresa Acesso Aurora").await;
    f.select(&owner, &company).await;
    for person in [&reader, &inspector] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Horizonte", None).await;
    let a = f
        .access_scope(&owner, "Contrato Estrutura", Some(&work))
        .await;
    let b = f
        .access_scope(&owner, "Contrato Instalações", Some(&work))
        .await;
    f.access_policy(&owner, &a, "100", 2, None).await;
    f.access_policy(&owner, &b, "100", 2, None).await;
    let reader_grant = f
        .give_access(&owner, &reader, &a, Role::Viewer, "0", false)
        .await;
    f.give_access(&owner, &inspector, &a, Role::Inspector, "1000", false)
        .await;
    let (foreign_company, _) = f.create(&other_owner, "Empresa Alheia").await;
    f.select(&other_owner, &foreign_company).await;
    let foreign = f.access_scope(&other_owner, "Obra Sigilosa", None).await;
    let before = f.access_rows().await;
    for (person, scope_id) in [
        (&reader, &a),
        (&inspector, &b),
        (&inspector, &work),
        (&inspector, &foreign),
        (&owner, &a),
    ] {
        refused(
            f.access_command(
                person,
                AccessCommand::Propose {
                    context: f.current_scope(person).await,
                    scope_id: scope_id.clone(),
                    action: Action::ApproveChange,
                    title: "Alteração indevida".into(),
                    increase: money("1"),
                    decrease: money("0"),
                    conflicted_user_ids: vec![],
                },
            )
            .await,
        );
    }
    refused(
        f.access_command(
            &reader,
            AccessCommand::Grant {
                context: f.current_scope(&reader).await,
                scope_id: a.clone(),
                user_email: reader.email.clone(),
                role: Role::Approver,
                actions: vec![Action::View, Action::ApproveChange],
                max_amount: money("1000"),
                valid_from: "2000-01-01T00:00:00Z".into(),
                valid_until: None,
                required: false,
            },
        )
        .await,
    );
    refused(
        f.access_command(
            &reader,
            AccessCommand::Delegate {
                context: f.current_scope(&reader).await,
                source_grant_id: reader_grant.id,
                source_version: reader_grant.version,
                user_email: inspector.email.clone(),
                actions: vec![Action::View],
                max_amount: money("0"),
                valid_from: reader_grant.valid_from,
                valid_until: reader_grant.valid_until.unwrap(),
                substitution: false,
                reason: DELEGATION_REASON.into(),
            },
        )
        .await,
    );
    assert_eq!(
        f.access_rows().await,
        before,
        "recusas não criam direitos, decisões ou auditoria de sucesso"
    );
    let proposal = f
        .propose_access(&inspector, &a, "Revisão autorizada da estrutura", "25", "0")
        .await;
    let inspector_state = f.access(&inspector).await.state;
    assert!(inspector_state.scopes.iter().any(|scope| scope.id == a));
    assert!(
        inspector_state
            .scopes
            .iter()
            .all(|scope| ![&b, &foreign].contains(&&scope.id))
    );
    assert_eq!(inspector_state.requests.len(), 1);
    assert_eq!(inspector_state.requests[0].id, proposal.id);
    let owner_state = f.access(&owner).await.state;
    assert!(owner_state.can_manage);
    assert!(
        owner_state.requests.is_empty(),
        "administrar direitos não dá leitura de pedidos sem vínculo específico"
    );
    let reader_state = f.access(&reader).await.state;
    assert!(!reader_state.can_manage);
    assert!(
        reader_state
            .requests
            .iter()
            .all(|request| request.allowed_grant_ids.is_empty())
    );
    assert!(f.access(&other_owner).await.state.requests.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_mixed_amount_segregation_concurrent_approvals_and_revisions_preserve_history() {
    let f = Fixture::new(5).await;
    let owner = f.person("quorum-owner", true).await;
    let proposer = f.person("quorum-proposer", true).await;
    let a = f.person("quorum-a", true).await;
    let b = f.person("quorum-b", true).await;
    let delegate = f.person("quorum-delegate", true).await;
    let low = f.person("quorum-low", true).await;
    let (company, _) = f.create(&owner, "Empresa Alçadas").await;
    f.select(&owner, &company).await;
    for person in [&proposer, &a, &b, &delegate, &low] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Alçadas", None).await;
    f.access_policy(&owner, &work, "100", 2, None).await;
    f.give_access(&owner, &proposer, &work, Role::Inspector, "1000", false)
        .await;
    let self_approval = f
        .give_access(&owner, &proposer, &work, Role::Approver, "1000", false)
        .await;
    let ga = f
        .give_access(&owner, &a, &work, Role::Approver, "100", false)
        .await;
    let gb = f
        .give_access(&owner, &b, &work, Role::Approver, "100", false)
        .await;
    let gl = f
        .give_access(&owner, &low, &work, Role::Approver, "99.99", false)
        .await;
    let gd = f
        .delegate_access(&proposer, &delegate, &self_approval)
        .await;
    let proposal = f
        .propose_access(
            &proposer,
            &work,
            "Acréscimo e redução simultâneos",
            "100",
            "100",
        )
        .await;
    assert_eq!(proposal.authority_amount, money("100"));
    assert!(proposal.contains_increase);
    assert_eq!(proposal.required_approvals, 2);
    assert_eq!(proposal.valid_approvals, 0);
    let before = f.access_rows().await;
    refused(f.decide_access(&proposer, &proposal, &self_approval).await);
    refused(f.decide_access(&delegate, &proposal, &gd).await);
    refused(f.decide_access(&low, &proposal, &gl).await);
    assert_eq!(f.access_rows().await, before);
    let command = AccessCommand::Decide {
        context: f.current_scope(&a).await,
        request_id: proposal.id.clone(),
        request_version: proposal.version,
        grant_id: ga.id.clone(),
        grant_version: ga.version,
        approve: true,
    };
    let (first, repeat) = tokio::join!(
        f.access_command(&a, command.clone()),
        f.access_command(&a, command)
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(repeat.is_ok()), 1);
    if let Err(error) = first {
        assert_eq!(error.kind(), AuthErrorKind::Conflict);
    }
    if let Err(error) = repeat {
        assert_eq!(error.kind(), AuthErrorKind::Conflict);
    }
    let pending = f.request_access_view(&proposer, &proposal.id).await;
    assert_eq!(pending.status, "pending");
    assert_eq!(pending.valid_approvals, 1);
    let response = f
        .access_command(
            &proposer,
            AccessCommand::ReviseRequest {
                context: f.current_scope(&proposer).await,
                request_id: proposal.id.clone(),
                request_version: proposal.version,
                title: "Acréscimo e redução, revisão de escopo".into(),
                increase: money("100"),
                decrease: money("100"),
                conflicted_user_ids: vec![],
            },
        )
        .await
        .unwrap();
    let revised = response
        .state
        .requests
        .iter()
        .find(|request| request.id == proposal.id)
        .unwrap()
        .clone();
    assert_eq!(revised.version, proposal.version + 1);
    assert_eq!(revised.status, "pending");
    assert_eq!(revised.valid_approvals, 0);
    let counts: (i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM canteiro.access_request_versions WHERE request_id=$1),(SELECT count(*) FROM canteiro.access_approvals WHERE request_id=$1)")
        .bind(Uuid::parse_str(&proposal.id).unwrap()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(
        counts,
        (2, 1),
        "nova versão preserva as assinaturas antigas sem reutilizá-las"
    );
    let before = f.access_rows().await;
    refused(f.decide_access(&a, &proposal, &ga).await);
    assert_eq!(f.access_rows().await, before);
    f.access_policy(&owner, &work, "50", 2, Some(1)).await;
    let changed = f.request_access_view(&proposer, &proposal.id).await;
    assert!(changed.policy_changed);
    let before = f.access_rows().await;
    refused(f.decide_access(&a, &revised, &ga).await);
    assert_eq!(f.access_rows().await, before);
    f.access_command(
        &proposer,
        AccessCommand::ReviseRequest {
            context: f.current_scope(&proposer).await,
            request_id: proposal.id.clone(),
            request_version: revised.version,
            title: "Pedido conferido na política atual".into(),
            increase: money("100"),
            decrease: money("100"),
            conflicted_user_ids: vec![],
        },
    )
    .await
    .unwrap();
    let current = f.request_access_view(&proposer, &proposal.id).await;
    assert_eq!(current.version, 3);
    assert_eq!(current.policy_version, 2);
    assert!(!current.policy_changed);
    assert_eq!(current.required_approvals, 2);
    f.decide_access(&a, &current, &ga).await.unwrap();
    assert_eq!(
        f.request_access_view(&proposer, &proposal.id)
            .await
            .valid_approvals,
        1
    );
    f.decide_access(&b, &current, &gb).await.unwrap();
    let approved = f.request_access_view(&proposer, &proposal.id).await;
    assert_eq!(approved.status, "approved");
    assert_eq!(approved.valid_approvals, 2);
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &proposer,
            AccessCommand::ReviseRequest {
                context: f.current_scope(&proposer).await,
                request_id: proposal.id,
                request_version: current.version,
                title: "Tentativa de reescrita após decisão".into(),
                increase: money("1"),
                decrease: money("0"),
                conflicted_user_ids: vec![],
            },
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_delegation_is_bounded_and_loses_effect_when_its_source_or_membership_ends() {
    let f = Fixture::new(4).await;
    let owner = f.person("delegate-owner", true).await;
    let source_person = f.person("delegate-source", true).await;
    let target = f.person("delegate-target", true).await;
    let proposer = f.person("delegate-proposer", false).await;
    let (company, _) = f.create(&owner, "Empresa Substituições").await;
    f.select(&owner, &company).await;
    for person in [&source_person, &target, &proposer] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Substituições", None).await;
    f.access_policy(&owner, &work, "100", 2, None).await;
    f.give_access(&owner, &proposer, &work, Role::Inspector, "1000", false)
        .await;
    let source_response = f
        .access_command(
            &owner,
            AccessCommand::Grant {
                context: f.current_scope(&owner).await,
                scope_id: work.clone(),
                user_email: source_person.email.clone(),
                role: Role::Approver,
                actions: vec![Action::View, Action::ApproveChange],
                max_amount: money("100"),
                valid_from: "2000-01-01T00:00:00Z".into(),
                valid_until: Some("2099-01-01T00:00:00Z".into()),
                required: false,
            },
        )
        .await
        .unwrap();
    let source = source_response
        .state
        .grants
        .iter()
        .find(|grant| grant.user_id == source_person.id.to_string())
        .unwrap()
        .clone();
    let before = f.access_rows().await;
    for (actions, maximum, from, until) in [
        (
            vec![Action::View, Action::ApproveChange],
            "100.01",
            source.valid_from.as_str(),
            "2099-01-01T00:00:00Z",
        ),
        (
            vec![Action::View, Action::AuthorizePayment],
            "100",
            source.valid_from.as_str(),
            "2099-01-01T00:00:00Z",
        ),
        (
            vec![Action::View, Action::ApproveChange],
            "100",
            "1999-12-31T23:59:59Z",
            "2099-01-01T00:00:00Z",
        ),
        (
            vec![Action::View, Action::ApproveChange],
            "100",
            source.valid_from.as_str(),
            "2099-01-01T00:00:01Z",
        ),
    ] {
        refused(
            f.access_command(
                &source_person,
                AccessCommand::Delegate {
                    context: f.current_scope(&source_person).await,
                    source_grant_id: source.id.clone(),
                    source_version: source.version,
                    user_email: target.email.clone(),
                    actions,
                    max_amount: money(maximum),
                    valid_from: from.into(),
                    valid_until: until.into(),
                    substitution: true,
                    reason: DELEGATION_REASON.into(),
                },
            )
            .await,
        );
    }
    let missing_reason = refused(
        f.access_command(
            &source_person,
            AccessCommand::Delegate {
                context: f.current_scope(&source_person).await,
                source_grant_id: source.id.clone(),
                source_version: source.version,
                user_email: target.email.clone(),
                actions: vec![Action::View, Action::ApproveChange],
                max_amount: money("100"),
                valid_from: source.valid_from.clone(),
                valid_until: source.valid_until.clone().unwrap(),
                substitution: true,
                reason: String::new(),
            },
        )
        .await,
    );
    assert_eq!(missing_reason.kind(), AuthErrorKind::InvalidInput);
    assert_eq!(f.access_rows().await, before);
    let delegated = f.delegate_access(&source_person, &target, &source).await;
    assert_eq!(delegated.reason, DELEGATION_REASON);
    let refreshed = f.access(&target).await;
    assert_eq!(
        refreshed
            .state
            .grants
            .iter()
            .find(|grant| grant.id == delegated.id)
            .unwrap()
            .reason,
        DELEGATION_REASON
    );
    let detail: String = sqlx::query_scalar("SELECT detail FROM canteiro.organization_audit WHERE event='access_delegated' AND subject_id=$1")
        .bind(Uuid::parse_str(&delegated.id).unwrap()).fetch_one(&f.admin).await.unwrap();
    assert!(detail.contains(DELEGATION_REASON));
    assert!(detail.contains(&source.id));
    refused(
        f.access_command(
            &target,
            AccessCommand::Delegate {
                context: f.current_scope(&target).await,
                source_grant_id: delegated.id.clone(),
                source_version: delegated.version,
                user_email: proposer.email.clone(),
                actions: vec![Action::View, Action::ApproveChange],
                max_amount: money("1"),
                valid_from: delegated.valid_from.clone(),
                valid_until: delegated.valid_until.clone().unwrap(),
                substitution: false,
                reason: DELEGATION_REASON.into(),
            },
        )
        .await,
    );
    let request = f
        .propose_access(&proposer, &work, "Decisão por substituto", "100", "0")
        .await;
    f.decide_access(&target, &request, &delegated)
        .await
        .unwrap();
    assert_eq!(
        f.request_access_view(&proposer, &request.id).await.status,
        "pending"
    );
    assert_eq!(
        f.request_access_view(&proposer, &request.id)
            .await
            .valid_approvals,
        1
    );
    f.access_command(
        &owner,
        AccessCommand::RevokeGrant {
            context: f.current_scope(&owner).await,
            grant_id: source.id.clone(),
            grant_version: source.version,
        },
    )
    .await
    .unwrap();
    let invalidated = f.request_access_view(&proposer, &request.id).await;
    assert_eq!(invalidated.valid_approvals, 0);
    assert_eq!(invalidated.status, "pending");
    let second_source = f
        .give_access(&owner, &source_person, &work, Role::Approver, "100", false)
        .await;
    let second_delegation = f
        .delegate_access(&source_person, &target, &second_source)
        .await;
    let second = f
        .propose_access(
            &proposer,
            &work,
            "Decisão antes da saída do titular",
            "100",
            "0",
        )
        .await;
    f.decide_access(&target, &second, &second_delegation)
        .await
        .unwrap();
    let membership = f
        .context(&owner)
        .await
        .state
        .active
        .unwrap()
        .members
        .into_iter()
        .find(|member| member.id == source_person.id.to_string())
        .unwrap();
    f.execute(
        &owner,
        OrganizationCommand::RemoveMember {
            scope: f.current_scope(&owner).await,
            user_id: membership.id,
            member_version: membership.version,
        },
    )
    .await
    .unwrap();
    let invalidated = f.request_access_view(&proposer, &second.id).await;
    assert_eq!(invalidated.valid_approvals, 0);
    assert_eq!(invalidated.status, "pending");
    let third = f
        .propose_access(&proposer, &work, "Pedido após saída do titular", "100", "0")
        .await;
    let before = f.access_rows().await;
    refused(f.decide_access(&target, &third, &second_delegation).await);
    assert_eq!(f.access_rows().await, before);
    let historical: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_approvals")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(
        historical, 2,
        "perda de autoridade não apaga decisões históricas"
    );
    let decision_actors: Vec<Uuid> =
        sqlx::query_scalar("SELECT actor_id FROM canteiro.access_approvals")
            .fetch_all(&f.admin)
            .await
            .unwrap();
    assert!(
        decision_actors.iter().all(|actor| *actor == target.id),
        "o substituto assina com a própria identidade, nunca em nome do titular"
    );
    let original_author: Uuid =
        sqlx::query_scalar("SELECT created_by FROM canteiro.access_grants WHERE id=$1")
            .bind(Uuid::parse_str(&second_delegation.id).unwrap())
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(
        original_author, source_person.id,
        "a saída do titular não reescreve a autoria da delegação"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_required_responsible_survives_member_changes_and_concurrent_revocations() {
    let f = Fixture::new(5).await;
    let owner = f.person("required-owner", true).await;
    let other_admin = f.person("required-admin", true).await;
    let a = f.person("required-a", true).await;
    let b = f.person("required-b", true).await;
    let (company, _) = f.create(&owner, "Empresa Responsabilidade").await;
    f.select(&owner, &company).await;
    f.add(&owner, &other_admin, OrganizationRole::Administrator)
        .await;
    f.select(&other_admin, &company).await;
    for person in [&a, &b] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Responsável", None).await;
    let ga = f
        .give_access(&owner, &a, &work, Role::TechnicalResponsible, "0", true)
        .await;
    let member_a = f
        .context(&owner)
        .await
        .state
        .active
        .unwrap()
        .members
        .into_iter()
        .find(|member| member.id == a.id.to_string())
        .unwrap();
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &owner,
            AccessCommand::RevokeGrant {
                context: f.current_scope(&owner).await,
                grant_id: ga.id.clone(),
                grant_version: ga.version,
            },
        )
        .await,
    );
    refused(
        f.execute(
            &owner,
            OrganizationCommand::RemoveMember {
                scope: f.current_scope(&owner).await,
                user_id: member_a.id.clone(),
                member_version: member_a.version,
            },
        )
        .await,
    );
    refused(
        f.execute(
            &owner,
            OrganizationCommand::ChangeMember {
                scope: f.current_scope(&owner).await,
                user_id: member_a.id,
                member_version: member_a.version,
                role: OrganizationRole::Member,
                valid_until: Some("2099-01-01T00:00:00Z".into()),
            },
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    let preserved = f.access(&owner).await.state;
    let holder = preserved
        .grants
        .iter()
        .find(|grant| grant.id == ga.id)
        .unwrap();
    assert!(holder.effective && !holder.revoked);
    assert_eq!(holder.version, ga.version);
    let member = preserved
        .members
        .iter()
        .find(|member| member.id == a.id.to_string())
        .unwrap();
    assert!(member.active);
    assert!(member.valid_until.is_none());
    let gb = f
        .give_access(&owner, &b, &work, Role::TechnicalResponsible, "0", true)
        .await;
    let ca = f.current_scope(&owner).await;
    let cb = f.current_scope(&other_admin).await;
    let (one, two) = tokio::join!(
        f.access_command(
            &owner,
            AccessCommand::RevokeGrant {
                context: ca,
                grant_id: ga.id,
                grant_version: ga.version
            }
        ),
        f.access_command(
            &other_admin,
            AccessCommand::RevokeGrant {
                context: cb,
                grant_id: gb.id,
                grant_version: gb.version
            }
        )
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    if one.is_err() {
        refused(one);
    }
    if two.is_err() {
        refused(two);
    }
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_grants g JOIN canteiro.organization_members m ON m.tenant_id=g.tenant_id AND m.user_id=g.user_id WHERE g.scope_id=$1 AND g.role='technical_responsible' AND g.revoked_at IS NULL AND g.source_grant_id IS NULL AND g.valid_until IS NULL AND m.active AND m.valid_until IS NULL")
        .bind(Uuid::parse_str(&work).unwrap()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(remaining, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_stale_context_grant_policy_and_request_versions_leave_no_business_write() {
    let f = Fixture::new(4).await;
    let owner = f.person("stale-owner", true).await;
    let proposer = f.person("stale-proposer", false).await;
    let approver = f.person("stale-approver", true).await;
    let (company, _) = f.create(&owner, "Empresa Versões").await;
    f.select(&owner, &company).await;
    for person in [&proposer, &approver] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Versões", None).await;
    f.access_policy(&owner, &work, "100", 1, None).await;
    f.give_access(&owner, &proposer, &work, Role::Inspector, "1000", false)
        .await;
    let grant = f
        .give_access(&owner, &approver, &work, Role::Approver, "1000", false)
        .await;
    let stale_context = f.current_scope(&proposer).await;
    f.access_policy(&owner, &work, "101", 1, Some(1)).await;
    let before = f.access_rows().await;
    let error = refused(
        f.access_command(
            &proposer,
            AccessCommand::Propose {
                context: stale_context,
                scope_id: work.clone(),
                action: Action::ApproveChange,
                title: "Pedido com contexto antigo".into(),
                increase: money("1"),
                decrease: money("0"),
                conflicted_user_ids: vec![],
            },
        )
        .await,
    );
    assert_eq!(error.kind(), AuthErrorKind::Conflict);
    refused(
        f.access_command(
            &owner,
            AccessCommand::SetPolicy {
                context: f.current_scope(&owner).await,
                scope_id: work.clone(),
                action: Action::ApproveChange,
                threshold: money("100"),
                approvals_above_threshold: 1,
                segregate_proposer: true,
                expected_version: Some(1),
            },
        )
        .await,
    );
    refused(
        f.access_command(
            &owner,
            AccessCommand::RevokeGrant {
                context: f.current_scope(&owner).await,
                grant_id: grant.id.clone(),
                grant_version: grant.version + 1,
            },
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    let request = f
        .propose_access(&proposer, &work, "Pedido atual", "100", "0")
        .await;
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &approver,
            AccessCommand::Decide {
                context: f.current_scope(&approver).await,
                request_id: request.id.clone(),
                request_version: request.version,
                grant_id: grant.id.clone(),
                grant_version: grant.version + 1,
                approve: true,
            },
        )
        .await,
    );
    refused(
        f.access_command(
            &approver,
            AccessCommand::Decide {
                context: f.current_scope(&approver).await,
                request_id: request.id.clone(),
                request_version: request.version + 1,
                grant_id: grant.id.clone(),
                grant_version: grant.version,
                approve: true,
            },
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    f.decide_access(&approver, &request, &grant).await.unwrap();
    let old = f.current_scope(&owner).await;
    let (other, _) = f.create(&owner, "Outra Empresa Versões").await;
    f.select(&owner, &other).await;
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &owner,
            AccessCommand::RevokeGrant {
                context: old,
                grant_id: grant.id,
                grant_version: grant.version,
            },
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_external_budgeter_only_operates_the_granted_budget_during_current_validity() {
    let f = Fixture::new(4).await;
    let owner = f.person("budget-scope-owner", true).await;
    let external = f.person("external-budgeter", false).await;
    let contract_only = f.person("contract-budgeter", false).await;
    let (company, _) = f.create(&owner, "Empresa Orçamentos Restritos").await;
    f.select(&owner, &company).await;
    for person in [&external, &contract_only] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra Orçamentos", None).await;
    let contract = f
        .access_scope(&owner, "Contrato de Projetos", Some(&work))
        .await;
    let mut budgets = Vec::new();
    for name in ["Orçamento Estrutural", "Orçamento Instalações"] {
        let response = f
            .access_command(
                &owner,
                AccessCommand::CreateScope {
                    context: f.current_scope(&owner).await,
                    kind: ScopeKind::Budget,
                    name: name.into(),
                    parent_id: Some(contract.clone()),
                },
            )
            .await
            .unwrap();
        let budget = response
            .state
            .scopes
            .iter()
            .find(|scope| scope.name == name)
            .unwrap();
        assert_eq!(budget.kind, ScopeKind::Budget);
        assert_eq!(budget.parent_id.as_deref(), Some(contract.as_str()));
        budgets.push(budget.id.clone());
    }
    let a = &budgets[0];
    let b = &budgets[1];
    for scope_id in [a, b] {
        f.access_command(
            &owner,
            AccessCommand::SetPolicy {
                context: f.current_scope(&owner).await,
                scope_id: scope_id.clone(),
                action: Action::ApproveBudget,
                threshold: money("100"),
                approvals_above_threshold: 2,
                segregate_proposer: true,
                expected_version: None,
            },
        )
        .await
        .unwrap();
    }
    let grant = f
        .give_access(&owner, &external, a, Role::Budgeter, "100", false)
        .await;
    f.give_access(
        &owner,
        &contract_only,
        &contract,
        Role::Budgeter,
        "100",
        false,
    )
    .await;
    assert!(grant.valid_until.is_some());
    let propose =
        |context: OrganizationScope, scope_id: &str, title: &str| AccessCommand::Propose {
            context,
            scope_id: scope_id.into(),
            action: Action::ApproveBudget,
            title: title.into(),
            increase: money("50"),
            decrease: money("0"),
            conflicted_user_ids: vec![],
        };
    let allowed = f
        .access_command(
            &external,
            propose(
                f.current_scope(&external).await,
                a,
                "Orçamento autorizado A",
            ),
        )
        .await
        .unwrap();
    assert!(
        allowed
            .state
            .requests
            .iter()
            .any(|request| request.scope_id == *a && request.action == Action::ApproveBudget)
    );
    assert!(allowed.state.scopes.iter().any(|scope| scope.id == *a));
    assert!(allowed.state.scopes.iter().all(|scope| scope.id != *b));
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &external,
            propose(f.current_scope(&external).await, b, "Orçamento alheio B"),
        )
        .await,
    );
    for budget in [a, b] {
        refused(
            f.access_command(
                &contract_only,
                propose(
                    f.current_scope(&contract_only).await,
                    budget,
                    "Tentativa por vínculo contratual",
                ),
            )
            .await,
        );
    }
    assert_eq!(f.access_rows().await, before);
    // Simula o vencimento pelo relógio do banco sem encerrar a sessão já aberta.
    sqlx::query("UPDATE canteiro.access_grants SET valid_until=clock_timestamp()-interval '1 second' WHERE tenant_id=$1 AND id=$2")
        .bind(Uuid::parse_str(&company).unwrap()).bind(Uuid::parse_str(&grant.id).unwrap())
        .execute(&f.admin).await.unwrap();
    assert!(f.access(&external).await.state.scopes.is_empty());
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &external,
            propose(
                f.current_scope(&external).await,
                a,
                "Tentativa após vencimento",
            ),
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    let renewed = f
        .give_access(&owner, &external, a, Role::Budgeter, "100", false)
        .await;
    f.access_command(
        &external,
        propose(
            f.current_scope(&external).await,
            a,
            "Pedido após nova concessão",
        ),
    )
    .await
    .unwrap();
    f.access_command(
        &owner,
        AccessCommand::RevokeGrant {
            context: f.current_scope(&owner).await,
            grant_id: renewed.id,
            grant_version: renewed.version,
        },
    )
    .await
    .unwrap();
    let before = f.access_rows().await;
    refused(
        f.access_command(
            &external,
            propose(
                f.current_scope(&external).await,
                a,
                "Tentativa após revogação",
            ),
        )
        .await,
    );
    assert_eq!(f.access_rows().await, before);
    assert!(f.access(&external).await.state.requests.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn authority_receipts_and_financial_effect_survive_retries_and_membership_removal() {
    use canteiro_contracts::reliability::ReliabilityScope;

    let f = Fixture::new(5).await;
    let owner = f.person("history-owner", true).await;
    let proposer = f.person("history-proposer", false).await;
    let approver = f.person("history-approver", true).await;
    let alternate = f.person("history-alternate", true).await;
    let (company, _) = f.create(&owner, "Empresa de efeitos concorrentes").await;
    f.select(&owner, &company).await;
    for person in [&proposer, &approver, &alternate] {
        f.join_access_company(&owner, person, &company).await;
    }
    let work = f.access_scope(&owner, "Obra do recibo", None).await;
    let contract = f
        .access_scope(&owner, "Contrato do recibo", Some(&work))
        .await;
    let another = f.access_scope(&owner, "Outro contrato", Some(&work)).await;
    for scope in [&contract, &another] {
        f.access_policy(&owner, scope, "1000", 2, None).await;
        f.give_access(&owner, &proposer, scope, Role::Inspector, "1000", false)
            .await;
    }
    let approval = f
        .give_access(&owner, &approver, &contract, Role::Approver, "1000", false)
        .await;
    f.give_access(&owner, &approver, &contract, Role::Viewer, "0", false)
        .await;
    let alternate_approval = f
        .give_access(&owner, &alternate, &contract, Role::Approver, "1000", false)
        .await;
    let new_proposal = |context, scope: &str, label: &str, increase: &str| AccessCommand::Propose {
        context,
        scope_id: scope.into(),
        action: Action::ApproveChange,
        title: label.into(),
        increase: money(increase),
        decrease: money("16.13"),
        conflicted_user_ids: vec![],
    };
    let original_context = f.current_scope(&proposer).await;
    let scope = ReliabilityScope {
        organization_id: company.clone(),
        context_version: original_context.context_version,
    };
    let execute = |person: &Person, command, key: &str| {
        let service = f.service.clone();
        let token = person.token.clone();
        let key = key.to_owned();
        async move {
            service
                .access_execute_with_key(&token, command, Some(&key))
                .await
        }
    };
    let proposal_key = Uuid::new_v4().to_string();
    let original = new_proposal(
        original_context.clone(),
        &contract,
        " Mudança auditável ",
        "100",
    );
    let equivalent = new_proposal(
        original_context.clone(),
        &contract,
        "Mudança auditável",
        "100.00",
    );
    let (first, second) = tokio::join!(
        execute(&proposer, original, &proposal_key),
        execute(&proposer, equivalent, &proposal_key)
    );
    let first = first.unwrap().receipt.unwrap();
    assert_eq!(Some(first.clone()), second.unwrap().receipt);
    assert_eq!(
        f.service
            .access_operation(&proposer.token, &scope, "authority_propose", &proposal_key)
            .await
            .unwrap()
            .unwrap()
            .receipt,
        Some(first.clone())
    );
    for (target, amount) in [(&contract, "100.01"), (&another, "100.00")] {
        let changed = new_proposal(
            original_context.clone(),
            target,
            "Mudança auditável",
            amount,
        );
        assert_eq!(
            refused(execute(&proposer, changed, &proposal_key).await).code(),
            "idempotency_conflict"
        );
    }
    let foreign_scope = ReliabilityScope {
        organization_id: company.clone(),
        context_version: f.current_scope(&approver).await.context_version,
    };
    assert!(
        f.service
            .access_operation(
                &approver.token,
                &foreign_scope,
                "authority_propose",
                &proposal_key
            )
            .await
            .unwrap()
            .is_none(),
        "a chave por si só não autoriza a consulta, mesmo dentro do contrato"
    );

    let revision_key = Uuid::new_v4().to_string();
    let revision = AccessCommand::ReviseRequest {
        context: f.current_scope(&proposer).await,
        request_id: first.request_id.clone(),
        request_version: first.version,
        title: "Mudança revisada".into(),
        increase: money("120.21"),
        decrease: money("20.20"),
        conflicted_user_ids: vec![],
    };
    let (saved, replayed) = tokio::join!(
        execute(&proposer, revision.clone(), &revision_key),
        execute(&proposer, revision, &revision_key)
    );
    let revised = saved.unwrap().receipt.unwrap();
    assert_eq!(revised.version, 2);
    assert_eq!(Some(revised.clone()), replayed.unwrap().receipt);
    assert_eq!(
        f.service
            .access_operation(&proposer.token, &scope, "authority_propose", &proposal_key)
            .await
            .unwrap()
            .unwrap()
            .receipt,
        Some(first.clone()),
        "revisar depois não altera o recibo da versão original"
    );

    let decide = |context, grant: &GrantView, approve| AccessCommand::Decide {
        context,
        request_id: revised.request_id.clone(),
        request_version: revised.version,
        grant_id: grant.id.clone(),
        grant_version: grant.version,
        approve,
    };
    let approver_context = f.current_scope(&approver).await;
    let alternate_context = f.current_scope(&alternate).await;
    let key_a = Uuid::new_v4().to_string();
    let key_b = Uuid::new_v4().to_string();
    let (a, b) = tokio::join!(
        execute(
            &approver,
            decide(approver_context.clone(), &approval, true),
            &key_a
        ),
        execute(
            &alternate,
            decide(alternate_context.clone(), &alternate_approval, true),
            &key_b
        )
    );
    let (winning_actor, winning_grant, winning_context, winning_key, receipt) = match (a, b) {
        (Ok(response), Err(error)) => {
            assert_eq!(error.code(), "version_conflict");
            (
                &approver,
                &approval,
                approver_context,
                key_a,
                response.receipt.unwrap(),
            )
        }
        (Err(error), Ok(response)) => {
            assert_eq!(error.code(), "version_conflict");
            (
                &alternate,
                &alternate_approval,
                alternate_context,
                key_b,
                response.receipt.unwrap(),
            )
        }
        _ => panic!("o pedido abaixo do limite deve ter uma única autorização final"),
    };
    assert_eq!(receipt.status, "approved");
    assert_eq!(
        execute(
            winning_actor,
            decide(winning_context.clone(), winning_grant, true),
            &winning_key
        )
        .await
        .unwrap()
        .receipt,
        Some(receipt.clone())
    );
    assert_eq!(
        refused(
            execute(
                winning_actor,
                decide(winning_context, winning_grant, false),
                &winning_key
            )
            .await
        )
        .code(),
        "idempotency_conflict"
    );
    assert_eq!(
        refused(
            execute(
                winning_actor,
                decide(f.current_scope(winning_actor).await, winning_grant, true),
                &Uuid::new_v4().to_string()
            )
            .await
        )
        .code(),
        "version_conflict"
    );

    let request_id = Uuid::parse_str(&first.request_id).unwrap();
    type HistoryRow = (String, Option<i64>, i64, Option<String>, String, Uuid);
    let history: Vec<HistoryRow> = sqlx::query_as(
        "SELECT h.event_kind,h.before_version,h.request_version,b.increase::text,
                n.increase::text,h.actor_id
         FROM canteiro.authority_history h JOIN canteiro.access_request_versions n
           ON (n.tenant_id,n.request_id,n.version)=(h.tenant_id,h.request_id,h.request_version)
         LEFT JOIN canteiro.access_request_versions b
           ON (b.tenant_id,b.request_id,b.version)=(h.tenant_id,h.request_id,h.before_version)
         WHERE h.request_id=$1 ORDER BY h.recorded_at,h.event_id",
    )
    .bind(request_id)
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(history.len(), 3);
    assert_eq!(
        history[0],
        (
            "proposed".into(),
            None,
            1,
            None,
            "100.00".into(),
            proposer.id
        )
    );
    assert_eq!(
        history[1],
        (
            "revised".into(),
            Some(1),
            2,
            Some("100.00".into()),
            "120.21".into(),
            proposer.id
        )
    );
    assert_eq!(
        history[2],
        (
            "decision".into(),
            Some(2),
            2,
            Some("120.21".into()),
            "120.21".into(),
            winning_actor.id
        )
    );
    let aggregate: (i64, String) = sqlx::query_as(
        "SELECT count(*),sum(v.increase-v.decrease)::text FROM canteiro.authority_history h
         JOIN canteiro.access_request_versions v ON (v.tenant_id,v.request_id,v.version)=(h.tenant_id,h.request_id,h.request_version)
         WHERE h.request_id=$1 AND h.event_kind='decision' AND h.after_status='approved'"
    ).bind(request_id).fetch_one(&f.admin).await.unwrap();
    assert_eq!(
        aggregate,
        (1, "100.01".into()),
        "efeito de autorização consolidado apenas uma vez"
    );
    let (other_actor, other_grant) = if winning_actor.id == approver.id {
        (&alternate, &alternate_approval)
    } else {
        (&approver, &approval)
    };
    let forged = sqlx::query(
        "INSERT INTO canteiro.authority_history
        (tenant_id,event_id,scope_id,request_id,request_version,actor_id,event_kind,
         before_version,before_status,before_valid_approvals,after_status,after_valid_approvals,grant_id,reason)
        VALUES($1,$2,$3,$4,2,$5,'decision',2,'pending',0,'pending',1,$6,'approval_recorded')"
    )
    .bind(Uuid::parse_str(&company).unwrap())
    .bind(Uuid::new_v4())
    .bind(Uuid::parse_str(&contract).unwrap())
    .bind(request_id)
    .bind(other_actor.id)
    .bind(Uuid::parse_str(&other_grant.id).unwrap())
    .execute(&f.admin)
    .await
    .unwrap_err();
    assert_eq!(
        forged
            .as_database_error()
            .and_then(|error| error.code())
            .as_deref(),
        Some("23503"),
        "a trilha não pode inventar a assinatura de uma pessoa com alçada"
    );

    f.access_command(
        &owner,
        AccessCommand::RevokeGrant {
            context: f.current_scope(&owner).await,
            grant_id: winning_grant.id.clone(),
            grant_version: winning_grant.version,
        },
    )
    .await
    .unwrap();
    let winner_scope = ReliabilityScope {
        organization_id: company.clone(),
        context_version: f.current_scope(winning_actor).await.context_version,
    };
    assert_eq!(
        refused(
            f.service
                .access_operation(
                    &winning_actor.token,
                    &winner_scope,
                    "authority_decide",
                    &winning_key
                )
                .await
        )
        .kind(),
        AuthErrorKind::Forbidden
    );
    let proposer_id = proposer.id;
    let member_version: i64 = sqlx::query_scalar(
        "SELECT version FROM canteiro.organization_members WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(Uuid::parse_str(&company).unwrap())
    .bind(proposer_id)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    f.execute(
        &owner,
        OrganizationCommand::RemoveMember {
            scope: f.current_scope(&owner).await,
            user_id: proposer_id.to_string(),
            member_version,
        },
    )
    .await
    .unwrap();
    assert!(
        f.service
            .access_operation(&proposer.token, &scope, "authority_propose", &proposal_key)
            .await
            .is_err()
    );
    let historical_actors: Vec<Uuid> =
        sqlx::query_scalar("SELECT actor_id FROM canteiro.authority_history WHERE request_id=$1")
            .bind(request_id)
            .fetch_all(&f.admin)
            .await
            .unwrap();
    assert_eq!(
        historical_actors
            .iter()
            .filter(|id| **id == proposer_id)
            .count(),
        2
    );

    let mut runtime = f.admin.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("SET LOCAL ROLE {}", f.runtime)))
        .execute(&mut *runtime)
        .await
        .unwrap();
    let error = sqlx::query("UPDATE canteiro.authority_history SET reason='request_revised'")
        .execute(&mut *runtime)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("42501")
    );
    runtime.rollback().await.unwrap();
    f.close().await;
}
