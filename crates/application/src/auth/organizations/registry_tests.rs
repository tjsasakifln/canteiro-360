//! Casos independentes de cadastro e carteira com PostgreSQL e papel restrito.
use super::*;
use canteiro_contracts::{parties::*, portfolio::*};

fn company_input(tax: &str) -> PartyInput {
    PartyInput {
        kind: PartyKind::Company,
        name: "Empresa fictícia Alfa".into(),
        tax_id: Some(tax.into()),
        roles: vec![PartyRole::Customer, PartyRole::Supplier],
        contacts: vec![],
        address: "Rua de teste, 100".into(),
        professional: None,
        active: true,
    }
}
fn save(context: OrganizationScope, input: PartyInput) -> PartyCommand {
    PartyCommand::Save {
        context,
        key: Uuid::new_v4().to_string(),
        id: None,
        expected_version: None,
        input,
        reason: "Cadastro de teste".into(),
    }
}
fn account(number: &str) -> BankAccount {
    BankAccount {
        bank: "Banco fictício".into(),
        branch: "0001".into(),
        account: number.into(),
        holder: "Empresa fictícia Alfa".into(),
        holder_tax_id: "12ABC34501DE35".into(),
    }
}
async fn company(f: &Fixture, p: &Person, title: &str) -> OrganizationScope {
    let (id, _) = f.create(p, title).await;
    f.select(p, &id).await
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_official_cnpj_text_export_dedup_and_tenant_isolation() {
    let f = Fixture::new(4).await;
    let a = f.person("cadastros-a", true).await;
    let b = f.person("cadastros-b", true).await;
    let sa = company(&f, &a, "Empresa A fictícia").await;
    let sb = company(&f, &b, "Empresa B fictícia").await;
    let first = f
        .service
        .parties_execute(
            &a.token,
            save(sa.clone(), company_input("12.ABC.345/01DE-35")),
        )
        .await
        .unwrap();
    let duplicate = f
        .service
        .parties_execute(&a.token, save(sa.clone(), company_input("12ABC34501DE35")))
        .await
        .unwrap();
    assert_eq!(first.ids, duplicate.ids);
    assert_eq!(duplicate.reused, 1);
    let other = f
        .service
        .parties_execute(&b.token, save(sb.clone(), company_input("12ABC34501DE35")))
        .await
        .unwrap();
    assert_ne!(first.ids, other.ids);
    let export = f.service.parties_export(&a.token, &sa, "").await.unwrap();
    assert_eq!(export.len(), 1);
    assert_eq!(export[0].input.tax_id.as_deref(), Some("12ABC34501DE35"));
    assert_eq!(
        export[0].input.clone().validated().unwrap(),
        export[0].input
    );
    assert!(
        f.service
            .party_detail(&b.token, &sb, &first.ids[0])
            .await
            .is_err()
    );
    assert!(
        f.service
            .party_detail(&a.token, &sb, &other.ids[0])
            .await
            .is_err()
    );
    let invalid = f
        .service
        .parties_execute(&a.token, save(sa.clone(), company_input("12ABC34501DE34")))
        .await;
    error(invalid, "invalid_input");
    let numeric = f
        .service
        .parties_execute(
            &a.token,
            save(sa.clone(), company_input("12.345.678/0001-95")),
        )
        .await
        .unwrap();
    assert_ne!(numeric.ids, first.ids);
    let rows = f
        .service
        .parties_context(&a.token, "12ABC", 0)
        .await
        .unwrap();
    assert_eq!(rows.total, 1);
    assert_eq!(rows.rows[0].id, first.ids[0]);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_idempotency_concurrent_creation_payload_change_and_old_version() {
    let f = Fixture::new(4).await;
    let admin = f.person("cadastro-idempotente", true).await;
    let scope = company(&f, &admin, "Empresa de teste").await;
    let command = save(scope.clone(), company_input("12ABC34501DE35"));
    let (a, b) = tokio::join!(
        f.service.parties_execute(&admin.token, command.clone()),
        f.service.parties_execute(&admin.token, command.clone())
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a, b);
    let mut different = command.clone();
    if let PartyCommand::Save { input, .. } = &mut different {
        input.name = "Outra entrada".into();
    }
    error(
        f.service.parties_execute(&admin.token, different).await,
        "idempotency_conflict",
    );
    let mut changed = company_input("12ABC34501DE35");
    changed.name = "Nome revisado".into();
    let update = PartyCommand::Save {
        context: scope.clone(),
        key: Uuid::new_v4().to_string(),
        id: Some(a.ids[0].clone()),
        expected_version: Some(1),
        input: changed,
        reason: "Correção independente".into(),
    };
    let updated = f
        .service
        .parties_execute(&admin.token, update.clone())
        .await
        .unwrap();
    assert_eq!(updated.versions, vec![2]);
    let mut old = update;
    if let PartyCommand::Save { key, input, .. } = &mut old {
        *key = Uuid::new_v4().to_string();
        input.name = "Rascunho obsoleto".into();
    }
    error(
        f.service.parties_execute(&admin.token, old).await,
        "version_conflict",
    );
    let history = f
        .service
        .party_revisions(&admin.token, &scope, &a.ids[0])
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(
        history[0].before.as_ref().unwrap().name,
        "Empresa fictícia Alfa"
    );
    assert_eq!(history[0].after.name, "Nome revisado");
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.parties")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(count, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_bank_permission_immutable_versions_and_inactive_supplier() {
    let f = Fixture::new(4).await;
    let admin = f.person("banco-admin", true).await;
    let member = f.person("banco-membro", false).await;
    company(&f, &admin, "Empresa bancária fictícia").await;
    f.add(&admin, &member, OrganizationRole::Member).await;
    let sa = f.current_scope(&admin).await;
    let sm = f.select(&member, &sa.organization_id).await;
    let party = f
        .service
        .parties_execute(
            &admin.token,
            save(sa.clone(), company_input("12ABC34501DE35")),
        )
        .await
        .unwrap();
    let party_id = party.ids[0].clone();
    let command = |context: OrganizationScope, version, number: &str| PartyCommand::SetBank {
        context,
        key: Uuid::new_v4().to_string(),
        id: party_id.clone(),
        expected_version: version,
        account: account(number),
        reason: "Conferência bancária do fornecedor".into(),
    };
    assert!(
        f.service
            .parties_execute(&member.token, command(sm.clone(), 1, "12345-6"))
            .await
            .is_err()
    );
    assert!(
        f.service
            .party_bank_history(&member.token, &sm, &party_id)
            .await
            .is_err()
    );
    f.service
        .parties_execute(&admin.token, command(sa.clone(), 1, "12345-6"))
        .await
        .unwrap();
    f.service
        .parties_execute(&admin.token, command(sa.clone(), 2, "98765-4"))
        .await
        .unwrap();
    let history = f
        .service
        .party_bank_history(&admin.token, &sa, &party_id)
        .await
        .unwrap();
    assert_eq!(history.versions.len(), 2);
    assert_eq!(history.versions[0].account.account, "98765-4");
    assert_eq!(history.versions[1].account.account, "12345-6");
    assert_eq!(history.versions[1].version, 2);
    let ordinary = f
        .service
        .parties_export(&member.token, &sm, "")
        .await
        .unwrap();
    assert!(
        !serde_json::to_string(&ordinary)
            .unwrap()
            .contains("98765-4")
    );
    let mut direct = f.admin.begin().await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("SET LOCAL ROLE {}", f.runtime)))
        .execute(&mut *direct)
        .await
        .unwrap();
    sqlx::query(
        "SELECT set_config('canteiro.tenant_id',$1,true),set_config('canteiro.actor_id',$2,true)",
    )
    .bind(&sa.organization_id)
    .bind(member.id.to_string())
    .execute(&mut *direct)
    .await
    .unwrap();
    let hidden: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.party_bank_versions")
        .fetch_one(&mut *direct)
        .await
        .unwrap();
    assert_eq!(hidden, 0);
    direct.rollback().await.unwrap();
    for table in [
        "party_history",
        "party_bank_versions",
        "registry_operations",
    ] {
        let mut tx = f.admin.begin().await.unwrap();
        assert!(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DELETE FROM canteiro.{table}")))
                .execute(&mut *tx)
                .await
                .is_err()
        );
        tx.rollback().await.unwrap();
    }
    let mut input = company_input("12ABC34501DE35");
    input.active = false;
    f.service
        .parties_execute(
            &admin.token,
            PartyCommand::Save {
                context: sa.clone(),
                key: Uuid::new_v4().to_string(),
                id: Some(party_id.clone()),
                expected_version: Some(3),
                input,
                reason: "Encerramento do relacionamento".into(),
            },
        )
        .await
        .unwrap();
    error(
        f.service
            .parties_execute(&admin.token, command(sa, 4, "00000-1"))
            .await,
        "invalid_input",
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_import_is_atomic_and_professional_does_not_create_login() {
    let f = Fixture::new(4).await;
    let admin = f.person("importador", true).await;
    let scope = company(&f, &admin, "Empresa de importação fictícia").await;
    let invalid = PartyCommand::Import {
        context: scope.clone(),
        key: Uuid::new_v4().to_string(),
        rows: vec![
            company_input("12ABC34501DE35"),
            company_input("12ABC34501DE34"),
        ],
    };
    error(
        f.service.parties_execute(&admin.token, invalid).await,
        "invalid_input",
    );
    assert_eq!(
        f.service
            .parties_context(&admin.token, "", 0)
            .await
            .unwrap()
            .total,
        0
    );
    let professional = PartyInput {
        kind: PartyKind::Person,
        name: "Profissional fictício".into(),
        tax_id: Some("52998224725".into()),
        roles: vec![PartyRole::Professional],
        contacts: vec![],
        address: String::new(),
        professional: Some(Professional {
            council: "CREA".into(),
            registration: "123456".into(),
            uf: "SC".into(),
            valid_from: Some("2026-01-01".parse().unwrap()),
            valid_until: Some("2026-11-19".parse().unwrap()),
        }),
        active: true,
    };
    f.service
        .parties_execute(&admin.token, save(scope, professional))
        .await
        .unwrap();
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro_auth.users")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(users, 1);
    let grants: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.access_grants")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(grants, 0);
    f.close().await;
}

fn work_input() -> WorkInput {
    WorkInput {
        title: "Obra fictícia".into(),
        development: "Empreendimento de teste".into(),
        asset_name: "Edificação de teste".into(),
        ownership: AssetOwnership::ThirdParty,
        cost_center: "CC-01".into(),
        external_reference: "EXT-01".into(),
        status: WorkStatus::Planned,
        address: String::new(),
        latitude: None,
        longitude: None,
        section: "Trecho de teste".into(),
        dimension: "10".parse().unwrap(),
        unit: "km".into(),
        localities: vec![
            LocalityAllocation {
                municipality: "Florianópolis".into(),
                uf: "SC".into(),
                percentage: "40".parse().unwrap(),
                dimension: "4".parse().unwrap(),
            },
            LocalityAllocation {
                municipality: "São José".into(),
                uf: "SC".into(),
                percentage: "60".parse().unwrap(),
                dimension: "6".parse().unwrap(),
            },
        ],
        contract_ids: vec![],
    }
}
async fn work_scope(f: &Fixture, scope: &OrganizationScope, owner: &Person) -> Uuid {
    let work = Uuid::new_v4();
    sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,created_by) VALUES($1,$2,'work','Obra fictícia',$3)").bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(work).bind(owner.id).execute(&f.admin).await.unwrap();
    work
}
async fn grant_work(
    f: &Fixture,
    scope: &OrganizationScope,
    work: Uuid,
    owner: &Person,
    who: &Person,
    edit: bool,
) -> Uuid {
    let grant = Uuid::new_v4();
    let actions = if edit {
        vec!["view", "manage_portfolio"]
    } else {
        vec!["view"]
    };
    sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,created_by) VALUES($1,$2,$3,$4,'manager',$5,0,clock_timestamp(),$6)").bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(grant).bind(work).bind(who.id).bind(actions).bind(owner.id).execute(&f.admin).await.unwrap();
    grant
}
fn save_work(scope: OrganizationScope, work: Uuid, input: WorkInput) -> PortfolioCommand {
    PortfolioCommand {
        context: scope,
        key: Uuid::new_v4().to_string(),
        work_id: work.to_string(),
        expected_version: None,
        input,
        reason: "Cadastro inicial da obra".into(),
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_portfolio_permissions_apply_to_search_map_totals_and_direct_reads() {
    let f = Fixture::new(4).await;
    let admin = f.person("carteira-admin", true).await;
    let reader = f.person("carteira-leitor", false).await;
    company(&f, &admin, "Empresa carteira fictícia").await;
    f.add(&admin, &reader, OrganizationRole::Member).await;
    let sa = f.current_scope(&admin).await;
    let sr = f.select(&reader, &sa.organization_id).await;
    let w1 = work_scope(&f, &sa, &admin).await;
    let w2 = work_scope(&f, &sa, &admin).await;
    assert_eq!(
        f.service
            .portfolio_context(&admin.token, "", None)
            .await
            .unwrap()
            .total,
        0
    );
    assert!(
        f.service
            .portfolio_execute(&admin.token, save_work(sa.clone(), w1, work_input()))
            .await
            .is_err()
    );
    grant_work(&f, &sa, w1, &admin, &admin, true).await;
    grant_work(&f, &sa, w2, &admin, &admin, true).await;
    grant_work(&f, &sr, w1, &admin, &reader, false).await;
    let mut visible = work_input();
    visible.title = "Visível no mapa".into();
    visible.latitude = Some("-27.59".parse().unwrap());
    visible.longitude = Some("-48.55".parse().unwrap());
    f.service
        .portfolio_execute(&admin.token, save_work(sa.clone(), w1, visible))
        .await
        .unwrap();
    let mut hidden = work_input();
    hidden.title = "Obra secreta".into();
    f.service
        .portfolio_execute(&admin.token, save_work(sa.clone(), w2, hidden))
        .await
        .unwrap();
    let list = f
        .service
        .portfolio_context(&reader.token, "", None)
        .await
        .unwrap();
    assert_eq!((list.total, list.located), (1, 1));
    assert_eq!(list.rows[0].id, w1.to_string());
    assert_eq!(
        f.service
            .portfolio_context(&reader.token, "secreta", None)
            .await
            .unwrap()
            .total,
        0
    );
    assert!(
        f.service
            .portfolio_detail(&reader.token, &sr, &w2.to_string())
            .await
            .is_err()
    );
    assert!(
        f.service
            .portfolio_execute(&reader.token, save_work(sr, w1, work_input()))
            .await
            .is_err()
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_portfolio_many_contracts_one_work_exact_allocation_and_conflicts() {
    let f = Fixture::new(4).await;
    let admin = f.person("carteira-vinculos", true).await;
    let scope = company(&f, &admin, "Carteira com vínculos fictícios").await;
    let work = work_scope(&f, &scope, &admin).await;
    grant_work(&f, &scope, work, &admin, &admin, true).await;
    let mut input = work_input();
    for _ in 0..2 {
        let contract = Uuid::new_v4();
        sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES($1,$2,'contract','Contrato fictício',$3,$4)").bind(Uuid::parse_str(&scope.organization_id).unwrap()).bind(contract).bind(work).bind(admin.id).execute(&f.admin).await.unwrap();
        grant_work(&f, &scope, contract, &admin, &admin, false).await;
        input.contract_ids.push(contract.to_string());
    }
    let mut invalid = input.clone();
    invalid.localities[0].percentage = "30".parse().unwrap();
    error(
        f.service
            .portfolio_execute(&admin.token, save_work(scope.clone(), work, invalid))
            .await,
        "invalid_input",
    );
    let command = save_work(scope.clone(), work, input);
    let (a, b) = tokio::join!(
        f.service.portfolio_execute(&admin.token, command.clone()),
        f.service.portfolio_execute(&admin.token, command.clone())
    );
    assert_eq!(a.unwrap().id, b.unwrap().id);
    let list = f
        .service
        .portfolio_context(&admin.token, "", None)
        .await
        .unwrap();
    assert_eq!((list.total, list.located), (1, 0));
    assert_eq!(list.rows[0].input.contract_ids.len(), 2);
    assert_eq!(list.rows[0].input.latitude, None);
    let mut different = command.clone();
    different.input.title = "Outro payload".into();
    error(
        f.service.portfolio_execute(&admin.token, different).await,
        "idempotency_conflict",
    );
    let mut revision = command;
    revision.key = Uuid::new_v4().to_string();
    revision.expected_version = Some(1);
    revision.input.status = WorkStatus::Active;
    f.service
        .portfolio_execute(&admin.token, revision.clone())
        .await
        .unwrap();
    revision.key = Uuid::new_v4().to_string();
    error(
        f.service.portfolio_execute(&admin.token, revision).await,
        "version_conflict",
    );
    let history = f
        .service
        .portfolio_revisions(&admin.token, &scope, &work.to_string())
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].after.status, WorkStatus::Planned);
    assert_eq!(history[0].after.status, WorkStatus::Active);
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM canteiro.portfolio_contract_links")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    assert_eq!(links, 2);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_revocation_prevents_receipt_replay_and_foreign_portfolio() {
    let f = Fixture::new(4).await;
    let admin = f.person("carteira-revogacao", true).await;
    let other = f.person("carteira-outra", true).await;
    let scope = company(&f, &admin, "Empresa A revogação").await;
    let foreign = company(&f, &other, "Empresa B revogação").await;
    let work = work_scope(&f, &scope, &admin).await;
    let grant = grant_work(&f, &scope, work, &admin, &admin, true).await;
    let command = save_work(scope.clone(), work, work_input());
    f.service
        .portfolio_execute(&admin.token, command.clone())
        .await
        .unwrap();
    assert!(
        f.service
            .portfolio_detail(&other.token, &foreign, &work.to_string())
            .await
            .is_err()
    );
    sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp(),version=version+1 WHERE id=$1").bind(grant).execute(&f.admin).await.unwrap();
    assert!(
        f.service
            .portfolio_execute(&admin.token, command)
            .await
            .is_err()
    );
    assert_eq!(
        f.service
            .portfolio_context(&admin.token, "", None)
            .await
            .unwrap()
            .total,
        0
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_context_refresh_recovers_same_receipts_without_duplication() {
    let f = Fixture::new(4).await;
    let admin = f.person("recibo-contexto", true).await;
    let scope = company(&f, &admin, "Empresa de recibos").await;
    let mut party = save(scope.clone(), company_input("12ABC34501DE35"));
    let saved_party = f
        .service
        .parties_execute(&admin.token, party.clone())
        .await
        .unwrap();
    let work = work_scope(&f, &scope, &admin).await;
    grant_work(&f, &scope, work, &admin, &admin, true).await;
    let mut portfolio = save_work(scope.clone(), work, work_input());
    let saved_work = f
        .service
        .portfolio_execute(&admin.token, portfolio.clone())
        .await
        .unwrap();
    f.execute(
        &admin,
        OrganizationCommand::Rename {
            scope: scope.clone(),
            name: "Nome atualizado da empresa".into(),
        },
    )
    .await
    .unwrap();
    f.execute(
        &admin,
        OrganizationCommand::ClearSelection {
            expected_context_version: scope.context_version,
        },
    )
    .await
    .unwrap();
    let current = f.select(&admin, &scope.organization_id).await;
    assert_ne!(current.context_version, scope.context_version);
    assert_ne!(current.organization_version, scope.organization_version);
    if let PartyCommand::Save { context, .. } = &mut party {
        *context = current.clone();
    }
    portfolio.context = current;
    let replay_party = f
        .service
        .parties_execute(&admin.token, party)
        .await
        .unwrap();
    let replay_work = f
        .service
        .portfolio_execute(&admin.token, portfolio)
        .await
        .unwrap();
    assert_eq!(replay_party, saved_party);
    assert_eq!(
        (replay_work.id, replay_work.version),
        (saved_work.id, saved_work.version)
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM canteiro.party_history),(SELECT count(*) FROM canteiro.portfolio_history),(SELECT count(*) FROM canteiro.registry_operations)").fetch_one(&f.admin).await.unwrap();
    assert_eq!(counts, (1, 1, 2));
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL real local e CANTEIRO_TEST_DATABASE_URL"]
async fn registry_expiry_while_waiting_for_row_lock_hides_records_and_history() {
    let f = Fixture::new(4).await;
    let admin = f.person("registro-prazo-admin", true).await;
    let reader = f.person("registro-prazo-leitor", false).await;
    company(&f, &admin, "Empresa de prazo").await;
    f.add(&admin, &reader, OrganizationRole::Member).await;
    let scope = f.current_scope(&admin).await;
    let read_scope = f.select(&reader, &scope.organization_id).await;
    let tenant = Uuid::parse_str(&scope.organization_id).unwrap();
    let party = f
        .service
        .parties_execute(
            &admin.token,
            save(scope.clone(), company_input("12ABC34501DE35")),
        )
        .await
        .unwrap()
        .ids[0]
        .clone();
    let work = work_scope(&f, &scope, &admin).await;
    grant_work(&f, &scope, work, &admin, &admin, true).await;
    let reader_grant = grant_work(&f, &scope, work, &admin, &reader, false).await;
    f.service
        .portfolio_execute(&admin.token, save_work(scope, work, work_input()))
        .await
        .unwrap();
    for case in 0..4 {
        sqlx::query("UPDATE canteiro.organization_members SET valid_until=NULL WHERE tenant_id=$1 AND user_id=$2").bind(tenant).bind(reader.id).execute(&f.admin).await.unwrap();
        sqlx::query("UPDATE canteiro.access_grants SET valid_until=NULL WHERE id=$1")
            .bind(reader_grant)
            .execute(&f.admin)
            .await
            .unwrap();
        let mut blocker = f.admin.begin().await.unwrap();
        let blocker_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *blocker)
            .await
            .unwrap();
        let (lock_query, target) = if case < 2 {
            (
                "SELECT id FROM canteiro.parties WHERE id=$1 FOR UPDATE",
                Uuid::parse_str(&party).unwrap(),
            )
        } else {
            (
                "SELECT id FROM canteiro.portfolio_works WHERE id=$1 FOR UPDATE",
                work,
            )
        };
        sqlx::query(lock_query)
            .bind(target)
            .execute(&mut *blocker)
            .await
            .unwrap();
        if case < 2 {
            sqlx::query("UPDATE canteiro.organization_members SET valid_until=clock_timestamp()+interval '650 milliseconds' WHERE tenant_id=$1 AND user_id=$2").bind(tenant).bind(reader.id).execute(&f.admin).await.unwrap();
        } else {
            sqlx::query("UPDATE canteiro.access_grants SET valid_until=clock_timestamp()+interval '650 milliseconds' WHERE id=$1").bind(reader_grant).execute(&f.admin).await.unwrap();
        }
        let service = f.service.clone();
        let token = reader.token.clone();
        let context = read_scope.clone();
        let identity = target.to_string();
        let reading = tokio::spawn(async move {
            match case {
                0 => service
                    .party_detail(&token, &context, &identity)
                    .await
                    .map(|_| ()),
                1 => service
                    .party_revisions(&token, &context, &identity)
                    .await
                    .map(|_| ()),
                2 => service
                    .portfolio_detail(&token, &context, &identity)
                    .await
                    .map(|_| ()),
                _ => service
                    .portfolio_revisions(&token, &context, &identity)
                    .await
                    .map(|_| ()),
            }
        });
        tokio::time::timeout(StdDuration::from_millis(500),async {
            loop {
                let blocked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))").bind(blocker_pid).fetch_one(&f.admin).await.unwrap();
                if blocked {break;}
                tokio::time::sleep(StdDuration::from_millis(10)).await;
            }
        }).await.expect("a consulta precisa aguardar o bloqueio da linha antes de expirar");
        if case < 2 {
            sqlx::query("SELECT pg_sleep(GREATEST(0,extract(epoch FROM valid_until-clock_timestamp())+0.02)) FROM canteiro.organization_members WHERE tenant_id=$1 AND user_id=$2").bind(tenant).bind(reader.id).execute(&f.admin).await.unwrap();
        } else {
            sqlx::query("SELECT pg_sleep(GREATEST(0,extract(epoch FROM valid_until-clock_timestamp())+0.02)) FROM canteiro.access_grants WHERE id=$1").bind(reader_grant).execute(&f.admin).await.unwrap();
        }
        blocker.commit().await.unwrap();
        error(reading.await.unwrap(), "organization_unavailable");
    }
    f.close().await;
}
