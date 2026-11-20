use canteiro_domain::{
    access::{
        AccessAction as Action, AccessError, AccessGrant, AccessRole as Role, AccessScope,
        ApprovalContext, ApprovalRule, MonetaryImpact, PermissionRequest, RecordedApproval,
        ScopeKind, approval_quorum, authorize, authorize_approval, validate_delegation,
        validate_grant,
    },
    decimal::Money,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

fn id(value: u128) -> Uuid {
    Uuid::from_u128(value)
}
fn money(value: &str) -> Money {
    value.parse().unwrap()
}
fn time(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}
fn now() -> DateTime<Utc> {
    time("2026-06-01T12:00:00Z")
}
fn scope() -> AccessScope {
    AccessScope {
        tenant_id: id(1),
        kind: ScopeKind::Contract,
        id: id(10),
    }
}
fn grant(number: u128, user: u128, role: Role) -> AccessGrant {
    AccessGrant {
        id: id(number),
        user_id: id(user),
        scope: scope(),
        role,
        actions: Action::ALL
            .into_iter()
            .filter(|action| role.permits(*action) && action.applies_to(scope().kind))
            .collect(),
        valid_from: time("2026-01-01T00:00:00Z"),
        valid_until: Some(time("2027-01-01T00:00:00Z")),
        max_amount: money("1000.00"),
        revoked: false,
        source_grant_id: None,
    }
}
fn delegated(number: u128, user: u128, source: &AccessGrant) -> AccessGrant {
    AccessGrant {
        id: id(number),
        user_id: id(user),
        source_grant_id: Some(source.id),
        ..source.clone()
    }
}
fn request(grant: &AccessGrant, action: Action, amount: &str) -> PermissionRequest {
    PermissionRequest {
        actor_id: grant.user_id,
        scope: scope(),
        action,
        amount: money(amount),
        at: now(),
    }
}
fn context() -> ApprovalContext {
    ApprovalContext {
        proposal_id: id(9000),
        scope: scope(),
        action: Action::ApproveChange,
        version: 3,
        proposer_id: id(999),
        impact: MonetaryImpact::new(money("100.00"), money("100.00")).unwrap(),
        rule: ApprovalRule {
            threshold: money("100.00"),
            approvals_above_threshold: 2,
            segregate_proposer: true,
        },
        conflicted_users: vec![],
    }
}
fn signed(grant: &AccessGrant) -> RecordedApproval {
    RecordedApproval {
        proposal_id: context().proposal_id,
        actor_id: grant.user_id,
        grant_id: grant.id,
        version: context().version,
    }
}

#[test]
fn wire_names_remain_stable_and_distinguish_technical_and_financial_roles() {
    for role in Role::ALL {
        assert_eq!(serde_json::to_value(role).unwrap(), role.as_str());
        assert!(!role.label().is_empty());
    }
    for action in Action::ALL {
        assert_eq!(serde_json::to_value(action).unwrap(), action.as_str());
        assert!(!action.label().is_empty());
    }
    for kind in [
        ScopeKind::Work,
        ScopeKind::Contract,
        ScopeKind::Budget,
        ScopeKind::Project,
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), kind.as_str());
        assert!(!kind.label().is_empty());
    }
    assert_eq!(
        serde_json::to_string(&Role::TechnicalResponsible).unwrap(),
        "\"technical_responsible\""
    );
    assert!(serde_json::from_str::<Role>("\"administrator\"").is_err());
    assert!(!Role::Inspector.permits(Action::RecordPayment));
    assert!(!Role::Finance.permits(Action::RecordExecution));
    assert!(!Role::Finance.permits(Action::AuthorizePayment));
    assert!(!Role::Approver.permits(Action::RecordPayment));
    assert!(!Role::Manager.permits(Action::ApproveBudget));
    for role in [Role::Support, Role::Viewer, Role::Client] {
        for action in Action::ALL {
            assert_eq!(role.permits(action), action == Action::View);
        }
    }
}

