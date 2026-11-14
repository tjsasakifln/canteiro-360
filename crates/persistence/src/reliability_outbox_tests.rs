//! PostgreSQL real: transações separadas, queda antes do ACK e isolamento.
use super::*;
use crate::{
    auth::{AuthStore, AuthTransaction},
    reliability_outbox::NotificationStep,
};

struct Contract {
    tenant: Uuid,
    author: Uuid,
    id: Uuid,
    session: Uuid,
    lease: Uuid,
}

async fn contract(fixture: &Fixture) -> Contract {
    let tenant = Uuid::new_v4();
    let author = Uuid::new_v4();
    let id = Uuid::new_v4();
    let session = Uuid::new_v4();
    let lease = Uuid::new_v4();
    let work = Uuid::new_v4();
    let mut setup = fixture.admin.begin().await.unwrap();
    sqlx::query("INSERT INTO canteiro_auth.users(id,email,password_hash,verified) VALUES($1,$2,'fixture-only',true)")
        .bind(author).bind(format!("{author}@example.invalid")).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash) VALUES($1,'Empresa fictícia',$2,$3)")
        .bind(tenant).bind(author).bind([17_u8; 32].as_slice()).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO canteiro.organization_members(tenant_id,user_id,role) VALUES($1,$2,'administrator')")
        .bind(tenant).bind(author).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES($1,$2,'work','Obra inicial',NULL,$5),($1,$3,'contract','Contrato inicial',$4,$5)")
        .bind(tenant).bind(work).bind(id).bind(work).bind(author).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'manager',ARRAY['view','edit_contract_record'],0,clock_timestamp(),$4)")
        .bind(tenant).bind(Uuid::new_v4()).bind(id).bind(author).execute(&mut *setup).await.unwrap();
    sqlx::query("INSERT INTO canteiro_auth.sessions(id,token_hash,user_id,password_version,stage,expires_at) SELECT $1,$2,id,password_version,'authenticated',clock_timestamp()+interval '1 hour' FROM canteiro_auth.users WHERE id=$3")
        .bind(session).bind([session.as_bytes().as_slice(), Uuid::new_v4().as_bytes().as_slice()].concat()).bind(author).execute(&mut *setup).await.unwrap();
    sqlx::query(
        "INSERT INTO canteiro_auth.organization_selections(session_id,tenant_id) VALUES($1,$2)",
    )
    .bind(session)
    .bind(tenant)
    .execute(&mut *setup)
    .await
    .unwrap();
    sqlx::query("INSERT INTO canteiro.contract_edit_leases(tenant_id,contract_id,actor_id,session_id,lease_id,expires_at) VALUES($1,$2,$3,$4,$5,clock_timestamp()+interval '5 minutes')")
        .bind(tenant).bind(id).bind(author).bind(session).bind(lease).execute(&mut *setup).await.unwrap();
    setup.commit().await.unwrap();
    Contract {
        tenant,
        author,
        id,
        session,
        lease,
    }
}

async fn scoped(database: &Database, tenant: Uuid, actor: Uuid) -> AuthTransaction {
    let mut tx = AuthStore::new(database.clone()).begin().await.unwrap();
    tx.org_set_actor(actor).await.unwrap();
    tx.org_set_tenant(tenant).await.unwrap();
    tx
}

async fn revise(tx: &mut AuthTransaction, c: &Contract) -> Uuid {
    sqlx::query_scalar(
        "SELECT canteiro.rename_contract_reference(
               $1,$2,$3,$4,$5,1,'Título revisado','Correção cadastral',NULL)",
    )
    .bind(c.tenant)
    .bind(c.id)
    .bind(c.author)
    .bind(c.session)
    .bind(c.lease)
    .fetch_one(&mut *tx.tx)
    .await
    .unwrap()
}

async fn notices(database: &Database, c: &Contract, actor: Uuid) -> Vec<Uuid> {
    let mut tx = scoped(database, c.tenant, actor).await;
    let notices = tx
        .contract_notifications(c.tenant, actor, c.id)
        .await
        .unwrap();
    for notice in &notices {
        assert_eq!(notice.version, 2);
    }
    tx.commit().await.unwrap();
    notices.into_iter().map(|item| item.event_id).collect()
}

