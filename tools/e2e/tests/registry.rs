//! Jornadas das issues 10 a 14. O scanner abaixo é um simulador de protocolo,
//! identificado na evidência; não representa uma base real de antimalware.
#[allow(dead_code)]
mod support;

use base64::Engine;
use canteiro_contracts::{access::*, documents::DocumentScope};
use serde_json::{Value, json};
use std::{fs, net::SocketAddr, time::Duration};
use support::{
    Browser, ENTER, Harness, TestResult,
    access_fixture::{AccountFixture, account, company, fixture_grant, join, scope},
    write_json,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use uuid::Uuid;

const WAIT: Duration = Duration::from_secs(30);
const CONTENT: &str = "Registro fictício de vistoria. Nenhum dado real de cliente.\n";

struct ScannerFixture {
    address: SocketAddr,
    task: JoinHandle<()>,
}
impl ScannerFixture {
    async fn start() -> TestResult<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let result: Result<(), std::io::Error> = async {
                        let mut command = [0u8; 10];
                        socket.read_exact(&mut command).await?;
                        assert_eq!(&command, b"zINSTREAM\0");
                        let mut bytes = Vec::new();
                        loop {
                            let count = socket.read_u32().await? as usize;
                            if count == 0 {
                                break;
                            }
                            assert!(count <= 65_536 && bytes.len() + count <= 5 * 1024 * 1024);
                            let start = bytes.len();
                            bytes.resize(start + count, 0);
                            socket.read_exact(&mut bytes[start..]).await?;
                        }
                        let reply: &[u8] = if bytes.windows(14).any(|v| v == b"FIXTURE_REJECT") {
                            b"stream: Canteiro-Test FOUND\0"
                        } else {
                            b"stream: OK\0"
                        };
                        socket.write_all(reply).await?;
                        Ok(())
                    }
                    .await;
                    assert!(result.is_ok(), "fixture scanner transport: {result:?}");
                });
            }
        });
        Ok(Self { address, task })
    }
}
impl Drop for ScannerFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn create_scope(h: &Harness, owner: &AccountFixture, name: &str) -> TestResult<String> {
    let state = h.auth.access_context(&owner.session).await?.state;
    let response = h
        .auth
        .access_execute(
            &owner.session,
            AccessCommand::CreateScope {
                context: scope(&state),
                kind: ScopeKind::Work,
                name: name.into(),
                parent_id: None,
            },
        )
        .await?;
    Ok(response
        .state
        .scopes
        .into_iter()
        .find(|s| s.name == name)
        .ok_or("missing scope")?
        .id)
}

async fn post(
    h: &Harness,
    who: &AccountFixture,
    path: &str,
    body: Value,
) -> TestResult<(u16, Value)> {
    let csrf = h.auth.documents_context(&who.session).await?.csrf;
    let response = h
        .http
        .post(format!("{}{path}", h.app_url))
        .header("Cookie", format!("__Host-canteiro-session={}", who.session))
        .header("Origin", &h.app_url)
        .header("X-CSRF-Token", csrf)
        .json(&body)
        .send()
        .await?;
    let status = response.status().as_u16();
    Ok((status, response.json().await?))
}

async fn set_cookie(h: &Harness, b: &Browser, who: &AccountFixture) -> TestResult {
    b.navigate(&h.app_url).await?;
    b.post(
        "cookie",
        json!({"cookie":{"name":"__Host-canteiro-session","value":who.session,
        "path":"/","secure":true,"httpOnly":true,"sameSite":"Strict"}}),
    )
    .await?;
    Ok(())
}

async fn capture(h: &Harness, b: &Browser, route: &str, selector: &str) -> TestResult<Vec<Value>> {
    let mut records = Vec::new();
    for width in [360u32, 1280] {
        b.viewport(width, 900, width == 360).await?;
        b.wait(
            &format!(
                "return !!document.querySelector({});",
                serde_json::to_string(selector)?
            ),
            WAIT,
        )
        .await?;
        let geometry = b.execute("return {width:innerWidth,scroll:document.documentElement.scrollWidth,headings:document.querySelectorAll('h1').length};").await?;
        assert!(
            geometry["scroll"].as_u64().unwrap() <= u64::from(width),
            "page overflow: {route} {geometry}"
        );
        assert_eq!(geometry["headings"], 1);
        b.full_screenshot(&h.evidence.join(format!("registry-{route}-{width}.png")))
            .await?;
        records.push(json!({"route":route,"width":width,"geometry":geometry}));
    }
    Ok(records)
}