#[test]
fn mixed_increase_and_reduction_preserve_authority_even_when_net_is_zero() {
    let mixed = MonetaryImpact::new(money("100.00"), money("100.00")).unwrap();
    assert_eq!(mixed.authority_amount(), money("100.00"));
    assert!(mixed.is_increase());
    let larger_reduction = MonetaryImpact::new(money("25.00"), money("150.00")).unwrap();
    assert_eq!(larger_reduction.authority_amount(), money("150.00"));
    assert!(larger_reduction.is_increase());
    let reduction = MonetaryImpact::new(money("0.00"), money("150.00")).unwrap();
    assert!(!reduction.is_increase());
    assert_eq!(reduction.authority_amount(), money("150.00"));
    let limit = money("999999999999999999.99");
    assert_eq!(
        MonetaryImpact::new(limit, limit)
            .unwrap()
            .authority_amount(),
        limit
    );
}

#[test]
fn negative_impacts_and_numeric_json_never_enter_the_monetary_policy() {
    assert_eq!(
        MonetaryImpact::new(money("-0.01"), money("0")),
        Err(AccessError::InvalidAmount)
    );
    assert_eq!(
        MonetaryImpact::new(money("0"), money("-0.01")),
        Err(AccessError::InvalidAmount)
    );
    for json in [
        r#"{"increase":"-1.00","decrease":"0.00"}"#,
        r#"{"increase":"0.00","decrease":"-1.00"}"#,
        r#"{"increase":100.00,"decrease":"0.00"}"#,
        r#"{"increase":"100.00","decrease":"0.00","net":"0.00"}"#,
    ] {
        assert!(serde_json::from_str::<MonetaryImpact>(json).is_err());
    }
    let mixed = context().impact;
    let serialized = serde_json::to_string(&mixed).unwrap();
    assert_eq!(serialized, r#"{"increase":"100.00","decrease":"100.00"}"#);
    assert_eq!(
        serde_json::from_str::<MonetaryImpact>(&serialized).unwrap(),
        mixed
    );
}

#[test]
fn threshold_is_inclusive_and_invalid_quorum_policies_fail_closed() {
    let rule = context().rule;
    for (increase, decrease, expected) in [
        ("99.99", "0", 1),
        ("100", "0", 2),
        ("100", "100", 2),
        ("0", "100", 2),
    ] {
        assert_eq!(
            rule.required_approvals(MonetaryImpact::new(money(increase), money(decrease)).unwrap())
                .unwrap(),
            expected
        );
    }
    for quantity in [0, 3, u8::MAX] {
        let invalid = ApprovalRule {
            approvals_above_threshold: quantity,
            ..rule
        };
        assert_eq!(
            invalid.required_approvals(context().impact),
            Err(AccessError::InvalidPolicy)
        );
    }
    let negative = ApprovalRule {
        threshold: money("-0.01"),
        ..rule
    };
    assert_eq!(negative.validate(), Err(AccessError::InvalidPolicy));
}

#[test]
fn read_only_role_cannot_mutate_even_with_a_forged_explicit_action() {
    let mut reader = grant(100, 30, Role::Viewer);
    assert!(authorize(&reader, None, &request(&reader, Action::View, "0")).is_ok());
    assert_eq!(
        authorize(&reader, None, &request(&reader, Action::EditBudget, "0")),
        Err(AccessError::ActionNotGranted)
    );
    reader.actions.push(Action::EditBudget);
    assert_eq!(
        authorize(&reader, None, &request(&reader, Action::EditBudget, "0")),
        Err(AccessError::InvalidGrant)
    );
    let mut approver = grant(101, 31, Role::Approver);
    approver.actions = vec![Action::View];
    assert_eq!(
        authorize(
            &approver,
            None,
            &request(&approver, Action::ApproveChange, "100")
        ),
        Err(AccessError::ActionNotGranted)
    );
}

#[test]
fn budget_review_is_an_explicit_grant_without_editing_or_approval_authority() {
    let mut reviewer = grant(100, 30, Role::Budgeter);
    reviewer.scope = AccessScope {
        kind: ScopeKind::Budget,
        id: id(11),
        ..scope()
    };
    reviewer.actions = vec![Action::View, Action::ReviewBudget];
    let review = PermissionRequest {
        scope: reviewer.scope,
        ..request(&reviewer, Action::ReviewBudget, "100")
    };
    assert!(authorize(&reviewer, None, &review).is_ok());
    assert!(!Action::ReviewBudget.is_approval());
    for action in [Action::EditBudget, Action::ApproveBudget] {
        assert_eq!(
            authorize(
                &reviewer,
                None,
                &PermissionRequest {
                    action,
                    ..review.clone()
                }
            ),
            Err(AccessError::ActionNotGranted)
        );
    }
    reviewer.actions = vec![Action::View];
    assert_eq!(
        authorize(&reviewer, None, &review),
        Err(AccessError::ActionNotGranted)
    );
}

#[test]
fn organization_work_contract_and_actor_all_require_exact_matches() {
    let inspector = grant(100, 30, Role::Inspector);
    let original = request(&inspector, Action::RecordExecution, "0");
    assert!(authorize(&inspector, None, &original).is_ok());
    for scope in [
        AccessScope {
            tenant_id: id(2),
            ..scope()
        },
        AccessScope {
            id: id(11),
            ..scope()
        },
        AccessScope {
            kind: ScopeKind::Work,
            ..scope()
        },
    ] {
        assert_eq!(
            authorize(
                &inspector,
                None,
                &PermissionRequest {
                    scope,
                    ..original.clone()
                }
            ),
            Err(AccessError::WrongScope)
        );
    }
    assert_eq!(
        authorize(
            &inspector,
            None,
            &PermissionRequest {
                actor_id: id(31),
                ..original
            }
        ),
        Err(AccessError::WrongIdentity)
    );
}

#[test]
fn a_budget_grant_cannot_edit_a_sibling_and_contract_access_is_not_inherited() {
    let mut budgeter = grant(100, 30, Role::Budgeter);
    let contract = budgeter.scope;
    let a = AccessScope {
        kind: ScopeKind::Budget,
        id: id(11),
        ..contract
    };
    let b = AccessScope { id: id(12), ..a };
    budgeter.scope = a;
    let edit = PermissionRequest {
        actor_id: budgeter.user_id,
        scope: a,
        action: Action::EditBudget,
        amount: money("100"),
        at: now(),
    };
    assert!(authorize(&budgeter, None, &edit).is_ok());
    assert_eq!(
        authorize(
            &budgeter,
            None,
            &PermissionRequest {
                scope: b,
                ..edit.clone()
            }
        ),
        Err(AccessError::WrongScope)
    );
    budgeter.scope = contract;
    for budget in [a, b] {
        assert_eq!(
            authorize(
                &budgeter,
                None,
                &PermissionRequest {
                    scope: budget,
                    ..edit.clone()
                }
            ),
            Err(AccessError::WrongScope)
        );
    }
}

#[test]
fn grants_have_inclusive_start_exclusive_end_and_immediate_revocation() {
    let mut approver = grant(100, 30, Role::Approver);
    let query = request(&approver, Action::ApproveChange, "1000.00");
    assert!(
        authorize(
            &approver,
            None,
            &PermissionRequest {
                at: approver.valid_from,
                ..query.clone()
            }
        )
        .is_ok()
    );
    assert_eq!(
        authorize(
            &approver,
            None,
            &PermissionRequest {
                at: approver.valid_from - chrono::Duration::seconds(1),
                ..query.clone()
            }
        ),
        Err(AccessError::NotEffective)
    );
    assert_eq!(
        authorize(
            &approver,
            None,
            &PermissionRequest {
                at: approver.valid_until.unwrap(),
                ..query.clone()
            }
        ),
        Err(AccessError::NotEffective)
    );
    assert_eq!(
        authorize(
            &approver,
            None,
            &request(&approver, Action::ApproveChange, "1000.01")
        ),
        Err(AccessError::ValueLimit)
    );
    approver.revoked = true;
    assert_eq!(
        authorize(&approver, None, &query),
        Err(AccessError::Revoked)
    );
}

#[test]
fn malformed_grants_and_permanent_support_cannot_be_used() {
    let original = grant(100, 30, Role::Approver);
    let mut invalid = original.clone();
    invalid.max_amount = money("-0.01");
    assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidAmount));
    let mut invalid = original.clone();
    invalid.valid_until = Some(invalid.valid_from);
    assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidPeriod));
    let mut invalid = original.clone();
    invalid.actions.clear();
    assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidGrant));
    let mut invalid = original.clone();
    invalid.actions.push(Action::View);
    assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidGrant));
    for bad_id in [Uuid::nil(), Uuid::max()] {
        let mut invalid = original.clone();
        invalid.scope.tenant_id = bad_id;
        assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidIdentity));
        let mut invalid = original.clone();
        invalid.scope.id = bad_id;
        assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidIdentity));
        let mut invalid = original.clone();
        invalid.user_id = bad_id;
        assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidIdentity));
    }
    let mut support = grant(200, 40, Role::Support);
    support.valid_until = None;
    assert_eq!(
        validate_grant(&support),
        Err(AccessError::SupportRequiresExpiry)
    );
}

