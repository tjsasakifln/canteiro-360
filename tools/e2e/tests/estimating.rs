#[allow(dead_code)]
mod support;

use canteiro_contracts::access::{AccessAction, AccessCommand, AccessRole, ScopeKind};
use canteiro_contracts::estimating::{CatalogCsvImportCommand, CostSource, NumberLocale};
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
        Some(work),
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
                competence: "2026-11".into(),
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
                .execute("return document.body.innerText.includes('HIST-30') && !document.body.innerText.includes('HIST-31');")
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
                "return {width:innerWidth,scroll:document.documentElement.scrollWidth,last:document.body.innerText.includes('HIST-31'),first:document.body.innerText.includes('HIST-01')};",
            )
            .await?;
        assert_eq!(report["width"], width);
        assert_eq!(report["last"], true);
        assert_eq!(report["first"], false);
        assert!(report["scroll"].as_u64().unwrap_or(u64::MAX) <= u64::from(width));
        browser
            .full_screenshot(&h.evidence.join(format!("catalog-history-{width}.png")))
            .await?;
    }

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
                "historical_catalog_page_two_keyboard"
            ],
            "viewports":viewports,
            "limitations":"Viewport emulation and keyboard checks, not physical device or full accessibility certification."
        }),
    )?;
    Ok(())
}
