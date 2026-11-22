//! Issue #15: PostgreSQL real, fila global, isolamento, deduplicação e recuperação.
use super::*;
use crate::{
    auth::{AuthStore, AuthTransaction},
    notifications::NewNotice,
    reliability_outbox::NotificationStep,
};

struct NoticeFixture {
    tenant: Uuid,
    actor: Uuid,
    scope: Uuid,
    event: Uuid,
    target: Uuid,
}

async fn subject(fixture: &Fixture) -> NoticeFixture {
    let result = NoticeFixture {
        tenant: Uuid::new_v4(),
        actor: Uuid::new_v4(),
        scope: Uuid::new_v4(),
        event: Uuid::new_v4(),
        target: Uuid::new_v4(),
    };
    let mut tx = fixture.admin.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO canteiro_auth.users(id,email,password_hash,verified)
         VALUES($1,$2,'fixture-only',true)",
    )
    .bind(result.actor)
    .bind(format!("{}@example.invalid", result.actor))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash)
         VALUES($1,'Empresa fictícia',$2,$3)",
    )
    .bind(result.tenant)
    .bind(result.actor)
    .bind([17_u8; 32].as_slice())
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.organization_members(tenant_id,user_id,role)
         VALUES($1,$2,'administrator')",
    )
    .bind(result.tenant)
    .bind(result.actor)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by)
         VALUES($1,$2,'work','Obra fictícia',NULL,$3)",
    )
    .bind(result.tenant)
    .bind(result.scope)
    .bind(result.actor)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    result
}

async fn scoped(database: &Database, subject: &NoticeFixture) -> AuthTransaction {
    let mut tx = AuthStore::new(database.clone()).begin().await.unwrap();
    tx.org_set_actor(subject.actor).await.unwrap();
    tx.org_set_tenant(subject.tenant).await.unwrap();
    tx
}