#[test]
fn delegated_authority_is_bounded_by_act_amount_period_and_scope() {
    let mut source = grant(100, 30, Role::Approver);
    source.actions = vec![Action::ApproveChange];
    let delegated = delegated(101, 31, &source);
    assert!(validate_delegation(&delegated, &source, now()).is_ok());
    assert!(
        authorize(
            &delegated,
            Some(&source),
            &request(&delegated, Action::ApproveChange, "1000.00")
        )
        .is_ok()
    );
    let mut more = delegated.clone();
    more.max_amount = money("1000.01");
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut more = delegated.clone();
    more.actions.push(Action::ApproveBudget);
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut more = delegated.clone();
    more.scope.id = id(11);
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut more = delegated.clone();
    more.valid_from -= chrono::Duration::seconds(1);
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut more = delegated.clone();
    more.valid_until = more
        .valid_until
        .map(|date| date + chrono::Duration::seconds(1));
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut more = delegated.clone();
    more.valid_until = None;
    assert_eq!(
        validate_delegation(&more, &source, now()),
        Err(AccessError::DelegationExceedsAuthority)
    );
    let mut later = delegated;
    later.valid_from = time("2026-07-01T00:00:00Z");
    assert!(validate_delegation(&later, &source, now()).is_ok());
    assert_eq!(
        authorize(
            &later,
            Some(&source),
            &request(&later, Action::ApproveChange, "10")
        ),
        Err(AccessError::NotEffective)
    );
}

