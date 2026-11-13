//! Verificações independentes da aplicação: papel SQL real, RLS e concorrência.
use super::*;
use crate::auth::AuthStore;
use chrono::{Duration as ChronoDuration, Utc};

async fn seed_user(fixture: &Fixture, label: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro_auth.users(id,email,password_hash,verified) VALUES($1,$2,'fixture-only',true)")
        .bind(id).bind(format!("{label}-{id}@example.invalid"))
        .execute(&fixture.admin).await.unwrap();
    id
}

async fn seed_organization(fixture: &Fixture, owner: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash) VALUES($1,'Empresa fictícia com nome repetido',$2,$3)")
        .bind(id).bind(owner).bind(vec![7_u8;32]).execute(&fixture.admin).await.unwrap();
    sqlx::query("INSERT INTO canteiro.organization_members(tenant_id,user_id,role) VALUES($1,$2,'administrator')")
        .bind(id).bind(owner).execute(&fixture.admin).await.unwrap();
    id
}

async fn seed_session(fixture: &Fixture, user: Uuid, marker: u8) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro_auth.sessions(id,token_hash,user_id,password_version,stage,mfa_verified,expires_at) VALUES($1,$2,$3,1,'authenticated',true,clock_timestamp()+interval '1 hour')")
        .bind(id).bind(vec![marker;32]).bind(user).execute(&fixture.admin).await.unwrap();
    id
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_rls_hides_foreign_rows_ids_counts_and_identity_directory() {
    let fixture = Fixture::ready().await;
    let person_a = seed_user(&fixture, "alice").await;
    let person_b = seed_user(&fixture, "bruno").await;
    let organization_a = seed_organization(&fixture, person_a).await;
    let organization_b = seed_organization(&fixture, person_b).await;
    let repeated_id = Uuid::new_v4();
    for (organization, person) in [(organization_a, person_a), (organization_b, person_b)] {
        sqlx::query("INSERT INTO canteiro.organization_invites(tenant_id,id,email,role,token_hash,expires_at,created_by) VALUES($1,$2,'guest@example.invalid','member',$3,clock_timestamp()+interval '1 day',$4)")
            .bind(organization).bind(repeated_id).bind(vec![8_u8;32]).bind(person).execute(&fixture.admin).await.unwrap();
        sqlx::query("INSERT INTO canteiro.organization_access_requests(tenant_id,id,user_id,requested_role) VALUES($1,$2,$3,'member')")
            .bind(organization).bind(repeated_id).bind(person).execute(&fixture.admin).await.unwrap();
        sqlx::query("INSERT INTO canteiro.organization_audit(tenant_id,id,actor_id,event) VALUES($1,$2,$3,'fixture')")
            .bind(organization).bind(repeated_id).bind(person).execute(&fixture.admin).await.unwrap();
    }
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut work = store.begin().await.unwrap();
    work.org_set_actor(person_a).await.unwrap();
    let own = work.org_own_memberships(person_a).await.unwrap();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].tenant_id, organization_a);
    assert!(work.org_own_memberships(person_b).await.unwrap().is_empty());
    assert!(work.org_get(organization_b, false).await.unwrap().is_none());
    work.org_set_tenant(organization_a).await.unwrap();
    assert_eq!(work.org_member_count(organization_a, "").await.unwrap(), 1);
    assert_eq!(work.org_member_count(organization_b, "").await.unwrap(), 0);
    assert!(
        work.org_member(organization_b, person_b)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        work.org_members(organization_b, "", 50, 0)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        work.org_invite(organization_b, repeated_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        work.org_invite_by_hash(organization_b, &[8; 32])
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        work.org_request(organization_b, repeated_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(work.org_requests(organization_b).await.unwrap().is_empty());
    assert!(
        !work
            .org_invite_revoke(organization_b, repeated_id, 1)
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_request_decide(organization_b, repeated_id, 1, "denied", person_a)
            .await
            .unwrap()
    );
    for table in [
        "organization_members",
        "organization_invites",
        "organization_access_requests",
        "organization_audit",
    ] {
        let query = format!("SELECT count(*) FROM canteiro.{table}");
        let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_one(&mut *work.tx)
            .await
            .unwrap();
        assert_eq!(
            count, 1,
            "a proteção deve existir no banco, inclusive sem WHERE: {table}"
        );
    }
    let error=sqlx::query("INSERT INTO canteiro.organization_audit(tenant_id,id,actor_id,event) VALUES($1,$2,$3,'forbidden')")
        .bind(organization_b).bind(Uuid::new_v4()).bind(person_a).execute(&mut *work.tx).await.unwrap_err();
    assert_eq!(sql_state(&error), "42501");
    drop(work);
    assert_clean_pool(&database).await;
    let actual: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_audit")
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
    assert_eq!(actual, 2);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_selection_is_session_owned_versioned_and_pool_context_is_local() {
    let fixture = Fixture::ready().await;
    let person_a = seed_user(&fixture, "alice").await;
    let person_b = seed_user(&fixture, "bruno").await;
    let organization_a = seed_organization(&fixture, person_a).await;
    let organization_b = seed_organization(&fixture, person_b).await;
    let session_a = seed_session(&fixture, person_a, 11).await;
    let session_b = seed_session(&fixture, person_b, 12).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let before_pid = assert_clean_pool(&database).await;
    let mut work = store.begin().await.unwrap();
    work.org_set_actor(person_a).await.unwrap();
    work.org_set_tenant(organization_a).await.unwrap();
    let first = work.org_selection(session_a).await.unwrap();
    assert_eq!(first.generation, 1);
    assert!(first.tenant_id.is_none());
    assert!(
        work.org_select(session_a, Some(organization_a), 1)
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_select(session_a, Some(organization_b), 1)
            .await
            .unwrap()
    );
    work.commit().await.unwrap();
    assert_eq!(before_pid, assert_clean_pool(&database).await);
    let mut rejected = store.begin().await.unwrap();
    rejected.org_set_actor(person_a).await.unwrap();
    assert!(rejected.org_selection(session_b).await.is_err());
    drop(rejected);
    assert_eq!(before_pid, assert_clean_pool(&database).await);
    let mut next = store.begin().await.unwrap();
    next.org_set_actor(person_b).await.unwrap();
    let own = next.org_own_memberships(person_b).await.unwrap();
    assert_eq!(own.len(), 1);
    assert_eq!(own[0].tenant_id, organization_b);
    let selection = next.org_selection(session_b).await.unwrap();
    assert_eq!(selection.generation, 1);
    assert!(selection.tenant_id.is_none());
    next.org_set_tenant(organization_b).await.unwrap();
    assert!(next.org_get(organization_a, false).await.unwrap().is_none());
    drop(next);
    assert_eq!(before_pid, assert_clean_pool(&database).await);
    let selection:(Uuid,i64)=sqlx::query_as("SELECT tenant_id,generation FROM canteiro_auth.organization_selections WHERE session_id=$1")
        .bind(session_a).fetch_one(&fixture.admin).await.unwrap();
    assert_eq!(selection, (organization_a, 2));
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_creation_and_audit_failure_roll_back_without_orphan_or_secret_error() {
    let fixture = Fixture::ready().await;
    let person = seed_user(&fixture, "alice").await;
    let organization = Uuid::new_v4();
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut work = store.begin().await.unwrap();
    work.org_set_actor(person).await.unwrap();
    work.org_set_tenant(organization).await.unwrap();
    work.org_create(organization, "Empresa fictícia", person, &[9; 32])
        .await
        .unwrap();
    assert!(
        work.org_put_member(organization, person, "administrator", true, None, None)
            .await
            .unwrap()
    );
    let marker = "fixture-sensitive-marker".repeat(30);
    let error = work
        .org_audit(organization, person, "created", None, &marker)
        .await
        .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("fixture-sensitive-marker"));
    assert!(!format!("{error:?} {error}").contains("organization_audit"));
    drop(work);
    assert_clean_pool(&database).await;
    for query in [
        "SELECT count(*) FROM canteiro.organizations",
        "SELECT count(*) FROM canteiro.organization_members",
        "SELECT count(*) FROM canteiro.organization_audit",
    ] {
        let count: i64 = sqlx::query_scalar(query)
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_invites_and_requests_are_versioned_one_use_and_revocable() {
    let fixture = Fixture::ready().await;
    let person = seed_user(&fixture, "alice").await;
    let guest = seed_user(&fixture, "guest").await;
    let organization = seed_organization(&fixture, person).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    let mut work = store.begin().await.unwrap();
    work.org_set_actor(person).await.unwrap();
    work.org_set_tenant(organization).await.unwrap();
    let first = Uuid::new_v4();
    work.org_invite_create(
        organization,
        first,
        "guest@example.invalid",
        "member",
        &[20; 32],
        Utc::now() + ChronoDuration::days(1),
        None,
        person,
    )
    .await
    .unwrap();
    assert!(
        !work
            .org_invite_consume(organization, first, 99)
            .await
            .unwrap()
    );
    assert!(
        work.org_invite_consume(organization, first, 1)
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_invite_consume(organization, first, 2)
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_invite_revoke(organization, first, 2)
            .await
            .unwrap()
    );
    let second = Uuid::new_v4();
    work.org_invite_create(
        organization,
        second,
        "guest@example.invalid",
        "member",
        &[21; 32],
        Utc::now() + ChronoDuration::days(1),
        None,
        person,
    )
    .await
    .unwrap();
    work.org_revoke_pending_invites(organization, "guest@example.invalid")
        .await
        .unwrap();
    assert!(
        !work
            .org_invite_consume(organization, second, 2)
            .await
            .unwrap()
    );
    let expired = Uuid::new_v4();
    work.org_invite_create(
        organization,
        expired,
        "other@example.invalid",
        "member",
        &[22; 32],
        Utc::now() + ChronoDuration::days(1),
        Some(Utc::now() - ChronoDuration::seconds(1)),
        person,
    )
    .await
    .unwrap();
    assert!(
        !work
            .org_invite_consume(organization, expired, 1)
            .await
            .unwrap()
    );
    let request = Uuid::new_v4();
    assert!(
        work.org_request_create(organization, request, guest, "member")
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_request_create(organization, Uuid::new_v4(), guest, "administrator")
            .await
            .unwrap()
    );
    assert!(
        !work
            .org_request_decide(organization, request, 2, "approved", person)
            .await
            .unwrap()
    );
    work.org_deny_pending_requests(organization, guest, person)
        .await
        .unwrap();
    assert_eq!(
        work.org_request(organization, request)
            .await
            .unwrap()
            .unwrap()
            .status,
        "denied"
    );
    assert!(
        !work
            .org_request_decide(organization, request, 2, "approved", person)
            .await
            .unwrap()
    );
    assert!(
        work.org_request_create(organization, Uuid::new_v4(), guest, "member")
            .await
            .unwrap()
    );
    work.commit().await.unwrap();
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_database_prevents_last_administrator_and_expiring_administrator() {
    let fixture = Fixture::ready().await;
    let first = seed_user(&fixture, "alice").await;
    let second = seed_user(&fixture, "bruno").await;
    let organization = seed_organization(&fixture, first).await;
    let database = fixture.database(limits()).await;
    let store = AuthStore::new(database.clone());
    for query in [
        "DELETE FROM canteiro.organization_members WHERE tenant_id=$1 AND user_id=$2",
        "UPDATE canteiro.organization_members SET active=false WHERE tenant_id=$1 AND user_id=$2",
        "UPDATE canteiro.organization_members SET role='member' WHERE tenant_id=$1 AND user_id=$2",
        "UPDATE canteiro.organization_members SET valid_until=clock_timestamp()+interval '1 day' WHERE tenant_id=$1 AND user_id=$2",
    ] {
        let mut work = store.begin().await.unwrap();
        work.org_set_actor(first).await.unwrap();
        work.org_set_tenant(organization).await.unwrap();
        let error = sqlx::query(query)
            .bind(organization)
            .bind(first)
            .execute(&mut *work.tx)
            .await
            .unwrap_err();
        assert_eq!(sql_state(&error), "23514");
        drop(work);
        assert_clean_pool(&database).await;
    }
    let mut identity_change = store.begin().await.unwrap();
    identity_change.org_set_actor(first).await.unwrap();
    identity_change.org_set_tenant(organization).await.unwrap();
    let error = sqlx::query(
        "UPDATE canteiro.organization_members SET user_id=$3 WHERE tenant_id=$1 AND user_id=$2",
    )
    .bind(organization)
    .bind(first)
    .bind(second)
    .execute(&mut *identity_change.tx)
    .await
    .unwrap_err();
    assert_eq!(sql_state(&error), "23514");
    drop(identity_change);
    assert_clean_pool(&database).await;
    let mut work = store.begin().await.unwrap();
    work.org_set_actor(first).await.unwrap();
    work.org_set_tenant(organization).await.unwrap();
    assert!(
        work.org_put_member(organization, second, "administrator", true, None, None)
            .await
            .unwrap()
    );
    assert!(
        work.org_put_member(organization, first, "member", true, None, Some(1))
            .await
            .unwrap()
    );
    assert_eq!(work.org_admin_count(organization).await.unwrap(), 1);
    work.commit().await.unwrap();
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_last_admin_survives_two_connections_and_repeatable_read_snapshot() {
    let fixture = Fixture::ready().await;
    let first = seed_user(&fixture, "alice").await;
    let second = seed_user(&fixture, "bruno").await;
    let database = fixture
        .database(TransactionLimits::new(2, 2_000, 2_000, 1_000, 4_000).unwrap())
        .await;
    let store = AuthStore::new(database.clone());
    for (isolation, expected_state) in [
        ("SET TRANSACTION ISOLATION LEVEL READ COMMITTED", "23514"),
        ("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ", "40001"),
    ] {
        let organization = seed_organization(&fixture, first).await;
        sqlx::query("INSERT INTO canteiro.organization_members(tenant_id,user_id,role) VALUES($1,$2,'administrator')")
            .bind(organization).bind(second).execute(&fixture.admin).await.unwrap();
        let mut one = store.begin().await.unwrap();
        sqlx::raw_sql(isolation)
            .execute(&mut *one.tx)
            .await
            .unwrap();
        one.org_set_actor(first).await.unwrap();
        one.org_set_tenant(organization).await.unwrap();
        let mut two = store.begin().await.unwrap();
        sqlx::raw_sql(isolation)
            .execute(&mut *two.tx)
            .await
            .unwrap();
        two.org_set_actor(second).await.unwrap();
        two.org_set_tenant(organization).await.unwrap();
        assert_eq!(one.org_admin_count(organization).await.unwrap(), 2);
        assert_eq!(two.org_admin_count(organization).await.unwrap(), 2);
        let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *two.tx)
            .await
            .unwrap();
        sqlx::query("UPDATE canteiro.organization_members SET role='member' WHERE tenant_id=$1 AND user_id=$2")
            .bind(organization).bind(first).execute(&mut *one.tx).await.unwrap();
        let concurrent = tokio::spawn(async move {
            match sqlx::query(
                "DELETE FROM canteiro.organization_members WHERE tenant_id=$1 AND user_id=$2",
            )
            .bind(organization)
            .bind(second)
            .execute(&mut *two.tx)
            .await
            {
                Ok(_) => {
                    two.commit().await.unwrap();
                    None
                }
                Err(error) => Some(sql_state(&error)),
            }
        });
        timeout(Duration::from_millis(700),async {
            loop {
                let waiting:bool=sqlx::query_scalar("SELECT coalesce(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1")
                    .bind(second_pid).fetch_one(&fixture.admin).await.unwrap();
                if waiting {break;}
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("a segunda conexão precisa disputar a organização antes do commit");
        one.commit().await.unwrap();
        assert_eq!(concurrent.await.unwrap().as_deref(), Some(expected_state));
        let remaining:i64=sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_members WHERE tenant_id=$1 AND active AND role='administrator'")
            .bind(organization).fetch_one(&fixture.admin).await.unwrap();
        assert_eq!(
            remaining, 1,
            "a organização não pode ficar sem administrador"
        );
    }
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_runtime_rejects_audit_column_privilege_truncate_and_disabled_selection_rls() {
    let fixture = Fixture::ready().await;
    let short = TransactionLimits::new(1, 120, 150, 50, 300).unwrap();
    let role = quoted_identifier(&fixture.runtime_role);
    let changes = [
        (
            format!("GRANT UPDATE(detail) ON canteiro.organization_audit TO {role}"),
            format!("REVOKE UPDATE(detail) ON canteiro.organization_audit FROM {role}"),
        ),
        (
            format!("GRANT DELETE ON canteiro.organization_audit TO {role}"),
            format!("REVOKE DELETE ON canteiro.organization_audit FROM {role}"),
        ),
        (
            format!("GRANT TRUNCATE ON canteiro.organization_invites TO {role}"),
            format!("REVOKE TRUNCATE ON canteiro.organization_invites FROM {role}"),
        ),
        (
            "ALTER TABLE canteiro_auth.organization_selections DISABLE ROW LEVEL SECURITY"
                .to_string(),
            "ALTER TABLE canteiro_auth.organization_selections ENABLE ROW LEVEL SECURITY"
                .to_string(),
        ),
    ];
    for (grant, revoke) in changes {
        sqlx::raw_sql(sqlx::AssertSqlSafe(grant))
            .execute(&fixture.admin)
            .await
            .unwrap();
        assert!(
            Database::connect(&fixture.runtime, DatabaseEnvironment::Test, short)
                .await
                .is_err()
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(revoke))
            .execute(&fixture.admin)
            .await
            .unwrap();
        let database = fixture.database(limits()).await;
        assert_clean_pool(&database).await;
        database.close().await;
    }
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn organization_membership_fk_does_not_deadlock_two_authenticated_actors() {
    let fixture = Fixture::ready().await;
    let first = seed_user(&fixture, "alice").await;
    let second = seed_user(&fixture, "bruno").await;
    let organization = seed_organization(&fixture, first).await;
    let database = fixture
        .database(TransactionLimits::new(2, 2_000, 2_000, 1_000, 4_000).unwrap())
        .await;
    let store = AuthStore::new(database.clone());
    let mut one = store.begin().await.unwrap();
    let mut two = store.begin().await.unwrap();
    one.user(first).await.unwrap().unwrap();
    two.user(second).await.unwrap().unwrap();
    one.org_set_actor(first).await.unwrap();
    two.org_set_actor(second).await.unwrap();
    one.org_set_tenant(organization).await.unwrap();
    two.org_set_tenant(organization).await.unwrap();
    one.org_get(organization, true).await.unwrap().unwrap();
    // Prazo apenas deste ensaio: precisamos observar a disputa antes da inserção.
    sqlx::raw_sql("SET LOCAL lock_timeout='3s'")
        .execute(&mut *two.tx)
        .await
        .unwrap();
    let second_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *two.tx)
        .await
        .unwrap();
    let competing = tokio::spawn(async move {
        two.org_get(organization, true).await.unwrap().unwrap();
        two.commit().await.unwrap();
    });
    timeout(Duration::from_millis(700), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT coalesce(wait_event_type='Lock',false) FROM pg_stat_activity WHERE pid=$1",
            )
            .bind(second_pid)
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a segunda pessoa deve estar esperando a organização");
    let inserted = timeout(
        Duration::from_millis(500),
        one.org_put_member(organization, second, "member", true, None, None),
    )
    .await
    .expect("a FK não pode aguardar o bloqueio de credenciais da segunda pessoa")
    .unwrap();
    assert!(inserted);
    one.commit().await.unwrap();
    competing.await.unwrap();
    let members: i64 =
        sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_members WHERE tenant_id=$1")
            .bind(organization)
            .fetch_one(&fixture.admin)
            .await
            .unwrap();
    assert_eq!(members, 2);
    database.close().await;
    fixture.cleanup().await;
}
