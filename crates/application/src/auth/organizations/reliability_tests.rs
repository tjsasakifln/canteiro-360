//! Casos da aplicação em conexões PostgreSQL distintas.
use super::*;
use canteiro_contracts::reliability::{
    ReliabilityCommand as Command, ReliabilityReply, ReliabilityScope,
};

async fn company(f: &Fixture, owner: &Person, label: &str, contract_id: Uuid) -> ReliabilityScope {
    let (company, _) = f.create(owner, label).await;
    let selected = f.select(owner, &company).await;
    let work_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by)
                 VALUES($1,$2,'work','Obra fictícia',NULL,$4),($1,$3,'contract',$5,$2,$4)",
    )
    .bind(Uuid::parse_str(&company).unwrap())
    .bind(work_id)
    .bind(contract_id)
    .bind(owner.id)
    .bind(label)
    .execute(&f.admin)
    .await
    .unwrap();
    ReliabilityScope {
        organization_id: company,
        context_version: selected.context_version,
    }
}

async fn editor(
    f: &Fixture,
    scope: &ReliabilityScope,
    contract: Uuid,
    owner: &Person,
    editor: &Person,
) -> Uuid {
    let grant = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by)
                 VALUES($1,$2,$3,$4,'manager',ARRAY['view','edit_contract_record'],0,clock_timestamp(),$5)")
        .bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(grant).bind(contract)
        .bind(editor.id).bind(owner.id).execute(&f.admin).await.unwrap();
    grant
}

fn acquire(scope: &ReliabilityScope, contract: &str, lease: Uuid) -> Command {
    Command::Acquire {
        scope: scope.clone(),
        contract_id: contract.into(),
        lease_id: lease.to_string(),
    }
}

fn save(
    scope: &ReliabilityScope,
    contract: &str,
    lease: Uuid,
    key: Uuid,
    expected_version: i64,
    title: &str,
) -> Command {
    Command::Save {
        scope: scope.clone(),
        contract_id: contract.into(),
        lease_id: lease.to_string(),
        key: key.to_string(),
        expected_version,
        title: title.into(),
        reason: "Correção de cadastro".into(),
        occurred_at: None,
    }
}