#[test]
fn delegation_needs_the_live_original_source_and_cannot_be_forwarded() {
    let mut source = grant(100, 30, Role::Approver);
    let delegated = delegated(101, 31, &source);
    let query = request(&delegated, Action::ApproveChange, "100");
    assert_eq!(
        authorize(&delegated, None, &query),
        Err(AccessError::MissingSource)
    );
    let unrelated = grant(999, 40, Role::Approver);
    assert_eq!(
        authorize(&delegated, Some(&unrelated), &query),
        Err(AccessError::InvalidDelegation)
    );
    source.revoked = true;
    assert_eq!(
        authorize(&delegated, Some(&source), &query),
        Err(AccessError::Revoked)
    );
    source.revoked = false;
    source.valid_until = Some(now());
    assert_eq!(
        authorize(&delegated, Some(&source), &query),
        Err(AccessError::NotEffective)
    );
    source.valid_until = Some(time("2027-01-01T00:00:00Z"));
    let forwarded = AccessGrant {
        id: id(102),
        user_id: id(32),
        source_grant_id: Some(delegated.id),
        ..delegated.clone()
    };
    assert_eq!(
        validate_delegation(&forwarded, &delegated, now()),
        Err(AccessError::InvalidDelegation)
    );
    let self_delegated = AccessGrant {
        user_id: source.user_id,
        ..delegated
    };
    assert_eq!(
        validate_delegation(&self_delegated, &source, now()),
        Err(AccessError::InvalidDelegation)
    );
}

#[test]
fn changing_the_source_limits_invalidates_a_previous_delegation() {
    let mut source = grant(100, 30, Role::Approver);
    let delegated = delegated(101, 31, &source);
    let query = request(&delegated, Action::ApproveChange, "100");
    source.max_amount = money("99.99");
    assert_eq!(
        authorize(&delegated, Some(&source), &query),
        Err(AccessError::DelegationExceedsAuthority)
    );
    source.max_amount = money("1000");
    source.actions = vec![Action::View, Action::ApproveBudget];
    assert_eq!(
        authorize(&delegated, Some(&source), &query),
        Err(AccessError::DelegationExceedsAuthority)
    );
}

