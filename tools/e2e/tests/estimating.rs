#[allow(dead_code)]
mod support;

use canteiro_contracts::access::{AccessAction, AccessCommand, AccessRole, ScopeKind};
use canteiro_contracts::estimating::{
    BudgetChange, BudgetCommand, BudgetLeaseCommand, CatalogCsvImportCommand, CostSource,
    NumberLocale,
};
use serde_json::{Value, json};
use std::time::Duration;
use support::{Browser, ENTER, Harness, TAB, TestResult, access_fixture::*, write_json};

async fn create_scope(
    h: &Harness,
    owner: &AccountFixture,
    kind: ScopeKind,
    name: &str,
    parent_id: Option<String>,
) -> TestResult<String> {
    let state = h.auth.access_context(&owner.session).await?.state;
    h.auth
        .access_execute(
            &owner.session,
            AccessCommand::CreateScope {
                context: scope(&state),
                kind,
                name: name.to_owned(),
                parent_id,
            },
        )
        .await?;
    let state = h.auth.access_context(&owner.session).await?.state;
    Ok(state
        .scopes
        .iter()
        .find(|item| item.name == name && item.kind == kind)
        .ok_or("New scope not visible to its administrator")?
        .id
        .clone())
}

#[tokio::test]
#[ignore = "requires built SSR/WASM server, PostgreSQL and WebDriver"]
async fn estimating_imports_reflow_and_work_by_keyboard_at_360_and_1280() -> TestResult {
    let h = Harness::start("estimating-interface").await?;
    let owner = account(&h, "est-own", true).await?;
    let editor = account(&h, "est-ed", false).await?;
    let company_id = company(&h, &owner, "Obra de teste de orcamentos").await?;
    join(&h, &owner, &editor, &company_id).await?;

    let work = create_scope(&h, &owner, ScopeKind::Work, "Obra orcamentaria", None).await?;
    let contract = create_scope(
        &h,
        &owner,
        ScopeKind::Contract,
        "Contrato para orcamento",
        Some(work.clone()),
    )
    .await?;
    let budget = create_scope(
        &h,
        &owner,
        ScopeKind::Budget,
        "Orcamento de ensaio",
        Some(contract),
    )
    .await?;
    fixture_grant(
        &h,
        &owner,
        &editor,
        &budget,
        AccessRole::Budgeter,
        vec![AccessAction::View, AccessAction::EditBudget],
    )
    .await?;
    // A permission-tree scope alone is not a persisted work in the portfolio.
    fixture_grant(
        &h,
        &owner,
        &owner,
        &work,
        AccessRole::Manager,
        vec![AccessAction::View, AccessAction::ManagePortfolio],
    )
    .await?;
    let state = h.auth.access_context(&owner.session).await?.state;
    h.auth
        .portfolio_execute(
            &owner.session,
            canteiro_contracts::portfolio::PortfolioCommand {
                context: scope(&state),
                key: uuid::Uuid::new_v4().to_string(),
                work_id: work.clone(),
                expected_version: None,
                input: serde_json::from_value(json!({
                    "title": "Obra orcamentaria",
                    "development": "",
                    "asset_name": "",
                    "ownership": "owned",
                    "cost_center": "",
                    "external_reference": "",
                    "status": "planned",
                    "address": "",
                    "latitude": null,
                    "longitude": null,
                    "section": "",
                    "dimension": "100",
                    "unit": "m2",
                    "localities": [{
                        "municipality": "Florianopolis",
                        "uf": "SC",
                        "percentage": "100",
                        "dimension": "100"
                    }],
                    "contract_ids": []
                }))?,
                reason: "Cadastro da obra para ensaio de orcamento".into(),
            },
        )
        .await
        .map_err(|error| format!("Portfolio fixture: {error:?}"))?;
    fixture_grant(
        &h,
        &owner,
        &editor,
        &work,
        AccessRole::Budgeter,
        vec![AccessAction::View],
    )
    .await?;
    let authorized = h.auth.estimating_context(&editor.session).await?;
    assert!(
        authorized.context.is_some()
            && authorized
                .scopes
                .iter()
                .any(|scope| scope.id == budget && scope.can_edit),
        "The editor must have an explicit, effective budget grant"
    );

    let browser = Browser::start(&h).await?;
    browser.navigate(&h.app_url).await?;
    browser
        .post(
            "cookie",
            json!({"cookie":{
                "name":"__Host-canteiro-session",
                "value":editor.session,
                "path":"/",
                "secure":true,
                "httpOnly":true,
                "sameSite":"Strict"
            }}),
        )
        .await?;
    browser
        .navigate(&format!("{}/orcamentos", h.app_url))
        .await?;

    let mut viewports: Vec<Value> = Vec::new();
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        let report = browser
            .execute(
                r#"const forms=[...document.querySelectorAll("form.record-form")];
const fields=forms.flatMap(f=>[...f.querySelectorAll("input:not([type='hidden']),select,textarea")])
  .filter(el=>el.getClientRects().length>0);
const unlabelled=fields.filter(el=>{
  const label=el.closest("label") || (el.id && document.querySelector('label[for="'+el.id+'"]'));
  return !label || !label.textContent.trim();
}).length;
const selector=document.querySelector("form[action='/orcamentos/conferir-csv'] select[name='scope_id']");
const budgetOptions=selector?[...selector.options].filter(x=>!x.disabled && x.value).length:0;
return {width:innerWidth,scroll:document.documentElement.scrollWidth,
  title:document.querySelector("h1")?.textContent?.trim(),
  fields:fields.length,unlabelled,budgetOptions,
  hasBudgetImport:!!selector,
  hasCatalogImport:!!document.querySelector("form[action='/orcamentos/catalogos/conferir']"),
  hasMain:!!document.querySelector("main")};"#,
            )
            .await?;
        assert_eq!(report["width"], width, "Unexpected CSS viewport");
        assert!(
            report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width),
            "Horizontal overflow on estimating page: {report}"
        );
        assert_eq!(report["unlabelled"], 0, "Unlabelled estimating fields");
        assert!(
            report["fields"].as_u64().unwrap_or(0) >= 10
                && report["budgetOptions"].as_u64().unwrap_or(0) >= 1
                && report["hasBudgetImport"] == true
                && report["hasCatalogImport"] == true
                && report["hasMain"] == true,
            "Authorized import forms are not usable: {report}"
        );
        browser
            .tab_to("form[action='/orcamentos/conferir-csv'] select[name='scope_id']")
            .await?;
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute(
                    "return document.activeElement?.matches(\"form[action='/orcamentos/conferir-csv'] textarea[name='csv']\");"
                )
                .await?,
            true,
            "The CSV editor must follow the budget selector in keyboard order"
        );
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute(
                    "return document.activeElement?.matches(\"form[action='/orcamentos/conferir-csv'] button[type='submit']\");"
                )
                .await?,
            true,
            "The preview action must be reachable immediately after the CSV field"
        );
        // A tabela de preços também deve ser navegável só pelo teclado.
        browser
            .tab_to("form[action='/orcamentos/catalogos/conferir'] select[name='scope_id']")
            .await?;
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute(
                    "return document.activeElement?.matches(\"form[action='/orcamentos/catalogos/conferir'] input[name='catalog_id']\");"
                )
                .await?,
            true,
            "O identificador da fonte deve vir após o escopo, inclusive no mobile"
        );
        browser
            .tab_to("form[action='/orcamentos/catalogos/conferir'] textarea[name='csv']")
            .await?;
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute(
                    "return document.activeElement?.matches(\"form[action='/orcamentos/catalogos/conferir'] button[type='submit']\");"
                )
                .await?,
            true,
            "A conferência de preços deve seguir o CSV na ordem de teclado"
        );
        browser
            .full_screenshot(&h.evidence.join(format!("estimating-interface-{width}.png")))
            .await?;
        viewports.push(report);
    }
    // Publish two immutable editions through the authenticated application service.
    let context = authorized.context.ok_or("Missing estimating context")?;
    for edition in [1, 2] {
        let mut csv =
            String::from("CANTEIRO-CUSTOS-CSV-1\ncodigo;descricao;categoria;unidade;preco\n");
        for index in 1..=31 {
            csv.push_str(&format!(
                "HIST-{index:02};Servico de ensaio;service;m;{edition}.00\n"
            ));
        }
        let input = CatalogCsvImportCommand {
            context: context.clone(),
            scope_id: budget.clone(),
            catalog_id: "historico-e2e".into(),
            edition,
            published_on: "2026-11-01".into(),
            source: CostSource {
                title: "Tabela de ensaio".into(),
                organization: "Fonte de ensaio".into(),
                license: "Uso interno".into(),
                locality: "SC".into(),
                tax_regime: "regular".into(),
                competence: if edition == 1 { "2021-11" } else { "2026-11" }.into(),
            },
            reason: "Teste de comparacao historica".into(),
            csv,
            locale: NumberLocale::Canonical,
        };
        let preview = h
            .auth
            .estimating_catalog_csv_preview(&editor.session, input.clone())
            .await?;
        assert_eq!(preview.items_total, 31);
        if edition == 2 {
            assert_eq!(preview.changed, 31);
            assert_eq!(preview.previous_edition, Some(1));
            assert_eq!(preview.previous_competence.as_deref(), Some("2021-11"));
            assert_eq!(preview.pages, 4);
            let last = h
                .auth
                .estimating_catalog_csv_preview_page(&editor.session, input.clone(), 4)
                .await?;
            assert_eq!(last.page, 4);
            assert_eq!(last.changed_sample.len(), 1);
            assert_eq!(last.changed_sample[0].after.code, "HIST-31");
        }
        h.auth
            .estimating_catalog_csv_publish(&editor.session, input)
            .await?;
    }

    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        let fill = format!(
            r#"const form=document.querySelector("form[action='/orcamentos/catalogos/conferir']");
const values={{scope_id:{scope},catalog_id:'historico-e2e',edition:'3',
published_on:'2026-11-01',source_title:'Tabela de ensaio',source_organization:'Fonte de ensaio',
source_license:'Uso interno',source_locality:'SC',source_tax_regime:'regular',
source_competence:'2026-11',reason:'Teste de previa integral',
csv:'CANTEIRO-CUSTOS-CSV-1\ncodigo;descricao;categoria;unidade;preco\n'+
Array.from({{length:31}},(_,i)=>'HIST-'+String(i+1).padStart(2,'0')+
';Servico de ensaio;service;m;3,00').join('\n')}};
for(const [name,value] of Object.entries(values)) form.elements.namedItem(name).value=value;
return form.checkValidity();"#,
            scope = serde_json::to_string(&budget)?
        );
        assert_eq!(browser.execute(&fill).await?, true);
        browser
            .tab_to("form[action='/orcamentos/catalogos/conferir'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('select[name=page]')?.value === '1';",
                Duration::from_secs(5),
            )
            .await?;
        assert_eq!(
            browser
                .execute("return document.body.innerText.includes('Alterado: HIST-10') && !document.body.innerText.includes('Alterado: HIST-31');")
                .await?,
            true
        );
        browser.choose("select[name='page']", 3, "4").await?;
        browser
            .tab_to("button[formaction='/orcamentos/catalogos/conferir']")
            .await?;
        browser.key(ENTER).await?;
        // The select already has value 4 before submission. Wait for the
        // heading rendered by the server, not the locally selected option.
        browser
            .wait(
                "return Array.from(document.querySelectorAll('h3')).some(h => h.textContent.trim() === 'Diferenças: página 4 de 4');",
                Duration::from_secs(5),
            )
            .await?;
        let preview_report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,last:document.body.innerText.includes('Alterado: HIST-31'),first:document.body.innerText.includes('Alterado: HIST-01'),pending:document.body.innerText.includes('Nenhuma linha foi publicada')};",
            )
            .await?;
        assert_eq!(preview_report["width"], width);
        assert_eq!(preview_report["last"], true);
        assert_eq!(preview_report["first"], false);
        assert_eq!(preview_report["pending"], true);
        assert!(preview_report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("catalog-preview-{width}.png")))
            .await?;

        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        browser
            .tab_to("form[action='/orcamentos/catalogos/comparar'] select[name='before']")
            .await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('select[name=page]')?.value === '1';",
                Duration::from_secs(5),
            )
            .await?;
        assert_eq!(
            browser
                .execute("return document.body.innerText.includes('HIST-30') && !document.body.innerText.includes('HIST-31') && document.body.innerText.includes('Base histórica anterior') && document.body.innerText.includes('competência 2021-11');")
                .await?,
            true
        );
        browser.choose("select[name='page']", 1, "2").await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('select[name=page]')?.value === '2' && location.search.includes('page=2');",
                Duration::from_secs(5),
            )
            .await?;
        let report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,last:document.body.innerText.includes('HIST-31'),first:document.body.innerText.includes('HIST-01'),historical:document.body.innerText.includes('Base histórica anterior')};",
            )
            .await?;
        assert_eq!(report["width"], width);
        assert_eq!(report["last"], true);
        assert_eq!(report["first"], false);
        assert_eq!(report["historical"], true);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("catalog-history-{width}.png")))
            .await?;

        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        browser
            .tab_to("form[action='/orcamentos/catalogos/consultar']:has(input[name='edition'][value='1']) button")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('select[name=page]')?.value === '1' && document.body.innerText.includes('Competência: 2021-11');",
                Duration::from_secs(5),
            )
            .await?;
        assert_eq!(
            browser
                .execute("return document.body.innerText.includes('Base histórica') && document.body.innerText.includes('Localidade: SC') && document.body.innerText.includes('HIST-30') && !document.body.innerText.includes('HIST-31');")
                .await?,
            true
        );
        browser.choose("select[name='page']", 1, "2").await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('page=2') && document.body.innerText.includes('HIST-31');",
                Duration::from_secs(5),
            )
            .await?;
        let edition_report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,historical:document.body.innerText.includes('Base histórica'),last:document.body.innerText.includes('HIST-31'),first:document.body.innerText.includes('HIST-01')};",
            )
            .await?;
        assert_eq!(edition_report["width"], width);
        assert_eq!(edition_report["historical"], true);
        assert_eq!(edition_report["last"], true);
        assert_eq!(edition_report["first"], false);
        assert!(edition_report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("catalog-edition-{width}.png")))
            .await?;

        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        browser
            .tab_to("details:has(input[name='before']) > summary")
            .await?;
        browser.key(ENTER).await?;
        browser
            .tab_to("form:has(input[name='before']) input[name='catalog_id']")
            .await?;
        for character in "historico-e2e".chars() {
            browser.key(&character.to_string()).await?;
        }
        browser.key(TAB).await?;
        browser.key("1").await?;
        browser.key(TAB).await?;
        browser.key("2").await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('before=1') && location.search.includes('after=2') && document.body.innerText.includes('HIST-30');",
                Duration::from_secs(5),
            )
            .await?;
    }

    // A memória deve mostrar a regra explícita e o preço histórico antes da conversão.
    h.auth
        .estimating_composition_write(
            &editor.session,
            serde_json::from_value(json!({
                "context": context.clone(),
                "scope_id": budget.clone(),
                "operation_key": uuid::Uuid::new_v4().to_string(),
                "expected_version": null,
                "reason": "Cadastro com conversao de unidade",
                "composition": {
                    "code": "CONV-E2E",
                    "description": "Servico com conversao explicita",
                    "unit": "un",
                    "cost_mode": {"mode":"formula"},
                    "lines": [{
                        "id": "insumo",
                        "source": {
                            "kind": "catalog",
                            "catalog_id": "historico-e2e",
                            "edition": 1,
                            "code": "HIST-01"
                        },
                        "coefficient": "3",
                        "unit": "un",
                        "conversion": {
                            "from": "m",
                            "to": "un",
                            "factor": "2",
                            "reference": "Peca de dois metros"
                        }
                    }],
                    "incidences": [],
                    "bdi": "0",
                    "discount": "0"
                }
            }))?,
        )
        .await?;

    // A consulta direta deve executar o servidor e manter a revisão da memória.
    h.auth
        .estimating_composition_write(
            &editor.session,
            serde_json::from_value(json!({
                "context": context.clone(),
                "scope_id": budget.clone(),
                "operation_key": uuid::Uuid::new_v4().to_string(),
                "expected_version": null,
                "reason": "Cadastro para ensaio da memoria",
                "composition": {
                    "code": "MEM-E2E",
                    "description": "Composicao de ensaio",
                    "unit": "un",
                    "cost_mode": {"mode":"informed","cost":"12.50","reason":"Pesquisa de ensaio"},
                    "lines": [{
                        "id": "referencia",
                        "source": {
                            "kind": "catalog",
                            "catalog_id": "historico-e2e",
                            "edition": 1,
                            "code": "HIST-01"
                        },
                        "coefficient": "3",
                        "unit": "un",
                        "conversion": {
                            "from": "m",
                            "to": "un",
                            "factor": "2",
                            "reference": "Referencia inativa de dois metros"
                        }
                    }],
                    "incidences": [],
                    "bdi": "0",
                    "discount": "0"
                }
            }))?,
        )
        .await?;
    assert!(
        h.auth
            .estimating_composition_memory(
                &editor.session,
                &context,
                &uuid::Uuid::new_v4().to_string(),
                "MEM-E2E",
            )
            .await
            .is_err()
    );
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        browser
            .tab_to("details:has(#composition-lookup) > summary")
            .await?;
        browser.key(ENTER).await?;
        browser
            .tab_to("#composition-lookup select[name='scope_id']")
            .await?;
        browser.key(TAB).await?;
        for character in "MEM-E2E".chars() {
            browser.key(&character.to_string()).await?;
        }
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('h1')?.textContent.includes('MEM-E2E');",
                Duration::from_secs(5),
            )
            .await?;
        let report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,revision:document.body.innerText.includes('revisão 1'),reason:document.body.innerText.includes('Pesquisa de ensaio'),conversion:document.body.innerText.includes('Referencia inativa de dois metros'),source:document.body.innerText.includes('HIST-01'),informed:document.body.innerText.includes('Custo unitário informado: R$ 12.500000'),inactive:document.body.innerText.includes('utilizadas para calcular o preço informado'),noCalculatedCost:!document.body.innerText.includes('parcela R$')};",
            )
            .await?;
        assert_eq!(report["width"], width);
        assert_eq!(report["revision"], true);
        assert_eq!(report["reason"], true);
        assert_eq!(report["conversion"], true);
        assert_eq!(report["source"], true);
        assert_eq!(report["informed"], true);
        assert_eq!(report["inactive"], true);
        assert_eq!(report["noCalculatedCost"], true);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("composition-memory-{width}.png")))
            .await?;

        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        browser
            .tab_to("details:has(#composition-lookup) > summary")
            .await?;
        browser.key(ENTER).await?;
        browser
            .tab_to("#composition-lookup select[name='scope_id']")
            .await?;
        browser.key(TAB).await?;
        for character in "CONV-E2E".chars() {
            browser.key(&character.to_string()).await?;
        }
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return document.querySelector('h1')?.textContent.includes('CONV-E2E');",
                Duration::from_secs(5),
            )
            .await?;
        let report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,source:document.body.innerText.includes('HIST-01'),rule:document.body.innerText.includes('Peca de dois metros'),original:document.body.innerText.includes('1.000000 / m'),converted:document.body.innerText.includes('2.000000'),total:document.body.innerText.includes('6.000000')};",
            )
            .await?;
        assert_eq!(report["width"], width);
        assert_eq!(report["source"], true);
        assert_eq!(report["rule"], true);
        assert_eq!(report["original"], true);
        assert_eq!(report["converted"], true);
        assert_eq!(report["total"], true);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(
                &h.evidence
                    .join(format!("composition-conversion-{width}.png")),
            )
            .await?;
    }

    // O ciclo precisa chegar ao usuário e não pode alterar a revisão persistida.
    let mut child = json!({
        "code": "CYCLE-E2E",
        "description": "Dependencia de ensaio",
        "unit": "un",
        "cost_mode": {"mode":"formula"},
        "lines": [{
            "id": "parent",
            "source": {"kind":"composition","code":"MEM-E2E"},
            "coefficient": "1",
            "unit": "un",
            "conversion": null
        }],
        "incidences": [],
        "bdi": "0",
        "discount": "0"
    });
    h.auth
        .estimating_composition_write(
            &editor.session,
            canteiro_contracts::estimating::CompositionWriteCommand {
                context: context.clone(),
                scope_id: budget.clone(),
                operation_key: uuid::Uuid::new_v4().to_string(),
                expected_version: None,
                reason: "Dependencia valida para ensaio de ciclo".into(),
                composition: serde_json::from_value(child.clone())?,
            },
        )
        .await?;
    child["code"] = json!("MEM-E2E");
    child["lines"][0]["source"] = json!({"kind":"composition","code":"CYCLE-E2E"});
    let error = h
        .auth
        .estimating_composition_calculate(
            &editor.session,
            &context,
            &budget,
            serde_json::from_value(child.clone())?,
        )
        .await
        .err()
        .ok_or("Previa aceitou composicao circular")?;
    assert!(
        error
            .to_string()
            .contains("MEM-E2E -> CYCLE-E2E -> MEM-E2E")
    );
    let error = h
        .auth
        .estimating_composition_write(
            &editor.session,
            canteiro_contracts::estimating::CompositionWriteCommand {
                context: context.clone(),
                scope_id: budget.clone(),
                operation_key: uuid::Uuid::new_v4().to_string(),
                expected_version: Some(1),
                reason: "Tentativa de criar ciclo".into(),
                composition: serde_json::from_value(child)?,
            },
        )
        .await
        .err()
        .ok_or("Gravacao aceitou composicao circular")?;
    assert!(
        error
            .to_string()
            .contains("MEM-E2E -> CYCLE-E2E -> MEM-E2E")
    );
    let (record, memory) = h
        .auth
        .estimating_composition_memory(&editor.session, &context, &budget, "MEM-E2E")
        .await?;
    assert_eq!(record.version, 1);
    assert!(memory.informed);

    // Stage a budget of 31 services and inspect the persisted batch, not a new upload.
    let actor = h
        .auth
        .access_context(&editor.session)
        .await?
        .state
        .current_user_id;
    let mut csv = String::from("CANTEIRO-ORCAMENTO-CSV-1\n");
    csv.push_str("tipo;orcamento;revisao;estado;responsavel;variacao;obra;escopo;grupo;etapa;subetapa;id;codigo;descricao;unidade;quantidade;preco;catalogo;edicao\n");
    let mut meta = vec![String::new(); 19];
    meta[0] = "META".into();
    meta[1] = budget.clone();
    meta[2] = "1".into();
    meta[3] = "draft".into();
    meta[4] = actor;
    csv.push_str(&format!("{}\n", meta.join(";")));
    let mut variation = vec![String::new(); 19];
    variation[0] = "VAR".into();
    variation[5] = "v1".into();
    variation[6] = work;
    variation[7] = "Obra de ensaio".into();
    csv.push_str(&format!("{}\n", variation.join(";")));
    for index in 1..=31 {
        let mut line = vec![String::new(); 19];
        line[0] = "ITEM".into();
        line[5] = "v1".into();
        line[8] = "Obra".into();
        line[9] = "Execucao".into();
        line[10] = "Servicos".into();
        line[11] = format!("import-{index:02}");
        line[12] = format!("IMP-{index:02}");
        line[13] = "Servico de ensaio".into();
        line[14] = "m".into();
        line[15] = "1".into();
        line[16] = "2".into();
        csv.push_str(&format!("{}\n", line.join(";")));
    }
    let mut schedule = vec![String::new(); 19];
    schedule[0] = "SCHEDULE".into();
    schedule[1] = "2026-11-01".into();
    schedule[2] = "89".into();
    schedule[3] = "fixed_30_days".into();
    csv.push_str(&format!("{}\n", schedule.join(";")));
    for (position, percentage) in ["33,34", "33,33", "33,33"].into_iter().enumerate() {
        let mut period = vec![String::new(); 19];
        period[0] = "PERIOD".into();
        period[5] = format!("periodo-{}", position + 1);
        period[6] = position.to_string();
        period[15] = percentage.into();
        csv.push_str(&format!("{}\n", period.join(";")));
    }
    let mut batch = String::new();
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        let script = format!(
            "const f=document.querySelector(\"form[action='/orcamentos/conferir-csv']\"); f.elements.scope_id.value={}; f.elements.csv.value={}; f.requestSubmit(); return true;",
            serde_json::to_string(&budget)?,
            serde_json::to_string(&csv)?
        );
        browser.execute(&script).await?;
        browser
            .wait(
                "return location.pathname==='/orcamentos/conferir-csv' && !!document.querySelector(\"form[action='/orcamentos/publicar-lote'] input[name='batch_id']\");",
                Duration::from_secs(10),
            )
            .await?;
        let report = browser
            .execute(
                r#"const f=document.querySelector("form[action='/orcamentos/publicar-lote']");
return {batch:f.elements.batch_id.value,hasFile:!!f.elements.csv,scroll:document.documentElement.scrollWidth};"#,
            )
            .await?;
        assert_eq!(report["hasFile"], false);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        let id = report["batch"]
            .as_str()
            .ok_or("Missing persisted preview")?;
        assert!(!uuid::Uuid::parse_str(id)?.is_nil());
        if batch.is_empty() {
            batch = id.to_owned();
        } else {
            assert_eq!(batch, id);
        }
        browser
            .tab_to("form[action='/orcamentos/publicar-lote'] input[name='reason']")
            .await?;
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute(
                    "return document.activeElement?.matches(\"form[action='/orcamentos/publicar-lote'] button[type='submit']\");",
                )
                .await?,
            true
        );
        browser
            .full_screenshot(
                &h.evidence
                    .join(format!("budget-import-preview-{width}.png")),
            )
            .await?;
    }
    let report_url = format!(
        "{}/orcamentos/importacao/diferencas?organization_id={}&context_version={}&scope_id={budget}&batch_id={batch}&page=1",
        h.app_url, context.organization_id, context.context_version
    );
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser.navigate(&report_url).await?;
        browser
            .wait(
                "return document.body.innerText.includes('IMP-30') && !document.body.innerText.includes('IMP-31');",
                Duration::from_secs(5),
            )
            .await?;
        browser.tab_to("input[name='page']").await?;
        browser.key("\u{e013}").await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('page=2') && document.body.innerText.includes('IMP-31');",
                Duration::from_secs(5),
            )
            .await?;
        let report = browser
            .execute(
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,first:document.body.innerText.includes('IMP-01')};",
            )
            .await?;
        assert_eq!(report["width"], width);
        assert_eq!(report["first"], false);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(
                &h.evidence
                    .join(format!("budget-import-details-{width}.png")),
            )
            .await?;
    }

    // Confirm only the persisted snapshot, never a client-provided revision.
    let lease_key = uuid::Uuid::new_v4().to_string();
    let request = BudgetLeaseCommand {
        context: context.clone(),
        scope_id: budget.clone(),
        lease_key: lease_key.clone(),
    };
    let unknown_batch = uuid::Uuid::new_v4().to_string();
    assert!(
        h.auth
            .estimating_budget_import_lease(&editor.session, request.clone(), &unknown_batch, None)
            .await
            .is_err()
    );
    assert!(
        h.auth
            .estimating_budget_import_lease(&editor.session, request.clone(), &batch, Some(1))
            .await
            .is_err()
    );
    let lease = h
        .auth
        .estimating_budget_import_lease(&editor.session, request, &batch, None)
        .await?;
    assert_eq!(lease.revision, 1);
    let command = BudgetCommand {
        context: context.clone(),
        scope_id: budget.clone(),
        operation_key: batch,
        expected_version: None,
        lease_key: Some(lease_key),
        reason: "Confirmacao do lote conferido".into(),
        change: BudgetChange::ApplyImport {},
    };
    let mut forged = command.clone();
    forged.operation_key = uuid::Uuid::new_v4().to_string();
    assert!(
        h.auth
            .estimating_budget_execute(&editor.session, forged)
            .await
            .is_err()
    );
    let saved = h
        .auth
        .estimating_budget_execute(&editor.session, command.clone())
        .await?;
    assert_eq!(saved.budget.version, 1);
    assert_eq!(saved.budget.snapshot.lines.len(), 31);
    let schedule = saved
        .budget
        .snapshot
        .schedule
        .as_ref()
        .ok_or("CSV confirmation lost the schedule")?;
    assert_eq!(schedule.deadline_days, 89);
    assert_eq!(schedule.periods.len(), 3);
    assert!(!saved.repeated);
    let replay = h
        .auth
        .estimating_budget_execute(&editor.session, command)
        .await?;
    assert!(replay.repeated);
    assert_eq!(replay.budget.operation_key, saved.budget.operation_key);
    let no_longer_ready = h
        .auth
        .estimating_budget_import_details(
            &editor.session,
            &context,
            &budget,
            &saved.budget.operation_key,
            1,
        )
        .await;
    assert!(no_longer_ready.is_err());

    // Download, preview and confirm the official workbook through the native forms.
    for (width, base_version) in [(360, 1), (1280, 2)] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        let current = h.auth.estimating_context(&editor.session).await?;
        let record = current
            .budgets
            .iter()
            .find(|record| record.scope_id == budget)
            .ok_or("Committed budget missing from the authorized context")?;
        assert_eq!(record.version, base_version);
        assert_eq!(record.snapshot.schedule, saved.budget.snapshot.schedule);
        record.snapshot.summarize()?;
        let page_state = browser
            .execute(
                r#"return {path:location.pathname,type:document.contentType,
heading:document.querySelector("h1")?.textContent,
schedule:!!document.querySelector("details.budget-schedule-preview > summary"),
text:document.body.innerText.slice(0,400)};"#,
            )
            .await?;
        if page_state["schedule"] != true {
            browser
                .full_screenshot(
                    &h.evidence
                        .join(format!("budget-schedule-missing-{width}.png")),
                )
                .await?;
            return Err(format!("Schedule not rendered at width {width}: {page_state}").into());
        }
        browser
            .tab_to("details.budget-schedule-preview > summary")
            .await?;
        browser.key(ENTER).await?;
        assert_eq!(
            browser
                .execute(
                    r#"const d=document.querySelector("details.budget-schedule-preview");
return d.open && d.innerText.includes("89 dias") && d.innerText.includes("Período 1: 33.34% · R$ 20.67 · até 2026-12-01 (30 dias)") && d.innerText.includes("Período 3: 33.33% · R$ 20.66 · até 2027-01-29 (29 dias, período residual)");"#,
                )
                .await?,
            true
        );
        assert!(
            browser
                .execute("return document.documentElement.scrollWidth;")
                .await?
                .as_u64()
                .unwrap_or(u64::MAX)
                <= u64::from(width)
        );
        browser
            .tab_to("form[action='/orcamentos/cronograma/consultar'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname==='/orcamentos/cronograma/consultar';",
                Duration::from_secs(10),
            )
            .await?;
        assert_eq!(
            browser
                .execute(
                    r#"const f=document.querySelector("form[action='/orcamentos/cronograma/consultar']");
const d=document.querySelector("details.budget-schedule-preview");
return !!f && d.open && d.innerText.includes("Página 1 de 1") && d.innerText.includes("Período 3: 33.33% · R$ 20.66");"#,
                )
                .await?,
            true
        );
        browser
            .full_screenshot(&h.evidence.join(format!("budget-schedule-{width}.png")))
            .await?;
        assert_eq!(
            browser
                .execute(
                    r#"const url=new URL(location.href);
url.searchParams.set("version","0");
return fetch(url).then(r=>r.status);"#,
                )
                .await?,
            400
        );
        assert_eq!(
            browser
                .execute(
                    r#"const url=new URL(location.href);
url.searchParams.set("scope_id","00000000-0000-0000-0000-000000000000");
return fetch(url).then(r=>r.status);"#,
                )
                .await?,
            400
        );
        assert!(
            browser
                .execute("return document.documentElement.scrollWidth;")
                .await?
                .as_u64()
                .unwrap_or(u64::MAX)
                <= u64::from(width)
        );
        browser
            .full_screenshot(&h.evidence.join(format!("budget-schedule-page-{width}.png")))
            .await?;
        browser.tab_to("a[href='/orcamentos']").await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname==='/orcamentos';",
                Duration::from_secs(10),
            )
            .await?;
        let script = format!(
            r#"const f=document.querySelector("form[action='/orcamentos/conferir-xlsx']");
f.elements.scope_id.value={};
const link=document.querySelector("a[href*='/export.xlsx']");
return fetch(link.href).then(async response => {{
 if (!response.ok) throw new Error("XLSX download failed");
 const transfer=new DataTransfer();
 transfer.items.add(new File([await response.blob()], "orcamento.xlsx", {{
  type:"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
 }}));
 f.elements.xlsx.files=transfer.files;
 return f.elements.xlsx.files.length;
}});"#,
            serde_json::to_string(&budget)?
        );
        assert_eq!(browser.execute(&script).await?, 1);
        browser
            .tab_to("form[action='/orcamentos/conferir-xlsx'] input[type='file']")
            .await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname==='/orcamentos/conferir-xlsx' && !!document.querySelector(\"form[action='/orcamentos/publicar-lote'] input[name='batch_id']\");",
                Duration::from_secs(10),
            )
            .await?;
        let report = browser
            .execute(
                r#"const f=document.querySelector("form[action='/orcamentos/publicar-lote']");
return {batch:f.elements.batch_id.value,version:f.elements.expected_version.value,hasFile:!!f.elements.xlsx||!!f.elements.csv,scroll:document.documentElement.scrollWidth,format:document.body.innerText.includes('CANTEIRO-ORCAMENTO-XLSX-1')};"#,
            )
            .await?;
        assert_eq!(report["version"], base_version.to_string());
        assert_eq!(report["hasFile"], false);
        assert_eq!(report["format"], true);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        let id = report["batch"].as_str().ok_or("Missing XLSX batch")?;
        assert!(!uuid::Uuid::parse_str(id)?.is_nil());
        browser
            .full_screenshot(&h.evidence.join(format!("budget-xlsx-preview-{width}.png")))
            .await?;

        // Before confirming, the preview must not have changed the current revision.
        let before = h.auth.estimating_context(&editor.session).await?;
        let previous = before
            .budgets
            .iter()
            .find(|record| record.scope_id == budget)
            .ok_or("Missing current budget before XLSX confirmation")?;
        assert_eq!(previous.version, base_version);
        browser
            .tab_to("form[action='/orcamentos/publicar-lote'] input[name='reason']")
            .await?;
        for character in "Reimportacao XLSX".chars() {
            browser.key(&character.to_string()).await?;
        }
        browser.key(TAB).await?;
        assert_eq!(
            browser
                .execute("return document.activeElement?.matches(\"form[action='/orcamentos/publicar-lote'] button[type='submit']\");")
                .await?,
            true
        );
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname==='/orcamentos/publicar-lote' && document.querySelector('h1')?.textContent.trim()==='Importação registrada';",
                Duration::from_secs(10),
            )
            .await?;
        let after = h.auth.estimating_context(&editor.session).await?;
        let persisted = after
            .budgets
            .iter()
            .find(|record| record.scope_id == budget)
            .ok_or("Missing committed XLSX revision")?;
        assert_eq!(persisted.version, base_version + 1);
        assert_eq!(persisted.operation_key, id);
        assert_eq!(persisted.reason, "Reimportacao XLSX");
        let mut expected = previous.snapshot.clone();
        expected.revision = u32::try_from(base_version + 1)?;
        assert_eq!(persisted.snapshot, expected);
        assert!(
            h.auth
                .estimating_budget_import_details(&editor.session, &context, &budget, id, 1)
                .await
                .is_err()
        );
        assert!(
            browser
                .execute("return document.documentElement.scrollWidth;")
                .await?
                .as_u64()
                .unwrap_or(u64::MAX)
                <= u64::from(width)
        );
        browser
            .full_screenshot(
                &h.evidence
                    .join(format!("budget-xlsx-published-{width}.png")),
            )
            .await?;
    }

    // A longer schedule must expose its final period without truncation.
    browser
        .navigate(&format!("{}/orcamentos", h.app_url))
        .await?;
    browser
        .execute(
            r#"const f=document.querySelector("form[action='/orcamentos/cronograma/salvar']");
f.closest("details").open=true;
f.elements.deadline_days.value="389";
f.elements.periods.value="13";
f.elements.reason.value="Cronograma com treze periodos";
f.elements.confirm_schedule.checked=true;
f.requestSubmit();
return true;"#,
        )
        .await?;
    browser
        .wait(
            "return location.pathname==='/orcamentos/cronograma/salvar' && document.body.innerText.includes('Revisão 4');",
            Duration::from_secs(10),
        )
        .await?;
    let revised = h.auth.estimating_context(&editor.session).await?;
    let revised_budget = revised
        .budgets
        .iter()
        .find(|record| record.scope_id == budget)
        .ok_or("Missing revised budget")?;
    assert_eq!(revised_budget.version, 4);
    let revised_schedule = revised_budget
        .snapshot
        .schedule
        .as_ref()
        .ok_or("Rescheduling removed the schedule")?;
    assert_eq!(revised_schedule.deadline_days, 389);
    assert_eq!(revised_schedule.periods.len(), 13);
    assert_eq!(revised_schedule.calendar_periods()?.len(), 13);
    let scoped = h
        .auth
        .estimating_budget_schedule(&editor.session, &context, &budget, 4)
        .await?
        .ok_or("Missing current schedule in scoped query")?;
    assert_eq!(scoped.snapshot, revised_budget.snapshot);
    assert!(
        h.auth
            .estimating_budget_schedule(&editor.session, &context, &budget, 3)
            .await?
            .is_none()
    );
    let mut stale_context = context.clone();
    stale_context.context_version += 1;
    assert!(
        h.auth
            .estimating_budget_schedule(&editor.session, &stale_context, &budget, 4)
            .await?
            .is_none()
    );
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/orcamentos", h.app_url))
            .await?;
        let state = browser
            .execute(
                r#"return {status:performance.getEntriesByType("navigation")[0]?.responseStatus,
type:document.contentType,path:location.pathname,
schedule:!!document.querySelector("details.budget-schedule-preview > summary"),
text:document.body.innerText.slice(0,500)};"#,
            )
            .await?;
        if state["schedule"] != true {
            browser
                .full_screenshot(
                    &h.evidence
                        .join(format!("budget-long-schedule-missing-{width}.png")),
                )
                .await?;
            return Err(format!("Missing schedule at width {width}: {state}").into());
        }
        browser
            .tab_to("details.budget-schedule-preview > summary")
            .await?;
        if browser
            .execute("return document.querySelector('details.budget-schedule-preview').open;")
            .await?
            != true
        {
            browser.key(ENTER).await?;
        }
        let query = browser
            .execute(
                r#"const f=document.querySelector("form[action='/orcamentos/cronograma/consultar']");
return {scope:f.elements.scope_id.value,version:f.elements.version.value,page:f.elements.page.value,open:f.closest("details").open};"#,
            )
            .await?;
        assert_eq!(query["scope"], budget.as_str());
        assert_eq!(query["version"], "4");
        assert_eq!(query["page"], "1");
        assert_eq!(query["open"], true);
        browser
            .tab_to("form[action='/orcamentos/cronograma/consultar'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        if let Err(error) = browser
            .wait(
                "return location.pathname==='/orcamentos/cronograma/consultar' && document.body.innerText.includes('Página 1 de 2');",
                Duration::from_secs(10),
            )
            .await
        {
            let state = browser
                .execute(
                    r#"const d=document.querySelector("details.budget-schedule-preview");
return {path:location.pathname,version:new URLSearchParams(location.search).get("version"),page:new URLSearchParams(location.search).get("page"),heading:document.querySelector("h1")?.textContent,open:d?.open,schedule:d?.textContent?.slice(0,1200),text:document.body.innerText.slice(0,500)};"#,
                )
                .await?;
            browser
                .full_screenshot(
                    &h.evidence
                        .join(format!("budget-long-schedule-failure-{width}.png")),
                )
                .await?;
            return Err(format!("{error}; width {width}; schedule page: {state}").into());
        }
        assert_eq!(
            browser
                .execute(
                    r#"const d=document.querySelector("details.budget-schedule-preview");
return d.innerText.includes("Período 12:") && !d.innerText.includes("Período 13:");"#,
                )
                .await?,
            true
        );
        assert_eq!(
            browser
                .execute(
                    r#"const nav=document.querySelector('nav[aria-label="Navegação do cronograma"]');
return !!nav && !nav.querySelector('a[rel="prev"]') && new URL(nav.querySelector('a[rel="next"]').href).searchParams.get('page')==='2';"#,
                )
                .await?,
            true
        );
        browser.tab_to("input[name='page']").await?;
        browser.key("\u{e013}").await?;
        browser.key(TAB).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('page=2') && document.body.innerText.includes('Página 2 de 2');",
                Duration::from_secs(5),
            )
            .await?;
        assert_eq!(
            browser
                .execute(
                    r#"const d=document.querySelector("details.budget-schedule-preview");
return d.innerText.includes("Período 13:") && d.innerText.includes("29 dias, período residual") && !d.innerText.includes("Período 12:") && d.querySelectorAll("li").length===1;"#,
                )
                .await?,
            true
        );
        assert!(
            browser
                .execute("return document.documentElement.scrollWidth;")
                .await?
                .as_u64()
                .unwrap_or(u64::MAX)
                <= u64::from(width)
        );
        browser
            .full_screenshot(
                &h.evidence
                    .join(format!("budget-schedule-page-two-{width}.png")),
            )
            .await?;
        assert_eq!(
            browser
                .execute(
                    r#"const nav=document.querySelector('nav[aria-label="Navegação do cronograma"]');
return !!nav && !nav.querySelector('a[rel="next"]') && new URL(nav.querySelector('a[rel="prev"]').href).searchParams.get('page')==='1';"#,
                )
                .await?,
            true
        );
        browser.tab_to("a[rel='prev']").await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('page=1') && document.body.innerText.includes('Página 1 de 2');",
                Duration::from_secs(10),
            )
            .await?;
        browser.tab_to("a[rel='next']").await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.search.includes('page=2') && document.body.innerText.includes('Página 2 de 2');",
                Duration::from_secs(10),
            )
            .await?;
    }
    let after_navigation = h.auth.estimating_context(&editor.session).await?;
    let unchanged_budget = after_navigation
        .budgets
        .iter()
        .find(|record| record.scope_id == budget)
        .ok_or("Missing budget after schedule navigation")?;
    assert_eq!(unchanged_budget.version, revised_budget.version);
    assert_eq!(unchanged_budget.snapshot, revised_budget.snapshot);

    write_json(
        &h.evidence.join("estimating-interface.json"),
        &json!({
            "status":"passed",
            "checks":[
                "authorized_budget_scope",
                "real_browser_360_1280",
                "no_horizontal_overflow",
                "labelled_import_controls",
                "budget_and_catalog_imports_visible",
                "tab_order_budget_csv_preview",
                "tab_order_catalog_csv_preview",
                "historical_catalog_page_two_keyboard",
                "historical_catalog_edition_page_two_keyboard",
                "historical_catalog_explicit_edition_keyboard",
                "budget_import_page_two_keyboard",
                "persisted_batch_confirmation_and_replay",
                "financial_schedule_keyboard_and_exact_amounts_360_1280",
                "financial_schedule_page_two_keyboard_and_scope_revision_rejection",
                "xlsx_download_native_upload_keyboard_preview",
                "xlsx_keyboard_confirmation_persisted_revision_and_consumed_batch"
            ],
            "viewports":viewports,
            "limitations":"Viewport emulation and keyboard checks, not physical device or full accessibility certification."
        }),
    )?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires built SSR/WASM server, PostgreSQL and WebDriver"]