async fn record(tx: &mut AuthTransaction, subject: &NoticeFixture) -> bool {
    tx.user_notice_record(&NewNotice {
        tenant: subject.tenant,
        recipient: subject.actor,
        event: subject.event,
        kind: "technical_obligation",
        scope: subject.scope,
        target: subject.target,
        due: None,
        essential: true,
    })
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn internal_notice_delivery_is_atomic_private_and_idempotent() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let first = subject(&fixture).await;
    let second = subject(&fixture).await;

    let mut rolled_back = scoped(&database, &first).await;
    assert!(record(&mut rolled_back, &first).await);
    assert_eq!(
        database.consume_user_notice_delivery().await.unwrap(),
        NotificationStep::Idle
    );
    rolled_back.tx.rollback().await.unwrap();

    let mut create = scoped(&database, &first).await;
    assert!(record(&mut create, &first).await);
    assert!(!record(&mut create, &first).await);
    create.commit().await.unwrap();
    let mut create = scoped(&database, &second).await;
    assert!(record(&mut create, &second).await);
    create.commit().await.unwrap();

    let (a, b) = tokio::join!(
        database.consume_user_notice_delivery(),
        database.consume_user_notice_delivery()
    );
    assert_eq!(a.unwrap(), NotificationStep::Delivered);
    assert_eq!(b.unwrap(), NotificationStep::Delivered);
    assert_eq!(
        database.consume_user_notice_delivery().await.unwrap(),
        NotificationStep::Idle
    );

    for s in [&first, &second] {
        let (state, attempts, receipt): (String, i32, String) = sqlx::query_as(
            "SELECT n.state, n.attempts, o.provider_receipt
             FROM canteiro.user_notices n
             JOIN canteiro.user_notice_outbox o USING(tenant_id,recipient_id,event_id,kind)
             WHERE n.tenant_id=$1 AND n.event_id=$2",
        )
        .bind(s.tenant)
        .bind(s.event)
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
        assert_eq!(
            (state.as_str(), attempts, receipt.as_str()),
            ("communicated", 1, "internal")
        );

        let history: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM canteiro.user_notice_delivery_history
             WHERE tenant_id=$1 AND event_id=$2 AND outcome='acknowledged'",
        )
        .bind(s.tenant)
        .bind(s.event)
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
        assert_eq!(history, 1);
    }

    // Simula a reentrega de uma chave já confirmada após resposta perdida.
    sqlx::query(
        "UPDATE canteiro_meta.user_notice_queue
         SET processed_at=NULL,ready_at=clock_timestamp()
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(first.tenant)
    .bind(first.event)
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        database.consume_user_notice_delivery().await.unwrap(),
        NotificationStep::Delivered
    );
    let history: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.user_notice_delivery_history
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(first.tenant)
    .bind(first.event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(history, 1);

    let mut read = scoped(&database, &first).await;
    assert_eq!(
        read.user_notice_list(first.tenant, first.actor, &[first.scope])
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        read.user_notice_list(second.tenant, second.actor, &[second.scope])
            .await
            .unwrap()
            .is_empty()
    );
    read.commit().await.unwrap();

    let no_global_queue_access: bool = sqlx::query_scalar(
        "SELECT NOT has_table_privilege(current_user,'canteiro_meta.user_notice_queue','SELECT')",
    )
    .fetch_one(&database.pool)
    .await
    .unwrap();
    assert!(no_global_queue_access);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn internal_notice_ack_failure_rolls_back_and_is_retried() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let s = subject(&fixture).await;
    let mut create = scoped(&database, &s).await;
    assert!(record(&mut create, &s).await);
    create.commit().await.unwrap();

    sqlx::raw_sql(
        "CREATE FUNCTION canteiro_meta.fail_notice_ack() RETURNS TRIGGER
         LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'Falha fictícia no recibo'; END $$;
         CREATE TRIGGER fail_notice_ack
         BEFORE INSERT ON canteiro.user_notice_delivery_history
         FOR EACH ROW EXECUTE FUNCTION canteiro_meta.fail_notice_ack()",
    )
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        database.consume_user_notice_delivery().await.unwrap(),
        NotificationStep::Deferred
    );
    let (state, attempts): (String, i32) = sqlx::query_as(
        "SELECT state,attempts FROM canteiro.user_notice_outbox
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(s.tenant)
    .bind(s.event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!((state.as_str(), attempts), ("queued", 0));
    let retry_count: i32 = sqlx::query_scalar(
        "SELECT attempts FROM canteiro_meta.user_notice_queue
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(s.tenant)
    .bind(s.event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(retry_count, 1);

    sqlx::raw_sql(
        "DROP TRIGGER fail_notice_ack ON canteiro.user_notice_delivery_history;
         DROP FUNCTION canteiro_meta.fail_notice_ack()",
    )
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE canteiro_meta.user_notice_queue SET ready_at=clock_timestamp()
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(s.tenant)
    .bind(s.event)
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        database.consume_user_notice_delivery().await.unwrap(),
        NotificationStep::Delivered
    );
    let history: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.user_notice_delivery_history
         WHERE tenant_id=$1 AND event_id=$2",
    )
    .bind(s.tenant)
    .bind(s.event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(history, 1);
    database.close().await;
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn repeated_notice_key_rejects_changed_scope_target_due_or_priority() {
    let fixture = Fixture::ready().await;
    let database = fixture.database(TransactionLimits::default()).await;
    let s = subject(&fixture).await;
    let mut tx = scoped(&database, &s).await;
    assert!(record(&mut tx, &s).await);

    let alternatives = [
        (Uuid::new_v4(), s.target, None, true),
        (s.scope, Uuid::new_v4(), None, true),
        (
            s.scope,
            s.target,
            Some(chrono::NaiveDate::from_ymd_opt(2027, 1, 1).unwrap()),
            true,
        ),
        (s.scope, s.target, None, false),
    ];
    for (scope, target, due, essential) in alternatives {
        assert!(
            tx.user_notice_record(&NewNotice {
                tenant: s.tenant,
                recipient: s.actor,
                event: s.event,
                kind: "technical_obligation",
                scope,
                target,
                due,
                essential,
            })
            .await
            .is_err(),
            "a mesma chave não pode ocultar conteúdo diferente"
        );
    }
    assert!(!record(&mut tx, &s).await);
    tx.commit().await.unwrap();

    let (scope, target, due, essential): (Uuid, Uuid, Option<chrono::NaiveDate>, bool) =
        sqlx::query_as(
            "SELECT scope_id,target_id,due_on,essential FROM canteiro.user_notices
         WHERE tenant_id=$1 AND recipient_id=$2 AND event_id=$3",
        )
        .bind(s.tenant)
        .bind(s.actor)
        .bind(s.event)
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
    assert_eq!(
        (scope, target, due, essential),
        (s.scope, s.target, None, true)
    );

    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canteiro.user_notice_outbox
         WHERE tenant_id=$1 AND recipient_id=$2 AND event_id=$3",
    )
    .bind(s.tenant)
    .bind(s.actor)
    .bind(s.event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(pending, 1);
    database.close().await;
    fixture.cleanup().await;
}
