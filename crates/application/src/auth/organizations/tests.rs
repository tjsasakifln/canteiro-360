use super::*;
use canteiro_persistence::{
    Database, DatabaseEnvironment, DatabaseSettings, TransactionLimits, migrate,
};
use sqlx::{
    ConnectOptions, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{str::FromStr, time::Duration as StdDuration};

#[path = "access_tests.rs"]
mod access_tests;
#[path = "documents_tests.rs"]
mod documents_tests;
#[path = "reliability_tests.rs"]
mod reliability_tests;

#[test]
fn names_codes_deadlines_and_spreadsheet_cells_are_bounded() {
    assert_eq!(name("  Empresa Aurora  ").unwrap(), "Empresa Aurora");
    for value in ["", "x", "A\nB", "A\u{0000}B"] {
        assert!(name(value).is_err());
    }
    assert!(name(&"á".repeat(161)).is_err());
    let tenant = Uuid::new_v4();
    let token = code(tenant);
    assert_eq!(parse_code(&token).unwrap().0, tenant);
    for value in [
        "",
        "abc",
        "./../../",
        "empresa.abc",
        "é.abc",
        "00000000-0000-0000-0000-000000000000.abc",
    ] {
        assert!(parse_code(value).is_err());
    }
    assert_ne!(
        crypto::hash("organization-access", &token),
        crypto::hash("organization-invitation", &token)
    );
    let now = DateTime::parse_from_rfc3339("2026-11-13T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    assert!(deadline(Some("2026-11-13T11:59:59Z"), OrganizationRole::Member, now).is_err());
    assert!(
        deadline(
            Some("2027-11-13T12:00:00Z"),
            OrganizationRole::Administrator,
            now
        )
        .is_err()
    );
    assert!(
        deadline(
            Some("2026-11-13T23:59:59-03:00"),
            OrganizationRole::Member,
            now
        )
        .is_ok()
    );
    assert_eq!(tsv("=unsafe@example.test"), "'=unsafe@example.test");
    assert_eq!(tsv("\t+function"), "' +function");
    assert_eq!(tsv("safe@example.test"), "safe@example.test");
}

#[test]
fn public_commands_reject_unknown_identity_or_scope_fields() {
    let injected = serde_json::json!({"action":"create","name":"Aurora","user_id":Uuid::new_v4()});
    assert!(serde_json::from_value::<OrganizationCommand>(injected).is_err());
    let injected = serde_json::json!({"action":"rename","name":"Aurora","scope":{"organization_id":Uuid::new_v4(),"context_version":1,"organization_version":1,"administrator":true}});
    assert!(serde_json::from_value::<OrganizationCommand>(injected).is_err());
    assert!(serde_json::from_value::<OrganizationRole>(serde_json::json!("owner")).is_err());
}

struct Fixture {
    bootstrap: PgPool,
    admin: PgPool,
    database: Database,
    service: AuthService,
    db: String,
    owner: String,
    runtime: String,
}
struct Person {
    id: Uuid,
    email: String,
    token: String,
}
impl Fixture {
    async fn new(connections: u32) -> Self {
        let url = std::env::var("CANTEIRO_TEST_DATABASE_URL")
            .expect("PostgreSQL local de teste obrigatório");
        let options = PgConnectOptions::from_str(&url)
            .unwrap()
            .disable_statement_logging();
        assert!(
            matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1")
                && options.get_socket().is_none()
        );
        assert!(options.get_database().is_some_and(|v| v.contains("test")));
        let bootstrap = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options.clone())
            .await
            .unwrap();
        let suffix = Uuid::new_v4().simple().to_string();
        let db = format!("org_test_{suffix}");
        let owner = format!("org_m_{suffix}");
        let runtime = format!("org_r_{suffix}");
        for role in [&owner, &runtime] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE ROLE {role} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION"))).execute(&bootstrap).await.unwrap();
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {db} OWNER {owner}"
        )))
        .execute(&bootstrap)
        .await
        .unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(options.clone().database(&db))
            .await
            .unwrap();
        let migrator = DatabaseSettings::from_external_url(
            options
                .clone()
                .database(&db)
                .username(&owner)
                .to_url_lossy()
                .as_str(),
        )
        .unwrap();
        let operational = DatabaseSettings::from_external_url(
            options
                .database(&db)
                .username(&runtime)
                .to_url_lossy()
                .as_str(),
        )
        .unwrap();
        migrate(&migrator, &operational, DatabaseEnvironment::Test)
            .await
            .unwrap();
        let limits = TransactionLimits::new(connections, 3000, 5000, 1500, 15000).unwrap();
        let database = Database::connect(&operational, DatabaseEnvironment::Test, limits)
            .await
            .unwrap();
        let service = AuthService::new(database.clone(), [73; 32]).await.unwrap();
        Self {
            bootstrap,
            admin,
            database,
            service,
            db,
            owner,
            runtime,
        }
    }
    async fn close(self) {
        self.database.close().await;
        self.admin.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE {} WITH(FORCE)",
            self.db
        )))
        .execute(&self.bootstrap)
        .await
        .unwrap();
        for role in [&self.runtime, &self.owner] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP ROLE {role}")))
                .execute(&self.bootstrap)
                .await
                .unwrap();
        }
        self.bootstrap.close().await;
    }
    /// Estado sintético posterior à autenticação para testar exclusivamente
    /// autorização/concorrência. A autenticação real é exercitada nos testes HTTP/e2e.
    async fn person(&self, tag: &str, mfa: bool) -> Person {
        let id = Uuid::new_v4();
        let email = format!("{tag}-{id}@example.test");
        let token = crypto::random_token();
        sqlx::query("INSERT INTO canteiro_auth.users(id,email,password_hash,verified,mfa_secret) VALUES($1,$2,'authorization-fixture-only',true,$3)")
            .bind(id).bind(&email).bind(if mfa{Some(vec![1u8])}else{None}).execute(&self.admin).await.unwrap();
        sqlx::query("INSERT INTO canteiro_auth.sessions(id,token_hash,user_id,password_version,stage,mfa_verified,expires_at) VALUES($1,$2,$3,1,'authenticated',$4,clock_timestamp()+interval '8 hours')")
            .bind(Uuid::new_v4()).bind(crypto::hash("session",&token)).bind(id).bind(mfa).execute(&self.admin).await.unwrap();
        Person { id, email, token }
    }
    async fn execute(
        &self,
        p: &Person,
        c: OrganizationCommand,
    ) -> Result<OrganizationResponse, AuthError> {
        self.service.organizations_execute(&p.token, c).await
    }
    async fn context(&self, p: &Person) -> OrganizationResponse {
        self.service.organizations_context(&p.token).await.unwrap()
    }
    async fn create(&self, p: &Person, title: &str) -> (String, String) {
        let r = self
            .execute(p, OrganizationCommand::Create { name: title.into() })
            .await
            .unwrap();
        let id = r
            .state
            .organizations
            .iter()
            .find(|v| v.name == title)
            .unwrap()
            .id
            .clone();
        (id, r.access_code.unwrap())
    }
    async fn select(&self, p: &Person, id: &str) -> OrganizationScope {
        let c = self.context(p).await;
        let r = self
            .execute(
                p,
                OrganizationCommand::Select {
                    organization_id: id.into(),
                    expected_context_version: c.state.context_version,
                },
            )
            .await
            .unwrap();
        scope(&r)
    }
    async fn current_scope(&self, p: &Person) -> OrganizationScope {
        scope(&self.context(p).await)
    }
    async fn invite(&self, admin: &Person, target: &Person, role: OrganizationRole) -> String {
        self.execute(
            admin,
            OrganizationCommand::Invite {
                scope: self.current_scope(admin).await,
                email: target.email.clone(),
                role,
                valid_for_hours: 24,
                membership_valid_until: None,
            },
        )
        .await
        .unwrap()
        .invitation_code
        .unwrap()
    }
    async fn add(&self, admin: &Person, target: &Person, role: OrganizationRole) {
        let token = self.invite(admin, target, role).await;
        self.execute(target, OrganizationCommand::AcceptInvitation { token })
            .await
            .unwrap();
    }
}
fn scope(r: &OrganizationResponse) -> OrganizationScope {
    let a = &r.state.active.as_ref().unwrap().organization;
    OrganizationScope {
        organization_id: a.id.clone(),
        context_version: r.state.context_version,
        organization_version: a.version,
    }
}
fn error<T>(result: Result<T, AuthError>, code: &str) {
    match result {
        Err(e) => assert_eq!(e.code(), code),
        Ok(_) => panic!("unexpected successful authorization"),
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn organization_creation_mfa_single_connection_and_clear_selection() {
    let f = Fixture::new(1).await;
    let member = f.person("member", false).await;
    error(
        f.execute(
            &member,
            OrganizationCommand::Create {
                name: "Sem autorização".into(),
            },
        )
        .await,
        "mfa_required",
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.organizations")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let owner = f.person("owner", true).await;
    let (tenant, access) = tokio::time::timeout(
        StdDuration::from_secs(5),
        f.create(&owner, "Empresa Aurora"),
    )
    .await
    .unwrap();
    assert_eq!(parse_code(&access).unwrap().0.to_string(), tenant);
    assert!(f.context(&owner).await.state.active.is_none());
    let selected = f.select(&owner, &tenant).await;
    let r = f
        .execute(
            &owner,
            OrganizationCommand::ClearSelection {
                expected_context_version: selected.context_version,
            },
        )
        .await
        .unwrap();
    assert!(r.state.active.is_none());
    assert!(f.context(&owner).await.state.active.is_none());
    assert_eq!(r.state.context_version, selected.context_version + 1);
    let result = tokio::time::timeout(
        StdDuration::from_secs(5),
        f.execute(
            &member,
            OrganizationCommand::RequestAccess {
                code: access,
                requested_role: OrganizationRole::Member,
            },
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(result.state.organizations.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn organizations_ids_search_export_and_private_members_never_cross() {
    let f = Fixture::new(4).await;
    let a = f.person("admin-a", true).await;
    let b = f.person("admin-b", true).await;
    let m = f.person("ordinary", false).await;
    let (aid, _) = f.create(&a, "Empresa Aurora").await;
    let (bid, _) = f.create(&b, "Empresa Horizonte").await;
    f.select(&a, &aid).await;
    f.select(&b, &bid).await;
    let c = f.context(&a).await;
    assert_eq!(c.state.organizations.len(), 1);
    error(
        f.execute(
            &a,
            OrganizationCommand::Select {
                organization_id: bid.clone(),
                expected_context_version: c.state.context_version,
            },
        )
        .await,
        "organization_unavailable",
    );
    error(
        f.service
            .organizations_members(&a.token, &bid, c.state.context_version, "", 0, 50)
            .await,
        "context_changed",
    );
    error(
        f.service
            .organizations_export(&a.token, &bid, c.state.context_version, "")
            .await,
        "context_changed",
    );
    error(
        f.execute(
            &a,
            OrganizationCommand::RemoveMember {
                scope: scope(&c),
                user_id: b.id.to_string(),
                member_version: 1,
            },
        )
        .await,
        "organization_unavailable",
    );
    f.add(&a, &m, OrganizationRole::Member).await;
    let ms = f.select(&m, &aid).await;
    let mv = f.context(&m).await;
    let d = mv.state.active.unwrap();
    assert_eq!(d.members.len(), 1);
    assert_eq!(d.members[0].id, m.id.to_string());
    assert!(d.invitations.is_empty());
    error(
        f.service
            .organizations_members(&m.token, &aid, ms.context_version, "", 0, 50)
            .await,
        "mfa_required",
    );
    let sc = f.current_scope(&a).await;
    let page = f
        .service
        .organizations_members(&a.token, &aid, sc.context_version, &m.email, 0, 50)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.members[0].id, m.id.to_string());
    let export = f
        .service
        .organizations_export(&a.token, &aid, sc.context_version, "")
        .await
        .unwrap();
    assert!(export.contains("Empresa Aurora"));
    assert!(export.contains("Revisão\t"));
    assert!(export.contains("Total de membros\t2"));
    assert!(export.contains(&m.email));
    assert!(!export.contains(&b.email));
    assert!(!export.contains("Horizonte"));
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn invitations_bind_identity_expiry_revocation_purpose_and_single_use() {
    let f = Fixture::new(4).await;
    let a = f.person("admin", true).await;
    let m = f.person("member", false).await;
    let x = f.person("other", false).await;
    let (tenant, access) = f.create(&a, "Empresa Convites").await;
    f.select(&a, &tenant).await;
    let token = f.invite(&a, &m, OrganizationRole::Member).await;
    error(
        f.execute(
            &x,
            OrganizationCommand::PreviewInvitation {
                token: token.clone(),
            },
        )
        .await,
        "invitation_invalid",
    );
    error(
        f.execute(
            &x,
            OrganizationCommand::AcceptInvitation {
                token: token.clone(),
            },
        )
        .await,
        "invitation_invalid",
    );
    assert!(f.context(&x).await.state.organizations.is_empty());
    error(
        f.execute(&m, OrganizationCommand::AcceptInvitation { token: access })
            .await,
        "invitation_invalid",
    );
    let preview = f
        .execute(
            &m,
            OrganizationCommand::PreviewInvitation {
                token: token.clone(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        preview.invitation_preview.unwrap().organization_name,
        "Empresa Convites"
    );
    assert!(preview.state.organizations.is_empty());
    sqlx::query("UPDATE canteiro.organization_invites SET created_at=clock_timestamp()-interval '2 days',expires_at=clock_timestamp()-interval '1 second' WHERE token_hash=$1").bind(crypto::hash("organization-invitation",&token)).execute(&f.admin).await.unwrap();
    error(
        f.execute(&m, OrganizationCommand::AcceptInvitation { token })
            .await,
        "invitation_invalid",
    );
    let revoked = f.invite(&a, &m, OrganizationRole::Member).await;
    let ctx = f.context(&a).await;
    let inv = ctx
        .state
        .active
        .as_ref()
        .unwrap()
        .invitations
        .iter()
        .find(|i| i.status == "pending")
        .unwrap();
    f.execute(
        &a,
        OrganizationCommand::RevokeInvite {
            scope: scope(&ctx),
            invitation_id: inv.id.clone(),
            invitation_version: inv.version,
        },
    )
    .await
    .unwrap();
    error(
        f.execute(&m, OrganizationCommand::AcceptInvitation { token: revoked })
            .await,
        "invitation_invalid",
    );
    let once = f.invite(&a, &m, OrganizationRole::Member).await;
    f.execute(
        &m,
        OrganizationCommand::AcceptInvitation {
            token: once.clone(),
        },
    )
    .await
    .unwrap();
    error(
        f.execute(
            &m,
            OrganizationCommand::AcceptInvitation {
                token: once.clone(),
            },
        )
        .await,
        "invitation_invalid",
    );
    let ctx = f.context(&a).await;
    let member = ctx
        .state
        .active
        .as_ref()
        .unwrap()
        .members
        .iter()
        .find(|v| v.id == m.id.to_string())
        .unwrap();
    f.execute(
        &a,
        OrganizationCommand::RemoveMember {
            scope: scope(&ctx),
            user_id: m.id.to_string(),
            member_version: member.version,
        },
    )
    .await
    .unwrap();
    error(
        f.execute(&m, OrganizationCommand::AcceptInvitation { token: once })
            .await,
        "invitation_invalid",
    );
    assert!(f.context(&m).await.state.organizations.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn concurrent_invitation_acceptance_has_one_effect_and_revoke_is_serialized() {
    let f = Fixture::new(5).await;
    let a = f.person("admin", true).await;
    let m = f.person("member", false).await;
    let (tenant, _) = f.create(&a, "Empresa Concorrência").await;
    f.select(&a, &tenant).await;
    let token = f.invite(&a, &m, OrganizationRole::Member).await;
    let (first, second) = tokio::join!(
        f.execute(
            &m,
            OrganizationCommand::AcceptInvitation {
                token: token.clone()
            }
        ),
        f.execute(&m, OrganizationCommand::AcceptInvitation { token })
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.organization_members WHERE user_id=$1 AND active",
    )
    .bind(m.id)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(count, 1);
    let n = f.person("new-member", false).await;
    let code = f.invite(&a, &n, OrganizationRole::Member).await;
    let c = f.context(&a).await;
    let inv = c
        .state
        .active
        .as_ref()
        .unwrap()
        .invitations
        .iter()
        .find(|v| v.email == n.email && v.status == "pending")
        .unwrap();
    let (accepted, revoked) = tokio::join!(
        f.execute(&n, OrganizationCommand::AcceptInvitation { token: code }),
        f.execute(
            &a,
            OrganizationCommand::RevokeInvite {
                scope: scope(&c),
                invitation_id: inv.id.clone(),
                invitation_version: inv.version
            }
        )
    );
    assert_ne!(accepted.is_ok(), revoked.is_ok());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.organization_members WHERE user_id=$1 AND active",
    )
    .bind(n.id)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(count, i64::from(accepted.is_ok()));
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn last_administrator_survives_concurrent_removal_and_old_versions() {
    let f = Fixture::new(5).await;
    let a = f.person("admin-a", true).await;
    let b = f.person("admin-b", true).await;
    let (tenant, _) = f.create(&a, "Empresa Administração").await;
    f.select(&a, &tenant).await;
    let old = f.current_scope(&a).await;
    error(
        f.execute(
            &a,
            OrganizationCommand::RemoveMember {
                scope: old.clone(),
                user_id: a.id.to_string(),
                member_version: 1,
            },
        )
        .await,
        "last_administrator",
    );
    f.add(&a, &b, OrganizationRole::Administrator).await;
    f.select(&b, &tenant).await;
    error(
        f.execute(
            &a,
            OrganizationCommand::Rename {
                scope: old,
                name: "Nome antigo".into(),
            },
        )
        .await,
        "version_conflict",
    );
    let ca = f.context(&a).await;
    let cb = f.context(&b).await;
    let (ra, rb) = tokio::join!(
        f.execute(
            &a,
            OrganizationCommand::RemoveMember {
                scope: scope(&ca),
                user_id: b.id.to_string(),
                member_version: 1
            }
        ),
        f.execute(
            &b,
            OrganizationCommand::RemoveMember {
                scope: scope(&cb),
                user_id: a.id.to_string(),
                member_version: 1
            }
        )
    );
    assert_eq!(usize::from(ra.is_ok()) + usize::from(rb.is_ok()), 1);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_members WHERE tenant_id=$1 AND active AND role='administrator'").bind(id(&tenant).unwrap()).fetch_one(&f.admin).await.unwrap();
    assert_eq!(count, 1);
    let removed = if ra.is_ok() { &b } else { &a };
    let state = f.context(removed).await;
    assert!(state.state.active.is_none());
    assert!(state.state.organizations.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn membership_expiry_session_revocation_and_administrator_mfa_are_rechecked() {
    let f = Fixture::new(4).await;
    let a = f.person("admin", true).await;
    let m = f.person("member", false).await;
    let (tenant, _) = f.create(&a, "Empresa Validades").await;
    f.select(&a, &tenant).await;
    f.add(&a, &m, OrganizationRole::Member).await;
    let selected = f.select(&m, &tenant).await;
    let c = f.context(&a).await;
    // Confirmação antiga na sessão não substitui um segundo fator ainda ativo.
    sqlx::query("UPDATE canteiro_auth.users SET mfa_secret=NULL WHERE id=$1")
        .bind(a.id)
        .execute(&f.admin)
        .await
        .unwrap();
    let without_factor = f.context(&a).await;
    assert!(!without_factor.state.mfa_verified);
    assert_eq!(
        without_factor.state.active.as_ref().unwrap().members.len(),
        1
    );
    error(
        f.execute(
            &a,
            OrganizationCommand::Rename {
                scope: scope(&without_factor),
                name: "Não alterar".into(),
            },
        )
        .await,
        "mfa_required",
    );
    sqlx::query("UPDATE canteiro_auth.users SET mfa_secret=$2 WHERE id=$1")
        .bind(a.id)
        .bind(vec![1u8])
        .execute(&f.admin)
        .await
        .unwrap();
    error(
        f.execute(
            &a,
            OrganizationCommand::ChangeMember {
                scope: scope(&c),
                user_id: m.id.to_string(),
                member_version: 1,
                role: OrganizationRole::Administrator,
                valid_until: None,
            },
        )
        .await,
        "invalid_input",
    );
    sqlx::query("UPDATE canteiro.organization_members SET valid_until=clock_timestamp()-interval '1 second' WHERE tenant_id=$1 AND user_id=$2").bind(id(&tenant).unwrap()).bind(m.id).execute(&f.admin).await.unwrap();
    let expired = f.context(&m).await;
    assert!(expired.state.active.is_none());
    assert!(expired.state.organizations.is_empty());
    assert_eq!(expired.state.context_version, selected.context_version + 1);
    sqlx::query(
        "UPDATE canteiro_auth.sessions SET revoked_at=clock_timestamp() WHERE token_hash=$1",
    )
    .bind(crypto::hash("session", &a.token))
    .execute(&f.admin)
    .await
    .unwrap();
    error(
        f.service.organizations_context(&a.token).await,
        "session_expired",
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn access_requests_need_decision_and_rotation_invalidates_shared_code() {
    let f = Fixture::new(5).await;
    let a = f.person("admin", true).await;
    let m = f.person("requester", false).await;
    let n = f.person("denied", false).await;
    let (tenant, old_code) = f.create(&a, "Empresa Pedidos").await;
    f.select(&a, &tenant).await;
    let rotated = f
        .execute(
            &a,
            OrganizationCommand::RotateAccessCode {
                scope: f.current_scope(&a).await,
            },
        )
        .await
        .unwrap()
        .access_code
        .unwrap();
    error(
        f.execute(
            &m,
            OrganizationCommand::RequestAccess {
                code: old_code,
                requested_role: OrganizationRole::Member,
            },
        )
        .await,
        "organization_unavailable",
    );
    for _ in 0..2 {
        let r = f
            .execute(
                &m,
                OrganizationCommand::RequestAccess {
                    code: rotated.clone(),
                    requested_role: OrganizationRole::Member,
                },
            )
            .await
            .unwrap();
        assert!(r.state.organizations.is_empty());
        assert!(r.state.active.is_none());
    }
    let c = f.context(&a).await;
    let requests = &c.state.active.as_ref().unwrap().access_requests;
    assert_eq!(requests.len(), 1);
    let req = &requests[0];
    let approve = OrganizationCommand::DecideAccess {
        scope: scope(&c),
        request_id: req.id.clone(),
        request_version: req.version,
        approve: true,
    };
    // Aprovar uma identidade que também faz consulta não pode formar ciclo com a FK.
    let (approved, read) = tokio::join!(
        f.execute(&a, approve),
        f.service.organizations_context(&m.token)
    );
    assert!(approved.is_ok());
    assert!(read.is_ok());
    assert_eq!(f.context(&m).await.state.organizations.len(), 1);
    f.select(&m, &tenant).await;
    f.execute(
        &n,
        OrganizationCommand::RequestAccess {
            code: rotated,
            requested_role: OrganizationRole::Member,
        },
    )
    .await
    .unwrap();
    let c = f.context(&a).await;
    let req = c
        .state
        .active
        .as_ref()
        .unwrap()
        .access_requests
        .iter()
        .find(|v| v.email == n.email)
        .unwrap();
    error(
        f.execute(
            &m,
            OrganizationCommand::DecideAccess {
                scope: f.current_scope(&m).await,
                request_id: req.id.clone(),
                request_version: req.version,
                approve: true,
            },
        )
        .await,
        "mfa_required",
    );
    f.execute(
        &a,
        OrganizationCommand::DecideAccess {
            scope: scope(&c),
            request_id: req.id.clone(),
            request_version: req.version,
            approve: false,
        },
    )
    .await
    .unwrap();
    assert!(f.context(&n).await.state.organizations.is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn stale_selection_blocks_cross_tab_writes_and_foreign_locks_reveal_nothing() {
    let f = Fixture::new(5).await;
    let a = f.person("admin-a", true).await;
    let b = f.person("admin-b", true).await;
    let (aid, _) = f.create(&a, "Empresa A").await;
    let (other, _) = f.create(&a, "Empresa A2").await;
    let (bid, _) = f.create(&b, "Empresa B").await;
    let old = f.select(&a, &aid).await;
    f.select(&a, &other).await;
    error(
        f.execute(
            &a,
            OrganizationCommand::Rename {
                scope: old.clone(),
                name: "Empresa errada".into(),
            },
        )
        .await,
        "context_changed",
    );
    error(
        f.execute(
            &a,
            OrganizationCommand::Select {
                organization_id: aid.clone(),
                expected_context_version: old.context_version,
            },
        )
        .await,
        "context_changed",
    );
    let mut hold = f.admin.begin().await.unwrap();
    sqlx::query("SELECT tenant_id FROM canteiro.organizations WHERE tenant_id=$1 FOR UPDATE")
        .bind(id(&bid).unwrap())
        .fetch_one(&mut *hold)
        .await
        .unwrap();
    let current = f.current_scope(&a).await;
    let blocked = tokio::time::timeout(
        StdDuration::from_millis(750),
        f.execute(
            &a,
            OrganizationCommand::Select {
                organization_id: bid,
                expected_context_version: current.context_version,
            },
        ),
    )
    .await
    .expect("foreign organization must not be locked");
    error(blocked, "organization_unavailable");
    let unknown = f
        .execute(
            &a,
            OrganizationCommand::Select {
                organization_id: Uuid::new_v4().to_string(),
                expected_context_version: current.context_version,
            },
        )
        .await;
    error(unknown, "organization_unavailable");
    hold.rollback().await.unwrap();
    f.close().await;
}

#[path = "project_records_tests.rs"]
mod project_records_tests;
#[path = "registry_tests.rs"]
mod registry_tests;

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn invitation_mail_is_encrypted_retried_and_cancelled_on_replacement() {
    let f = Fixture::new(2).await;
    let admin = f.person("mail-admin", true).await;
    let recipient = f.person("mail-recipient", false).await;
    let (org, _) = f.create(&admin, "Convites por email").await;
    f.select(&admin, &org).await;
    let old = f.invite(&admin, &recipient, OrganizationRole::Member).await;
    let protected: bool = sqlx::query_scalar(
        "SELECT octet_length(payload)>0 AND position(convert_to($1,'UTF8') in payload)=0 AND position(convert_to($2,'UTF8') in payload)=0 FROM canteiro_auth.mail_outbox WHERE invitation_id IS NOT NULL",
    )
    .bind(&old)
    .bind(&recipient.email)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(protected);
    let mut first = f.service.pending_mail(1).await.unwrap();
    assert_eq!(first.len(), 1);
    let first = first.remove(0);
    assert_eq!(first.kind, "organization_invitation");
    assert_eq!(first.email, recipient.email);
    assert_eq!(first.token, old);
    let next = f.invite(&admin, &recipient, OrganizationRole::Member).await;
    let mut second = f.service.pending_mail(1).await.unwrap();
    assert_eq!(second.len(), 1);
    let second = second.remove(0);
    assert_eq!(second.token, next);
    let row: (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE discarded_at IS NOT NULL AND octet_length(payload)=0), count(*) FILTER (WHERE delivered_at IS NOT NULL) FROM canteiro_auth.mail_outbox WHERE invitation_id IS NOT NULL",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(row, (1, 0));
    sqlx::query("UPDATE canteiro_auth.mail_outbox SET lease_until=clock_timestamp()-interval '1 second' WHERE discarded_at IS NULL AND invitation_id IS NOT NULL")
        .execute(&f.admin).await.unwrap();
    let retry = f.service.pending_mail(1).await.unwrap().remove(0);
    assert_eq!(retry.token, next);
    assert_ne!(retry.id, second.id);
    // Recibos atrasados não podem confirmar um convite substituído ou uma reserva vencida.
    assert!(!f.service.mark_mail_delivered(&first.id).await.unwrap());
    assert!(!f.service.mark_mail_delivered(&second.id).await.unwrap());
    let premature: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro_auth.mail_outbox WHERE invitation_id IS NOT NULL AND delivered_at IS NOT NULL",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(premature, 0);
    assert!(f.service.mark_mail_delivered(&retry.id).await.unwrap());
    assert!(!f.service.mark_mail_delivered(&retry.id).await.unwrap());
    let delivered: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro_auth.mail_outbox WHERE invitation_id IS NOT NULL AND delivered_at IS NOT NULL AND attempts=2 AND octet_length(payload)=0")
        .fetch_one(&f.admin).await.unwrap();
    assert_eq!(delivered, 1);
    assert!(f.service.pending_mail(1).await.unwrap().is_empty());

    // Cada reserva e desfecho permanece disponível para auditoria sem o código ou e-mail.
    let history: Vec<(i32, String)> = sqlx::query_as(
        "SELECT h.attempt,h.event FROM canteiro_meta.invitation_mail_history h
         JOIN canteiro_auth.mail_outbox m ON m.id=h.mail_id
         WHERE m.delivered_at IS NOT NULL
         ORDER BY h.attempt,h.event",
    )
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        history,
        vec![
            (1, "claimed".to_owned()),
            (1, "unconfirmed".to_owned()),
            (2, "accepted_by_provider".to_owned()),
            (2, "claimed".to_owned()),
        ]
    );
    let cancelled: Vec<(i32, String)> = sqlx::query_as(
        "SELECT h.attempt,h.event FROM canteiro_meta.invitation_mail_history h
         JOIN canteiro_auth.mail_outbox m ON m.id=h.mail_id
         WHERE m.discarded_at IS NOT NULL
         ORDER BY h.attempt,h.event",
    )
    .fetch_all(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        cancelled,
        vec![(1, "claimed".to_owned()), (1, "discarded".to_owned())]
    );
    let accessible: bool = sqlx::query_scalar(
        "SELECT has_table_privilege($1::text,
         'canteiro_meta.invitation_mail_history',
         'SELECT,INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER')",
    )
    .bind(&f.runtime)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(!accessible);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn accepted_and_expired_invitation_mail_is_discarded_before_delivery() {
    let f = Fixture::new(2).await;
    let admin = f.person("mail-owner", true).await;
    let accepted = f.person("mail-accepted", false).await;
    let expired = f.person("mail-expired", false).await;
    let (org, _) = f.create(&admin, "Convites com cancelamento").await;
    f.select(&admin, &org).await;

    let token = f.invite(&admin, &accepted, OrganizationRole::Member).await;
    f.execute(&accepted, OrganizationCommand::AcceptInvitation { token })
        .await
        .unwrap();
    assert!(f.service.pending_mail(1).await.unwrap().is_empty());

    f.invite(&admin, &expired, OrganizationRole::Member).await;
    sqlx::query(
        "UPDATE canteiro_auth.mail_outbox
         SET invitation_expires_at=clock_timestamp()-interval '1 second'
         WHERE invitation_id IS NOT NULL AND discarded_at IS NULL",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    assert!(f.service.pending_mail(1).await.unwrap().is_empty());

    let discarded: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro_auth.mail_outbox
         WHERE invitation_id IS NOT NULL AND discarded_at IS NOT NULL
           AND delivered_at IS NULL AND attempts=0 AND octet_length(payload)=0",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(discarded, 2);

    let revoked = f.person("mail-revoked", false).await;
    let token = f.invite(&admin, &revoked, OrganizationRole::Member).await;
    let reserved = f.service.pending_mail(1).await.unwrap().remove(0);
    assert_eq!(reserved.token, token);
    let ctx = f.context(&admin).await;
    let invite = ctx
        .state
        .active
        .as_ref()
        .unwrap()
        .invitations
        .iter()
        .find(|i| i.email == revoked.email && i.status == "pending")
        .unwrap();
    f.execute(
        &admin,
        OrganizationCommand::RevokeInvite {
            scope: scope(&ctx),
            invitation_id: invite.id.clone(),
            invitation_version: invite.version,
        },
    )
    .await
    .unwrap();
    assert!(!f.service.mark_mail_delivered(&reserved.id).await.unwrap());
    let discarded: bool = sqlx::query_scalar(
        "SELECT discarded_at IS NOT NULL AND delivered_at IS NULL
            AND attempts=1 AND octet_length(payload)=0
         FROM canteiro_auth.mail_outbox WHERE invitation_id=$1",
    )
    .bind(id(&invite.id).unwrap())
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(discarded);
    assert!(f.service.pending_mail(1).await.unwrap().is_empty());
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real isolado"]
async fn invitation_retry_does_not_preempt_waiting_mail_and_respects_expiration() {
    let f = Fixture::new(2).await;
    let admin = f.person("queue-owner", true).await;
    let first = f.person("queue-first", false).await;
    let waiting = f.person("queue-waiting", false).await;
    let (org, _) = f.create(&admin, "Fila de convites").await;
    f.select(&admin, &org).await;
    let first_token = f.invite(&admin, &first, OrganizationRole::Member).await;
    let first_delivery = f.service.pending_mail(1).await.unwrap().remove(0);
    assert_eq!(first_delivery.token, first_token);
    let waiting_token = f.invite(&admin, &waiting, OrganizationRole::Member).await;

    // Torna a retentativa elegível depois do envio que ainda aguarda a primeira tentativa.
    sqlx::query(
        "UPDATE canteiro_auth.mail_outbox
         SET lease_until=clock_timestamp()-interval '1 second',
             created_at=clock_timestamp()-interval '20 seconds'
         WHERE invitation_id IS NOT NULL AND attempts=1",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE canteiro_auth.mail_outbox
         SET created_at=clock_timestamp()-interval '10 seconds'
         WHERE invitation_id IS NOT NULL AND attempts=0",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    let next = f.service.pending_mail(1).await.unwrap().remove(0);
    assert_eq!(next.token, waiting_token);
    assert!(f.service.mark_mail_delivered(&next.id).await.unwrap());

    sqlx::query(
        "UPDATE canteiro_auth.mail_outbox
         SET invitation_expires_at=clock_timestamp()+interval '30 seconds'
         WHERE invitation_id IS NOT NULL AND delivered_at IS NULL",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    let retry = f.service.pending_mail(1).await.unwrap().remove(0);
    assert_eq!(retry.token, first_token);
    let (bounded, attempts): (bool, i32) = sqlx::query_as(
        "SELECT lease_until<=invitation_expires_at, attempts
         FROM canteiro_auth.mail_outbox
         WHERE invitation_id IS NOT NULL AND delivered_at IS NULL",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(bounded);
    assert_eq!(attempts, 2);

    // Mesmo com reserva válida, a confirmação não reabre um convite expirado.
    sqlx::query(
        "UPDATE canteiro_auth.mail_outbox
         SET invitation_expires_at=clock_timestamp()-interval '1 second'
         WHERE invitation_id IS NOT NULL AND delivered_at IS NULL",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    assert!(!f.service.mark_mail_delivered(&retry.id).await.unwrap());
    assert!(f.service.pending_mail(1).await.unwrap().is_empty());
    let discarded: bool = sqlx::query_scalar(
        "SELECT discarded_at IS NOT NULL AND octet_length(payload)=0
         FROM canteiro_auth.mail_outbox
         WHERE invitation_id IS NOT NULL AND delivered_at IS NULL",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(discarded);
    f.close().await;
}