#[tokio::test]
#[ignore = "requer PostgreSQL, servidor, WASM e Chrome/ChromeDriver fixados"]
async fn registry_documents_and_projects_work_with_real_authorization_and_keyboard() -> TestResult {
    let scanner = ScannerFixture::start().await?;
    let h = Harness::start_with_document_scanner(
        "registry-interface",
        Some(&scanner.address.to_string()),
    )
    .await?;
    let owner = account(&h, "regown", true).await?;
    let reader = account(&h, "regread", false).await?;
    let org = company(&h, &owner, "Construtora Vereda Fictícia").await?;
    join(&h, &owner, &reader, &org).await?;
    let work = create_scope(&h, &owner, "Obra de teste Vereda").await?;
    fixture_grant(
        &h,
        &owner,
        &owner,
        &work,
        AccessRole::Manager,
        vec![
            AccessAction::View,
            AccessAction::ManagePortfolio,
            AccessAction::ManageTechnicalObligations,
        ],
    )
    .await?;
    let b = Browser::start(&h).await?;
    set_cookie(&h, &b, &owner).await?;
    let result = journey(&h, &b, &owner, &reader, &work).await;
    if result.is_err() {
        let _ = b
            .full_screenshot(&h.evidence.join("registry-failure.png"))
            .await;
        if let Ok(text) = b.execute("return document.body.innerText;").await {
            let _ = write_json(&h.evidence.join("registry-failure-text.json"), &text);
        }
        if let Ok(logs) = b.logs().await {
            let _ = write_json(&h.evidence.join("registry-failure-browser.json"), &logs);
        }
    }
    let close = b.close().await;
    result?;
    close
}