fn saved(reply: ReliabilityReply) -> canteiro_contracts::reliability::SaveReceipt {
    let ReliabilityReply::Saved { receipt } = reply else {
        panic!("recibo de revisão esperado")
    };
    receipt
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn reliability_distinct_editors_and_tenants_cannot_overwrite_or_replay_each_others_effect() {
    let f = Fixture::new(8).await;
    let first = f.person("gestor", true).await;
    let second = f.person("gestora", false).await;
    let contract = Uuid::new_v4();
    let a = company(&f, &first, "Contrato na primeira empresa", contract).await;
    f.add(&first, &second, OrganizationRole::Member).await;
    let scope = f.select(&second, &a.organization_id).await;
    let a_second = ReliabilityScope {
        organization_id: a.organization_id.clone(),
        context_version: scope.context_version,
    };
    editor(&f, &a, contract, &first, &first).await;
    editor(&f, &a, contract, &first, &second).await;
    let id = &contract.to_string();

    let (one, two) = tokio::join!(
        f.service.reliability_detail(&first.token, &a, id, None),
        f.service
            .reliability_detail(&second.token, &a_second, id, None),
    );
    assert_eq!(one.unwrap().contract.version, 1);
    assert_eq!(two.unwrap().contract.version, 1);

    let lease_a = Uuid::new_v4();
    let lease_b = Uuid::new_v4();
    let (first_acquire, second_acquire) = tokio::join!(
        f.service
            .reliability_execute(&first.token, acquire(&a, id, lease_a)),
        f.service
            .reliability_execute(&second.token, acquire(&a_second, id, lease_b)),
    );
    assert_ne!(first_acquire.is_ok(), second_acquire.is_ok());
    let (winner, winner_scope, winner_lease, loser, loser_scope, loser_lease) =
        if first_acquire.is_ok() {
            error(second_acquire, "edit_reserved");
            (&first, &a, lease_a, &second, &a_second, lease_b)
        } else {
            error(first_acquire, "edit_reserved");
            (&second, &a_second, lease_b, &first, &a, lease_a)
        };

    let key = Uuid::new_v4();
    let key_str = key.to_string();
    let original = save(
        winner_scope,
        id,
        winner_lease,
        key,
        1,
        "Gravação da primeira sessão",
    );
    let stale = save(
        loser_scope,
        id,
        loser_lease,
        Uuid::new_v4(),
        1,
        "Rascunho da segunda sessão",
    );
    let receipt = saved(
        f.service
            .reliability_execute(&winner.token, original.clone())
            .await
            .unwrap(),
    );
    assert_eq!(receipt.version, 2);
    let conflict = f
        .service
        .reliability_execute(&loser.token, stale)
        .await
        .unwrap();
    let ReliabilityReply::VersionConflict { current } = conflict else {
        panic!("conflito de versão esperado")
    };
    assert_eq!((current.version, current.title), (2, receipt.title.clone()));
    assert_eq!(
        f.service
            .reliability_operation(&loser.token, loser_scope, id, &key_str)
            .await
            .unwrap(),
        None
    );

    f.service
        .reliability_execute(&loser.token, acquire(loser_scope, id, loser_lease))
        .await
        .unwrap();
    error(
        f.service
            .reliability_execute(
                &loser.token,
                save(
                    loser_scope,
                    id,
                    loser_lease,
                    key,
                    2,
                    "Chave de outra pessoa",
                ),
            )
            .await,
        "idempotency_conflict",
    );
    let second_save = f
        .service
        .reliability_execute(
            &loser.token,
            save(
                loser_scope,
                id,
                loser_lease,
                Uuid::new_v4(),
                2,
                "Rascunho da segunda sessão",
            ),
        )
        .await
        .unwrap();
    assert_eq!(saved(second_save).version, 3);
    assert_eq!(
        saved(
            f.service
                .reliability_execute(&winner.token, original)
                .await
                .unwrap()
        ),
        receipt
    );

    let other_owner = f.person("outra-empresa", true).await;
    let b = company(&f, &other_owner, "Contrato da empresa vizinha", contract).await;
    editor(&f, &b, contract, &other_owner, &other_owner).await;
    assert_eq!(
        f.service
            .reliability_operation(&other_owner.token, &b, id, &key_str)
            .await
            .unwrap(),
        None
    );
    let lease_c = Uuid::new_v4();
    f.service
        .reliability_execute(&other_owner.token, acquire(&b, id, lease_c))
        .await
        .unwrap();
    let other = saved(
        f.service
            .reliability_execute(
                &other_owner.token,
                save(&b, id, lease_c, key, 1, "Gravação na empresa vizinha"),
            )
            .await
            .unwrap(),
    );
    assert_ne!(other.event_id, receipt.event_id);
    error(
        f.service
            .reliability_operation(&winner.token, &b, id, &key_str)
            .await,
        "context_changed",
    );

    let history = f
        .service
        .reliability_history(&winner.token, winner_scope, id)
        .await
        .unwrap();
    assert_eq!(
        history.iter().map(|v| v.version).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(
        history[1].before.as_deref(),
        Some(history[0].after.as_str())
    );
    assert_eq!(
        history[2].before.as_deref(),
        Some(history[1].after.as_str())
    );
    assert_eq!(history[1].actor_id, winner.id.to_string());
    assert_eq!(history[2].actor_id, loser.id.to_string());
    let effect_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM canteiro_meta.contract_notification_outbox")
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert_eq!(effect_count, 3);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn reliability_expired_lease_is_recovered_and_failing_outbox_rolls_back_entire_change() {
    let f = Fixture::new(5).await;
    let owner = f.person("gestor", true).await;
    let user = f.person("sucessor", false).await;
    let contract = Uuid::new_v4();
    let a = company(&f, &owner, "Contrato de recuperação", contract).await;
    f.add(&owner, &user, OrganizationRole::Member).await;
    let selected = f.select(&user, &a.organization_id).await;
    let b = ReliabilityScope {
        organization_id: a.organization_id.clone(),
        context_version: selected.context_version,
    };
    editor(&f, &a, contract, &owner, &owner).await;
    editor(&f, &a, contract, &owner, &user).await;
    let id = &contract.to_string();

    let expired = Uuid::new_v4();
    f.service
        .reliability_execute(&owner.token, acquire(&a, id, expired))
        .await
        .unwrap();
    sqlx::query("UPDATE canteiro.contract_edit_leases SET expires_at=clock_timestamp()-interval '1 second' WHERE contract_id=$1")
        .bind(contract).execute(&f.admin).await.unwrap();
    error(
        f.service
            .reliability_execute(
                &owner.token,
                Command::Renew {
                    scope: a.clone(),
                    contract_id: id.clone(),
                    lease_id: expired.to_string(),
                },
            )
            .await,
        "edit_expired",
    );
    let new = Uuid::new_v4();
    f.service
        .reliability_execute(&user.token, acquire(&b, id, new))
        .await
        .unwrap();
    f.service
        .reliability_execute(
            &owner.token,
            Command::Release {
                scope: a.clone(),
                contract_id: id.clone(),
                lease_id: expired.to_string(),
            },
        )
        .await
        .unwrap();
    assert!(
        f.service
            .reliability_detail(&user.token, &b, id, None)
            .await
            .unwrap()
            .lease_until
            .is_some()
    );

    sqlx::raw_sql(
        "CREATE FUNCTION canteiro_meta.fail_new_notice() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN RAISE EXCEPTION 'failure-fixture'; END $$;
        CREATE TRIGGER fail_new_notice BEFORE INSERT ON canteiro_meta.contract_notification_outbox
          FOR EACH ROW EXECUTE FUNCTION canteiro_meta.fail_new_notice()",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    let key = Uuid::new_v4();
    let command = save(&b, id, new, key, 1, "Gravação recuperável");
    error(
        f.service
            .reliability_execute(&user.token, command.clone())
            .await,
        "unavailable",
    );
    let after_error = f
        .service
        .reliability_detail(&user.token, &b, id, None)
        .await
        .unwrap();
    assert_eq!(after_error.contract.version, 1);
    assert_eq!(after_error.history.len(), 1);
    assert!(after_error.lease_until.is_some());
    assert_eq!(
        f.service
            .reliability_operation(&user.token, &b, id, &key.to_string())
            .await
            .unwrap(),
        None
    );

    sqlx::raw_sql("DROP TRIGGER fail_new_notice ON canteiro_meta.contract_notification_outbox")
        .execute(&f.admin)
        .await
        .unwrap();
    let committed = saved(
        f.service
            .reliability_execute(&user.token, command)
            .await
            .unwrap(),
    );
    assert_eq!(committed.version, 2);
    let detail = f
        .service
        .reliability_detail(&user.token, &b, id, None)
        .await
        .unwrap();
    assert_eq!(detail.history.len(), 2);
    assert!(detail.lease_until.is_none());

    f.execute(
        &owner,
        OrganizationCommand::RemoveMember {
            scope: f.current_scope(&owner).await,
            user_id: user.id.to_string(),
            member_version: 1,
        },
    )
    .await
    .unwrap();
    error(
        f.service
            .reliability_operation(&user.token, &b, id, &key.to_string())
            .await,
        "organization_unavailable",
    );
    assert_eq!(
        f.service
            .reliability_history(&owner.token, &a, id)
            .await
            .unwrap()[1]
            .actor_id,
        user.id.to_string(),
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn reliability_permission_expiring_while_waiting_for_edit_lock_cannot_commit() {
    let f = Fixture::new(4).await;
    let owner = f.person("gestor-temporario", true).await;
    let contract = Uuid::new_v4();
    let scope = company(&f, &owner, "Contrato antes do bloqueio", contract).await;
    let grant = editor(&f, &scope, contract, &owner, &owner).await;
    let lease = Uuid::new_v4();
    let id = contract.to_string();
    f.service
        .reliability_execute(&owner.token, acquire(&scope, &id, lease))
        .await
        .unwrap();

    // O relógio é do PostgreSQL. Só esperamos o prazo após observar a outra
    // conexão bloqueada de verdade na gravação.
    let mut blocker = f.admin.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("SELECT 1 FROM canteiro.contract_edit_leases WHERE contract_id=$1 FOR UPDATE")
        .bind(contract)
        .execute(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("UPDATE canteiro.access_grants SET valid_until=clock_timestamp()+interval '650 milliseconds' WHERE id=$1")
        .bind(grant).execute(&f.admin).await.unwrap();
    let key = Uuid::new_v4();
    let command = save(&scope, &id, lease, key, 1, "Tentativa aguardando bloqueio");
    let service = f.service.clone();
    let token = owner.token.clone();
    let saving = tokio::spawn(async move { service.reliability_execute(&token, command).await });
    tokio::time::timeout(StdDuration::from_millis(500), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                 WHERE $1=ANY(pg_blocking_pids(pid)) AND query LIKE '%rename_contract_reference%')",
            )
            .bind(blocker_pid)
            .fetch_one(&f.admin)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .expect("a gravação precisa esperar pelo bloqueio do lease");
    sqlx::query(
        "SELECT pg_sleep(GREATEST(0,extract(epoch FROM valid_until-clock_timestamp())+0.02))
                 FROM canteiro.access_grants WHERE id=$1",
    )
    .bind(grant)
    .execute(&f.admin)
    .await
    .unwrap();
    blocker.commit().await.unwrap();
    error(saving.await.unwrap(), "edit_expired");
    let state: (i64, String, i64, i64) = sqlx::query_as(
        "SELECT s.version,s.name,
           (SELECT count(*) FROM canteiro.contract_history h WHERE h.tenant_id=s.tenant_id AND h.contract_id=s.id),
           (SELECT count(*) FROM canteiro.operation_receipts r WHERE r.tenant_id=s.tenant_id AND r.scope_id=s.id)
         FROM canteiro.access_scopes s WHERE s.id=$1",
    ).bind(contract).fetch_one(&f.admin).await.unwrap();
    assert_eq!(state, (1, "Contrato antes do bloqueio".into(), 1, 0));
    f.close().await;
}