#[test]
fn consultation_only_grants_never_create_access_for_another_person() {
    for role in [
        Role::Viewer,
        Role::Client,
        Role::Support,
        Role::Budgeter,
        Role::Approver,
    ] {
        let mut source = grant(100, 30, role);
        source.actions = vec![Action::View];
        let delegated = delegated(101, 31, &source);
        assert_eq!(
            validate_delegation(&delegated, &source, now()),
            Err(AccessError::ActionNotGranted)
        );
        assert_eq!(
            authorize(
                &delegated,
                Some(&source),
                &request(&delegated, Action::View, "0")
            ),
            Err(AccessError::ActionNotGranted)
        );
    }
    let source = grant(100, 30, Role::Budgeter);
    let mut delegated = delegated(101, 31, &source);
    delegated.actions = vec![Action::View];
    assert!(validate_delegation(&delegated, &source, now()).is_ok());
    assert!(
        authorize(
            &delegated,
            Some(&source),
            &request(&delegated, Action::View, "0")
        )
        .is_ok()
    );
}

#[test]
fn segregation_blocks_the_proposer_both_directly_and_through_delegation() {
    let mut ctx = context();
    let source = grant(100, 30, Role::Approver);
    let delegated = delegated(101, 31, &source);
    ctx.proposer_id = source.user_id;
    assert_eq!(
        authorize_approval(&ctx, &source, None, source.user_id, now()),
        Err(AccessError::ConflictOfInterest)
    );
    assert_eq!(
        authorize_approval(&ctx, &delegated, Some(&source), delegated.user_id, now()),
        Err(AccessError::ConflictOfInterest)
    );
    ctx.proposer_id = delegated.user_id;
    assert_eq!(
        authorize_approval(&ctx, &delegated, Some(&source), delegated.user_id, now()),
        Err(AccessError::ConflictOfInterest)
    );
    ctx.rule.segregate_proposer = false;
    assert!(authorize_approval(&ctx, &delegated, Some(&source), delegated.user_id, now()).is_ok());
    ctx.conflicted_users = vec![source.user_id];
    assert_eq!(
        authorize_approval(&ctx, &delegated, Some(&source), delegated.user_id, now()),
        Err(AccessError::ConflictOfInterest)
    );
}

#[test]
fn approvals_cannot_be_reused_for_another_version_or_proposal() {
    let a = grant(100, 30, Role::Approver);
    let b = grant(101, 31, Role::Approver);
    let mut stale = signed(&b);
    stale.version -= 1;
    let mut unrelated = signed(&b);
    unrelated.proposal_id = id(9001);
    let result = approval_quorum(
        &context(),
        &[signed(&a), stale, unrelated],
        &[a.clone(), b.clone()],
        now(),
    )
    .unwrap();
    assert_eq!(result.valid, 1);
    assert!(!result.approved);
    let result = approval_quorum(&context(), &[signed(&a), signed(&b)], &[a, b], now()).unwrap();
    assert!(result.approved);
    assert_eq!(result.valid, 2);
}

#[test]
fn two_grants_or_repeated_rows_from_one_person_remain_one_signature() {
    let a = grant(100, 30, Role::Approver);
    let b = grant(101, 30, Role::Approver);
    let approvals = [signed(&a), signed(&a), signed(&b)];
    let result = approval_quorum(&context(), &approvals, &[a, b], now()).unwrap();
    assert_eq!(result.valid, 1);
    assert!(!result.approved);
}

#[test]
fn original_holder_and_delegates_never_supply_two_independent_powers() {
    let source = grant(100, 30, Role::Approver);
    let other_source_same_person = grant(101, 30, Role::Approver);
    let first = delegated(110, 31, &source);
    let second = delegated(111, 32, &other_source_same_person);
    let approvals = [signed(&source), signed(&first), signed(&second)];
    let result = approval_quorum(
        &context(),
        &approvals,
        &[source, other_source_same_person, first, second],
        now(),
    )
    .unwrap();
    assert_eq!(result.valid, 1);
    assert!(!result.approved);
}