async fn delivery_state(fixture: &Fixture) -> (i64, i64) {
    sqlx::query_as(
        "SELECT (SELECT count(*) FROM canteiro_meta.contract_notification_outbox WHERE delivered_at IS NULL),                 (SELECT count(*) FROM canteiro.contract_notifications)",
    )
    .fetch_one(&fixture.admin)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn revision_and_outbox_commit_together_and_worker_disconnect_is_recoverable() {
    let fixture = Fixture::ready().await;
    let db = fixture.database(TransactionLimits::default()).await;
    let c = contract(&fixture).await;

    let mut change = scoped(&db, c.tenant, c.author).await;
    let rolled_back_event = revise(&mut change, &c).await;
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Idle
    );
    change.tx.rollback().await.unwrap();
    assert_eq!(delivery_state(&fixture).await, (0, 0));

    let mut change = scoped(&db, c.tenant, c.author).await;
    let event = revise(&mut change, &c).await;
    change.commit().await.unwrap();
    assert_ne!(rolled_back_event, event);
    assert_eq!(delivery_state(&fixture).await, (1, 0));
    assert!(notices(&db, &c, c.author).await.is_empty());

    let mut worker = db.pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *worker)
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT canteiro_meta.consume_contract_notification()")
        .fetch_one(&mut *worker)
        .await
        .unwrap();
    assert_eq!(status, "delivered");
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Idle
    );
    assert_eq!(delivery_state(&fixture).await, (1, 0));
    let terminated: bool = sqlx::query_scalar("SELECT pg_terminate_backend($1,2000)")
        .bind(pid)
        .fetch_one(&fixture.admin)
        .await
        .unwrap();
    assert!(terminated);
    assert!(worker.commit().await.is_err());

    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Delivered
    );
    assert_eq!(delivery_state(&fixture).await, (0, 1));
    assert_eq!(notices(&db, &c, c.author).await, [event]);
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Idle
    );
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn concurrent_consumers_skip_locked_and_restore_tenant_and_actor() {
    let fixture = Fixture::ready().await;
    let db = fixture.database(TransactionLimits::default()).await;
    let first = contract(&fixture).await;
    let second = contract(&fixture).await;
    let mut change = scoped(&db, first.tenant, first.author).await;
    let first_event = revise(&mut change, &first).await;
    change.commit().await.unwrap();
    let mut change = scoped(&db, second.tenant, second.author).await;
    let second_event = revise(&mut change, &second).await;
    change.commit().await.unwrap();

    let mut first_worker = scoped(&db, second.tenant, second.author).await;
    let status: String = sqlx::query_scalar("SELECT canteiro_meta.consume_contract_notification()")
        .fetch_one(&mut *first_worker.tx)
        .await
        .unwrap();
    assert_eq!(status, "delivered");
    let current: (String, String) = sqlx::query_as(
        "SELECT current_setting('canteiro.tenant_id'),current_setting('canteiro.actor_id')",
    )
    .fetch_one(&mut *first_worker.tx)
    .await
    .unwrap();
    assert_eq!(
        current,
        (second.tenant.to_string(), second.author.to_string())
    );

    let (queue_read, direct_enqueue): (bool, bool) = sqlx::query_as(
        "SELECT has_table_privilege(current_user,'canteiro_meta.contract_notification_outbox','SELECT'),
                has_function_privilege(current_user,'canteiro_meta.enqueue_contract_notification(uuid,uuid)','EXECUTE')",
    )
    .fetch_one(&mut *first_worker.tx)
    .await
    .unwrap();
    assert!(!queue_read && !direct_enqueue);

    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Delivered
    );
    assert_eq!(notices(&db, &second, second.author).await, [second_event]);
    assert!(notices(&db, &first, first.author).await.is_empty());
    first_worker.commit().await.unwrap();
    assert_eq!(notices(&db, &first, first.author).await, [first_event]);
    assert!(notices(&db, &first, second.author).await.is_empty());
    assert_eq!(delivery_state(&fixture).await, (0, 2));
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Idle
    );
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn acknowledgement_failure_is_deferred_and_a_redelivered_row_cannot_duplicate() {
    let fixture = Fixture::ready().await;
    let db = fixture.database(TransactionLimits::default()).await;
    let first = contract(&fixture).await;
    let second = contract(&fixture).await;
    let mut change = scoped(&db, first.tenant, first.author).await;
    let first_event = revise(&mut change, &first).await;
    change.commit().await.unwrap();
    let mut change = scoped(&db, second.tenant, second.author).await;
    let second_event = revise(&mut change, &second).await;
    change.commit().await.unwrap();

    sqlx::raw_sql(
        "CREATE FUNCTION canteiro_meta.fail_fixture_ack() RETURNS TRIGGER LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.delivered_at IS NOT NULL AND OLD.delivered_at IS NULL AND NEW.attempts = 1 THEN
                RAISE EXCEPTION 'Falha fictícia no ACK';
            END IF;
            RETURN NEW;
        END $$;
        CREATE TRIGGER fail_fixture_ack BEFORE UPDATE ON canteiro_meta.contract_notification_outbox
            FOR EACH ROW EXECUTE FUNCTION canteiro_meta.fail_fixture_ack()",
    )
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Deferred
    );
    assert_eq!(delivery_state(&fixture).await, (2, 0));
    let deferred: bool = sqlx::query_scalar(
        "SELECT next_attempt_at > queued_at AND attempts=1 FROM canteiro_meta.contract_notification_outbox WHERE event_id=$1",
    )
    .bind(first_event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert!(deferred);
    sqlx::query(
        "UPDATE canteiro_meta.contract_notification_outbox
         SET next_attempt_at=clock_timestamp()+interval '1 hour' WHERE event_id=$1",
    )
    .bind(first_event)
    .execute(&fixture.admin)
    .await
    .unwrap();
    sqlx::raw_sql("DROP TRIGGER fail_fixture_ack ON canteiro_meta.contract_notification_outbox")
        .execute(&fixture.admin)
        .await
        .unwrap();
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Delivered
    );
    assert_eq!(notices(&db, &second, second.author).await, [second_event]);

    sqlx::query("UPDATE canteiro_meta.contract_notification_outbox SET next_attempt_at=clock_timestamp() WHERE event_id=$1")
        .bind(first_event).execute(&fixture.admin).await.unwrap();
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Delivered
    );
    let delivered_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT delivered_at FROM canteiro.contract_notifications WHERE event_id=$1",
    )
    .bind(first_event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();

    sqlx::query(
        "UPDATE canteiro_meta.contract_notification_outbox SET delivered_at=NULL WHERE event_id=$1",
    )
    .bind(first_event)
    .execute(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(
        db.consume_contract_notification().await.unwrap(),
        NotificationStep::Delivered
    );
    let after_retry: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT delivered_at FROM canteiro.contract_notifications WHERE event_id=$1",
    )
    .bind(first_event)
    .fetch_one(&fixture.admin)
    .await
    .unwrap();
    assert_eq!(after_retry, delivered_at);
    assert_eq!(delivery_state(&fixture).await, (0, 2));
    assert_eq!(notices(&db, &first, first.author).await, [first_event]);
    fixture.cleanup().await;
}