async fn technical_notice_opens_exact_record_and_revoked_link_reveals_nothing() -> TestResult {
    use canteiro_contracts::obligations::{ObligationCommand, ObligationKind, ObligationSpec};

    let h = Harness::start("notice-destination").await?;
    let owner = account(&h, "notice-own", true).await?;
    let editor = account(&h, "notice-ed", false).await?;
    let company_id = company(&h, &owner, "Empresa de ensaio de avisos").await?;
    join(&h, &owner, &editor, &company_id).await?;
    let work = create_scope(&h, &owner, ScopeKind::Work, "Obra de avisos", None).await?;
    fixture_grant(
        &h,
        &owner,
        &editor,
        &work,
        AccessRole::TechnicalResponsible,
        vec![AccessAction::View, AccessAction::ManageTechnicalObligations],
    )
    .await?;
    let ctx = h.auth.obligations_context(&editor.session).await?;
    let description = "Licenca reservada para conferencia de aviso";
    let spec: ObligationSpec = serde_json::from_value(json!({
        "kind":"license",
        "activity":"execution",
        "discipline":"Civil",
        "description":description,
        "professional_id":null,
        "number":"",
        "issuer":"",
        "valid_from":"2026-01-01",
        "valid_until":"2027-01-01",
        "activity_on":"2026-11-01",
        "registered_on":null,
        "rrt_timing":null,
        "document_id":null,
        "document_version":null,
        "exception":null,
        "replaces_id":null
    }))?;
    assert_eq!(spec.kind, ObligationKind::License);
    let command = ObligationCommand {
        context: ctx.context.ok_or("Missing obligation context")?,
        operation_key: uuid::Uuid::new_v4().to_string(),
        scope_id: work.clone(),
        obligation_id: None,
        expected_version: None,
        reason: "Conferencia documental de ensaio".into(),
        spec,
        confirm: false,
    };
    let saved = h
        .auth
        .obligations_execute(&editor.session, command.clone())
        .await?;
    let repeated = h.auth.obligations_execute(&editor.session, command).await?;
    assert!(repeated.repeated);
    assert_eq!(saved.obligation.id, repeated.obligation.id);
    let destination = format!("/obrigacoes?obligation={}", saved.obligation.id);
    let notices = h.auth.notices_context(&editor.session).await?;
    assert_eq!(notices.notices.len(), 1);
    assert_eq!(notices.notices[0].destination, destination);

    let browser = Browser::start(&h).await?;
    browser.navigate(&h.app_url).await?;
    browser
        .post(
            "cookie",
            json!({"cookie":{
                "name":"__Host-canteiro-session",
                "value":editor.session,
                "path":"/",
                "secure":true,
                "httpOnly":true,
                "sameSite":"Strict"
            }}),
        )
        .await?;
    for width in [360, 1280] {
        browser.viewport(width, 900, width == 360).await?;
        browser
            .navigate(&format!("{}/pendencias", h.app_url))
            .await?;
        let report = browser
            .execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth};")
            .await?;
        assert_eq!(report["width"], width);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("notice-destination-{width}.png")))
            .await?;
        assert_eq!(
            browser
                .execute("return document.querySelectorAll('[data-notice-day]').length;")
                .await?,
            1
        );
        browser.tab_to("input[name='group_by_day']").await?;
        browser.key("\u{e00d}").await?;
        browser
            .tab_to("form[action='/pendencias/preferencias'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname === '/pendencias' && !document.querySelector('input[name=group_by_day]').checked && !document.querySelector('[data-notice-day]');",
                Duration::from_secs(5),
            )
            .await?;
        browser.tab_to("input[name='group_by_day']").await?;
        browser.key("\u{e00d}").await?;
        browser
            .tab_to("form[action='/pendencias/preferencias'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname === '/pendencias' && document.querySelector('input[name=group_by_day]').checked && !!document.querySelector('[data-notice-day]');",
                Duration::from_secs(5),
            )
            .await?;
        let saved_preferences = h.auth.notices_context(&editor.session).await?.preferences;
        browser.tab_to("input[name='reminder_days']").await?;
        browser.key("\u{e010}").await?;
        browser.key("x").await?;
        browser
            .tab_to("form[action='/pendencias/preferencias'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return !!document.querySelector('[role=alert]') && document.querySelector('input[name=reminder_days]')?.value.endsWith('x');",
                Duration::from_secs(5),
            )
            .await?;
        assert_eq!(
            h.auth.notices_context(&editor.session).await?.preferences,
            saved_preferences
        );
        browser
            .full_screenshot(&h.evidence.join(format!("notice-preferences-error-{width}.png")))
            .await?;
        browser.tab_to("input[name='reminder_days']").await?;
        browser.key("\u{e010}").await?;
        browser.key("\u{e003}").await?;
        browser
            .tab_to("form[action='/pendencias/preferencias'] button[type='submit']")
            .await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname === '/pendencias' && !document.querySelector('[role=alert]');",
                Duration::from_secs(5),
            )
            .await?;
        browser.tab_to(&format!("a[href='{destination}']")).await?;
        browser.key(ENTER).await?;
        browser
            .wait(
                "return location.pathname === '/obrigacoes' && location.search.includes('obligation=') && document.body.innerText.includes('Licenca reservada para conferencia de aviso');",
                Duration::from_secs(5),
            )
            .await?;
    }
    let unchanged = h.auth.obligations_context(&editor.session).await?;
    assert!(!unchanged.obligations[0].confirmed);
    assert_eq!(unchanged.obligations[0].version, saved.obligation.version);

    let state = h.auth.access_context(&owner.session).await?.state;
    let grant = state
        .grants
        .iter()
        .find(|g| g.scope_id == work && g.user_email == editor.email)
        .ok_or("Missing technical grant")?;
    h.auth
        .access_execute(
            &owner.session,
            AccessCommand::RevokeGrant {
                context: scope(&state),
                grant_id: grant.id.clone(),
                grant_version: grant.version,
            },
        )
        .await?;
    assert!(
        h.auth
            .notices_context(&editor.session)
            .await?
            .notices
            .is_empty()
    );
    browser
        .navigate(&format!("{}{}", h.app_url, destination))
        .await?;
    assert_eq!(
        browser
            .execute(
                "return document.body.innerText.includes('Licenca reservada para conferencia de aviso');",
            )
            .await?,
        false
    );
    write_json(
        &h.evidence.join("notice-destination.json"),
        &json!({
            "status":"passed",
            "checks":[
                "idempotent_obligation_event",
                "exact_destination_keyboard_360_1280",
                "persisted_grouping_toggle_keyboard_360_1280",
                "get_does_not_confirm",
                "revoked_scope_hides_notice_and_target"
            ]
        }),
    )?;
    Ok(())
}