#[test]
fn quorum_finds_independent_pair_even_if_the_first_candidate_collides_with_both() {
    let root_a = grant(100, 30, Role::Approver);
    let root_b = grant(101, 40, Role::Approver);
    let root_c = grant(102, 50, Role::Approver);
    // {20,30} aparece primeiro, mas colide com {20,40} e {30,50}.
    // Os dois últimos formam o par independente que a escolha gulosa perderia.
    let crossed = delegated(110, 20, &root_a);
    let first_independent = delegated(111, 20, &root_b);
    let second_independent = delegated(112, 30, &root_c);
    let mut approvals = vec![
        signed(&crossed),
        signed(&first_independent),
        signed(&second_independent),
    ];
    let grants = [
        crossed,
        first_independent,
        second_independent,
        root_a,
        root_b,
        root_c,
    ];
    let first = approval_quorum(&context(), &approvals, &grants, now()).unwrap();
    approvals.reverse();
    let reversed = approval_quorum(&context(), &approvals, &grants, now()).unwrap();
    assert_eq!(first, reversed);
    assert_eq!(first.valid, 2);
    assert_eq!(first.approvers, vec![id(20), id(30)]);
    assert!(first.approved);
}

#[test]
fn quorum_uses_current_authority_instead_of_historic_approval_existence() {
    let a = grant(100, 30, Role::Approver);
    let mut b = grant(101, 31, Role::Approver);
    let approvals = [signed(&a), signed(&b)];
    assert!(
        approval_quorum(&context(), &approvals, &[a.clone(), b.clone()], now())
            .unwrap()
            .approved
    );
    b.revoked = true;
    assert!(
        !approval_quorum(&context(), &approvals, &[a.clone(), b.clone()], now())
            .unwrap()
            .approved
    );
    b.revoked = false;
    b.valid_until = Some(now());
    assert!(
        !approval_quorum(&context(), &approvals, &[a.clone(), b.clone()], now())
            .unwrap()
            .approved
    );
    b.valid_until = Some(time("2027-01-01T00:00:00Z"));
    b.max_amount = money("99.99");
    assert!(
        !approval_quorum(&context(), &approvals, &[a.clone(), b], now())
            .unwrap()
            .approved
    );
    assert!(
        !approval_quorum(&context(), &approvals, &[a], now())
            .unwrap()
            .approved
    );
}

#[test]
fn quorum_rechecks_the_delegator_and_requires_distinct_unconflicted_authorities() {
    let mut source = grant(100, 30, Role::Approver);
    let delegated = delegated(110, 31, &source);
    let other = grant(101, 40, Role::Approver);
    let approvals = [signed(&delegated), signed(&other)];
    let mut grants = vec![source.clone(), delegated, other];
    assert!(
        approval_quorum(&context(), &approvals, &grants, now())
            .unwrap()
            .approved
    );
    source.revoked = true;
    grants[0] = source;
    assert!(
        !approval_quorum(&context(), &approvals, &grants, now())
            .unwrap()
            .approved
    );
    grants[0].revoked = false;
    let mut ctx = context();
    ctx.conflicted_users = vec![id(30)];
    assert!(
        !approval_quorum(&ctx, &approvals, &grants, now())
            .unwrap()
            .approved
    );
}

#[test]
fn invalid_contexts_and_ambiguous_grant_identity_fail_closed() {
    let grant = grant(100, 30, Role::Approver);
    let mut ctx = context();
    ctx.version = 0;
    assert_eq!(
        approval_quorum(&ctx, &[], &[], now()),
        Err(AccessError::InvalidVersion)
    );
    ctx = context();
    ctx.action = Action::View;
    assert_eq!(
        approval_quorum(&ctx, &[], &[], now()),
        Err(AccessError::NotApproval)
    );
    ctx = context();
    ctx.proposal_id = Uuid::nil();
    assert_eq!(
        approval_quorum(&ctx, &[], &[], now()),
        Err(AccessError::InvalidIdentity)
    );
    assert_eq!(
        approval_quorum(
            &context(),
            &[signed(&grant)],
            &[grant.clone(), grant],
            now()
        ),
        Err(AccessError::DuplicateGrant)
    );
}