async fn journey(
    h: &Harness,
    b: &Browser,
    owner: &AccountFixture,
    reader: &AccountFixture,
    work: &str,
) -> TestResult {
    let mut layouts = Vec::new();
    b.navigate(&format!("{}/cadastros", h.app_url)).await?;
    b.wait("return document.querySelector('#party-name') && !document.querySelector('#party-name').matches(':disabled');", WAIT).await?;
    b.fill("#party-name", "Empresa do vetor oficial").await?;
    b.fill("#party-tax-id", "12ABC34501DE35").await?;
    b.click("details:has(#party-add-contact) summary").await?;
    b.click("#party-add-contact").await?;
    b.fill("#contact-name-0", "Contato de ensaio").await?;
    b.fill("#contact-email-0", "contato@example.test").await?;
    b.tab_to("form:has(#party-name) button[type='submit']")
        .await?;
    b.key(ENTER).await?;
    b.wait("return document.querySelector('.registry-list')?.textContent.includes('Empresa do vetor oficial');", WAIT).await?;
    let parties = h
        .auth
        .parties_context(&owner.session, "Empresa do vetor oficial", 0)
        .await?;
    assert_eq!(parties.rows.len(), 1);
    assert_eq!(
        parties.rows[0].input.tax_id.as_deref(),
        Some("12ABC34501DE35")
    );
    assert_eq!(parties.rows[0].input.contacts.len(), 1);
    assert_eq!(
        parties.rows[0].input.contacts[0].email,
        "contato@example.test"
    );
    b.fill("#party-name", "Rascunho preservado").await?;
    b.fill("#party-tax-id", "00000000000000").await?;
    b.click("form:has(#party-name) button[type='submit']")
        .await?;
    b.wait(
        "return document.querySelector('#registry-message')?.textContent.includes('caracteres ou dígitos verificadores inválidos');",
        WAIT,
    )
    .await?;
    assert_eq!(
        b.property("#party-name", "value").await?,
        "Rascunho preservado"
    );
    assert_eq!(
        b.property("#party-tax-id", "value").await?,
        "00000000000000"
    );
    assert_eq!(
        h.auth
            .parties_context(&owner.session, "Rascunho preservado", 0)
            .await?
            .total,
        0
    );
    layouts.extend(capture(h, b, "cadastros", "#parties-title").await?);

    b.navigate(&format!("{}/carteira", h.app_url)).await?;
    b.wait(
        "return document.querySelector('#work-reference')?.options.length > 1;",
        WAIT,
    )
    .await?;
    b.click(&format!("#work-reference option[value='{work}']"))
        .await?;
    b.fill("#work-title", "Obra de teste Vereda").await?;
    b.click("details:has(#work-latitude) summary").await?;
    b.fill("#work-latitude", "-27,5954").await?;
    b.fill("#work-longitude", "-48,5480").await?;
    b.fill("#work-dimension", "100,0000").await?;
    b.fill("#work-localities", "Florianópolis;SC;100,00;100,0000")
        .await?;
    b.tab_to("form:has(#work-title) button[type='submit']")
        .await?;
    b.key(ENTER).await?;
    b.wait("return document.querySelector('#work-list-title')?.parentElement.textContent.includes('Obra de teste Vereda');", WAIT).await?;
    assert_eq!(
        h.auth
            .portfolio_context(&owner.session, "", None)
            .await?
            .rows
            .len(),
        1
    );
    assert_eq!(
        h.auth
            .portfolio_context(&owner.session, "", None)
            .await?
            .located,
        1
    );
    assert!(
        h.auth
            .portfolio_context(&reader.session, "", None)
            .await?
            .rows
            .is_empty()
    );
    b.wait("return document.querySelector('.registry-map circle')?.getAttribute('cx') === '131.452' && document.querySelector('.registry-map circle')?.getAttribute('cy') === '117.5954';", WAIT).await?;
    b.fill("#work-title", "Obra de teste Vereda revisada")
        .await?;
    b.fill("#work-reason", "Conferência do título da obra")
        .await?;
    b.tab_to("form:has(#work-title) button[type='submit']")
        .await?;
    b.key(ENTER).await?;
    b.wait("return document.querySelector('#work-list-title')?.parentElement.textContent.includes('Vereda revisada') && document.querySelector('#work-list-title')?.parentElement.textContent.includes('versão 2');", WAIT).await?;
    assert_eq!(
        h.auth
            .portfolio_context(&owner.session, "", None)
            .await?
            .rows[0]
            .version,
        2
    );
    layouts.extend(capture(h, b, "carteira", "#portfolio-title").await?);

    b.click(".workspace-navigation a[href='/projetos']").await?;
    b.wait(
        "return !!document.querySelector(\"form[action='/projetos/acao']\");",
        WAIT,
    )
    .await?;
    b.fill("input[name='title']", "Projeto antes da obra")
        .await?;
    b.fill("input[name='discipline']", "Estruturas").await?;
    b.fill(
        "textarea[name='objective']",
        "Definir a estrutura de um empreendimento fictício.",
    )
    .await?;
    b.fill(
        "textarea[name='scope']",
        "Estudo preliminar e critérios de aceite, sem obra ou contrato vinculado.",
    )
    .await?;
    b.tab_to("form[action='/projetos/acao'] button[type='submit']")
        .await?;
    b.key(ENTER).await?;
    b.wait("return document.body.textContent.includes('Projeto antes da obra') && document.body.textContent.includes('Em elaboração');", WAIT).await?;
    let projects = h.auth.projects_context(&owner.session).await?;
    let project = projects
        .projects
        .iter()
        .find(|p| p.snapshot.spec.title == "Projeto antes da obra")
        .ok_or("project not saved through form")?;
    assert!(project.snapshot.spec.work_id.is_none());
    assert!(!project.can_approve);
    let (status, denied) = post(
        h,
        owner,
        "/api/projects/command",
        json!({
            "context": projects.context,
            "operation_key": Uuid::new_v4().to_string(),
            "project_id": project.id,
            "expected_version": project.version,
            "reason": "Tentativa de aprovação sem o ato específico",
            "change": { "action": "approve" }
        }),
    )
    .await?;
    assert_eq!(status, 403, "{denied}");
    assert_eq!(
        h.auth.projects_context(&owner.session).await?.projects[0].version,
        project.version
    );
    layouts.extend(capture(h, b, "projetos", "h1").await?);

    b.navigate(&format!("{}/documentos", h.app_url)).await?;
    b.wait(
        "return document.querySelector('#documents-scope')?.options.length > 1;",
        WAIT,
    )
    .await?;
    b.click(&format!("#documents-scope option[value='{work}']"))
        .await?;
    b.wait("return !!document.querySelector('#documents-file');", WAIT)
        .await?;
    b.fill(
        "#documents-description",
        "Vistoria fictícia para teste integrado",
    )
    .await?;
    let file = h.evidence.join("registry-upload-fixture.txt");
    fs::write(&file, CONTENT)?;
    let id = b.element("#documents-file").await?;
    b.post(
        &format!("element/{id}/value"),
        json!({"text":file.to_string_lossy()}),
    )
    .await?;
    b.wait("return document.querySelector('#documents-feedback')?.textContent.includes('Arquivo preparado');", WAIT).await?;
    b.tab_to("form:has(#documents-file) button[type='submit']")
        .await?;
    b.key(ENTER).await?;
    b.wait(
        "return document.querySelector('.document-version')?.textContent.includes('Versão ativa');",
        WAIT,
    )
    .await?;
    let context = h.auth.documents_context(&owner.session).await?;
    let doc_scope = DocumentScope {
        organization_id: context.organization_id.ok_or("selected organization")?,
        context_version: context.context_version,
        scope_id: work.into(),
    };
    let list = h
        .auth
        .documents_list(&owner.session, &doc_scope, None)
        .await?;
    assert_eq!(list.versions.len(), 1);
    let document = &list.versions[0];
    let link_body =
        json!({"scope":doc_scope,"document_id":document.document_id,"version":1,"preview":false});
    let (status, link) = post(h, owner, "/api/documents/link", link_body.clone()).await?;
    assert_eq!(status, 200, "{link}");
    let url = format!(
        "{}{}",
        h.app_url,
        link["url"].as_str().ok_or("download URL")?
    );
    let original = h
        .http
        .get(&url)
        .header(
            "Cookie",
            format!("__Host-canteiro-session={}", owner.session),
        )
        .send()
        .await?;
    assert!(original.status().is_success());
    assert_eq!(original.headers()["x-content-type-options"], "nosniff");
    assert_eq!(original.bytes().await?.as_ref(), CONTENT.as_bytes());
    assert!(
        !h.http
            .get(&url)
            .header(
                "Cookie",
                format!("__Host-canteiro-session={}", reader.session)
            )
            .send()
            .await?
            .status()
            .is_success()
    );
    let (status, _) = post(h, reader, "/api/documents/link", link_body).await?;
    assert_eq!(status, 403);

    let upload = json!({"scope":doc_scope,"operation_key":Uuid::new_v4().to_string(),"document_id":null,"expected_version":0,
        "file_name":"rejeitado.txt","description":"Conteúdo da fixture recusado pelo protocolo","category":"evidence",
        "content_base64":base64::engine::general_purpose::STANDARD.encode("FIXTURE_REJECT")});
    let (status, rejected) = post(h, owner, "/api/documents/upload", upload.clone()).await?;
    assert_eq!(status, 200, "{rejected}");
    assert_eq!(rejected["status"], "rejected");
    assert!(rejected["document_id"].is_null());
    let (status, repeated) = post(h, owner, "/api/documents/upload", upload).await?;
    assert_eq!(status, 200);
    assert_eq!(rejected["upload_id"], repeated["upload_id"]);
    assert_eq!(
        h.auth
            .documents_list(&owner.session, &doc_scope, None)
            .await?
            .versions
            .len(),
        1
    );
    layouts.extend(capture(h, b, "documentos", "#documents-title").await?);

    b.click(".workspace-navigation a[href='/obrigacoes']")
        .await?;
    b.wait(
        "return document.body.textContent.includes('ART') && !!document.querySelector('h1');",
        WAIT,
    )
    .await?;
    b.click(&format!("select[name='scope_id'] option[value='{work}']"))
        .await?;
    b.fill("input[name='discipline']", "Estruturas").await?;
    b.fill(
        "textarea[name='description']",
        "Registro pendente da responsabilidade pela execução estrutural",
    )
    .await?;
    b.fill("input[name='number']", "PENDENTE").await?;
    b.tab_to("button[name='confirm'][value='false']").await?;
    b.key(ENTER).await?;
    b.wait("return document.body.textContent.includes('Pendente de conferência') && !!document.querySelector(\"input[name='obligation_id'][value]:not([value=''])\");", WAIT).await?;
    let pending = h.auth.obligations_context(&owner.session).await?;
    assert_eq!(pending.obligations.len(), 1);
    assert!(!pending.obligations[0].confirmed);
    b.fill(
        "input[name='reason']",
        "Tentativa de conferência com dados ainda incompletos",
    )
    .await?;
    b.click("button[name='confirm'][value='true']").await?;
    b.wait("return document.body.textContent.includes('Indique o profissional') && !!document.querySelector(\"input[name='number']\");", WAIT).await?;
    assert_eq!(
        b.property("input[name='number']", "value").await?,
        "PENDENTE"
    );
    assert!(
        !h.auth
            .obligations_context(&owner.session)
            .await?
            .obligations[0]
            .confirmed
    );
    layouts.extend(capture(h, b, "obrigacoes", "h1").await?);
    write_json(
        &h.evidence.join("registry-interface.json"),
        &json!({
            "status":"passed","browser":b.versions,"environment":h.environment,"layouts":layouts,
            "scanner":"controlled INSTREAM protocol fixture, not a real antimalware database",
            "scenarios":["official alphanumeric CNPJ submitted through UI", "invalid identifier preserves all other input", "work and exact locality allocation saved by keyboard", "reader excluded from portfolio", "project created without work or contract via SSR form", "manager cannot approve own project", "file selected and uploaded through browser", "clean protocol response commits original bytes", "download revalidates session and scope", "prohibited scanner verdict never creates active document", "repeated upload has one result", "ART pending record submitted by keyboard", "incomplete ART cannot be confirmed and form input is preserved", "native SSR routes reached from hydrated navigation", "five screens at 360 and 1280 CSS pixels"]
        }),
    )?;
    Ok(())
}
