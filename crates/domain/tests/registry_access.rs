//! Fronteiras dos atos introduzidos pelos cadastros, projetos e obrigações.
use canteiro_domain::access::{
    AccessAction as A, AccessError, AccessGrant, AccessRole as R, AccessScope, PermissionRequest,
    ScopeKind as K, authorize, validate_grant,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    "2026-11-19T12:00:00Z".parse().unwrap()
}

fn grant(kind: K, action: A) -> AccessGrant {
    AccessGrant {
        id: Uuid::from_u128(11),
        user_id: Uuid::from_u128(12),
        scope: AccessScope {
            tenant_id: Uuid::from_u128(13),
            id: Uuid::from_u128(14),
            kind,
        },
        role: R::Manager,
        actions: vec![A::View, action],
        max_amount: "0".parse().unwrap(),
        valid_from: "2026-01-01T00:00:00Z".parse().unwrap(),
        valid_until: None,
        revoked: false,
        source_grant_id: None,
    }
}

#[test]
fn work_and_project_management_cannot_be_granted_to_the_wrong_record_kind() {
    for (kind, action) in [(K::Work, A::ManagePortfolio), (K::Project, A::EditProject)] {
        let correct = grant(kind, action);
        assert!(validate_grant(&correct).is_ok());
        for wrong in [K::Work, K::Contract, K::Budget, K::Project]
            .into_iter()
            .filter(|k| *k != kind)
        {
            let mut invalid = correct.clone();
            invalid.scope.kind = wrong;
            assert_eq!(validate_grant(&invalid), Err(AccessError::InvalidGrant));
        }
    }
    for action in [A::ApproveProjectScope, A::ApproveTechnicalException] {
        assert!(!R::Manager.permits(action));
        assert!(!R::TechnicalResponsible.permits(action));
        assert!(R::Approver.permits(action));
    }
}

#[test]
fn an_explicit_project_grant_does_not_survive_revocation_or_cross_tenants() {
    let mut rights = grant(K::Project, A::EditProject);
    let mut request = PermissionRequest {
        actor_id: rights.user_id,
        scope: rights.scope,
        action: A::EditProject,
        amount: "0".parse().unwrap(),
        at: now(),
    };
    assert!(authorize(&rights, None, &request).is_ok());
    request.scope.tenant_id = Uuid::from_u128(99);
    assert_eq!(
        authorize(&rights, None, &request),
        Err(AccessError::WrongScope)
    );
    request.scope = rights.scope;
    rights.revoked = true;
    assert_eq!(
        authorize(&rights, None, &request),
        Err(AccessError::Revoked)
    );
    rights.revoked = false;
    rights.valid_until = Some(now());
    assert_eq!(
        authorize(&rights, None, &request),
        Err(AccessError::NotEffective)
    );
}
