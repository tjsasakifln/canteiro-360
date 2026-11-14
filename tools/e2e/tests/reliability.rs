//! Browser acceptance for conflict, recovery and authorization in contract history.
#[allow(dead_code)]
mod support;

use canteiro_contracts::{
    access::{AccessAction, AccessCommand, AccessRole, ScopeKind},
    reliability::ReliabilityReply,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use support::{
    Browser, ENTER, HOME, Harness, TestResult,
    access_fixture::{AccountFixture, account, company, fixture_grant, join, scope},
    write_json,
};

const WAIT: Duration = Duration::from_secs(30);
const ORIGINAL: &str = "Contrato da Praça";
const FIRST: &str = "Praça municipal revisada";
const SECOND: &str = "Praça e entorno";
const THIRD: &str = "Praça e paisagismo";

#[tokio::test]
#[ignore = "requer binário, WASM, PostgreSQL e Chrome/ChromeDriver fixados"]
async fn recoverable_edits_and_private_contract_history() -> TestResult {
    let h = Harness::start("contract-history-interface").await?;
    let owner = account(&h, "h-owner", true).await?;
    let peer = account(&h, "h-peer", false).await?;
    let reader = account(&h, "h-reader", false).await?;
    let org = company(&h, &owner, "Horizonte Construções").await?;
    for person in [&peer, &reader] {
        join(&h, &owner, person, &org).await?;
    }
    let work = create_scope(&h, &owner, ScopeKind::Work, "Praça", None).await?;
    let contract = create_scope(&h, &owner, ScopeKind::Contract, ORIGINAL, Some(&work)).await?;
    let private = "Acesso exclusivo da diretoria";
    let hidden = create_scope(&h, &owner, ScopeKind::Contract, private, Some(&work)).await?;
    for editor in [&owner, &peer] {
        fixture_grant(
            &h,
            &owner,
            editor,
            &contract,
            AccessRole::Manager,
            vec![AccessAction::View, AccessAction::EditContractRecord],
        )
        .await?;
    }
    fixture_grant(
        &h,
        &owner,
        &reader,
        &contract,
        AccessRole::Viewer,
        vec![AccessAction::View],
    )
    .await?;
    fixture_grant(
        &h,
        &owner,
        &owner,
        &hidden,
        AccessRole::Viewer,
        vec![AccessAction::View],
    )
    .await?;

    let public = h
        .http
        .get(format!("{}/historico", h.app_url))
        .send()
        .await?;
    assert!(public.status().is_success());
    let html = public.text().await?;
    assert!(html.contains("Histórico de contratos") && !html.contains(ORIGINAL));

    let a = Browser::start(&h).await?;
    let b = Browser::start(&h).await?;
    let result = journey(&h, &a, &b, &owner, &peer, &reader, &contract, private).await;
    if let Err(error) = &result {
        for (browser, suffix) in [(&a, "owner"), (&b, "peer")] {
            let _ = browser
                .full_screenshot(
                    &h.evidence
                        .join(format!("contract-history-failure-{suffix}.png")),
                )
                .await;
            let response = browser
                .execute("return document.querySelector('.history-page')?.innerText;")
                .await
                .ok();
            let _ = write_json(
                &h.evidence
                    .join(format!("contract-history-failure-{suffix}.json")),
                &json!({"error":error.to_string(),"page":response}),
            );
        }
    }
    a.close().await?;
    b.close().await?;
    result
}

async fn create_scope(
    h: &Harness,
    person: &AccountFixture,
    kind: ScopeKind,
    name: &str,
    parent: Option<&str>,
) -> TestResult<String> {
    let context = h.auth.access_context(&person.session).await?;
    let changed = h
        .auth
        .access_execute(
            &person.session,
            AccessCommand::CreateScope {
                context: scope(&context.state),
                kind,
                name: name.into(),
                parent_id: parent.map(str::to_owned),
            },
        )
        .await?;
    changed
        .state
        .scopes
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .ok_or_else(|| "Referência de teste ausente".into())
}

async fn authenticate(h: &Harness, browser: &Browser, person: &AccountFixture) -> TestResult {
    browser.navigate(&h.app_url).await?;
    browser_cookie(browser, person).await?;
    browser.tab_to("a[href='/historico']").await?;
    browser.key(ENTER).await?;
    ready(browser).await
}

async fn browser_cookie(browser: &Browser, person: &AccountFixture) -> TestResult {
    browser
        .post(
            "cookie",
            json!({"cookie": {
                "name":"__Host-canteiro-session", "value":person.session, "path":"/",
                "secure":true, "httpOnly":true, "sameSite":"Strict"
            }}),
        )
        .await?;
    Ok(())
}

async fn ready(browser: &Browser) -> TestResult {
    browser.wait("return document.querySelector('[data-history-ready]')?.dataset.historyReady === 'true';", WAIT).await
}

async fn action(browser: &Browser, selector: &str) -> TestResult {
    browser.tab_to(selector).await?;
    browser.key(ENTER).await?;
    ready(browser).await
}

async fn open(browser: &Browser, contract: &str) -> TestResult {
    browser
        .click(&format!("#history-contract option[value='{contract}']"))
        .await?;
    action(browser, "#history-open").await?;
    assert_eq!(
        browser
            .execute("return document.activeElement?.id;")
            .await?,
        "history-contract-heading"
    );
    Ok(())
}

async fn draft(browser: &Browser, title: &str, reason: &str) -> TestResult {
    browser.fill("#history-title", title).await?;
    browser.fill("#history-reason", reason).await?;
    Ok(())
}

async fn network_block(h: &Harness, browser: &Browser, block: bool) -> TestResult {
    browser.cdp("Network.enable", json!({})).await?;
    let urls = if block {
        vec![format!("{}/api/reliability/command", h.app_url)]
    } else {
        Vec::new()
    };
    browser
        .cdp("Network.setBlockedURLs", json!({"urls":urls}))
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn journey(
    h: &Harness,
    a: &Browser,
    b: &Browser,
    owner: &AccountFixture,
    peer: &AccountFixture,
    reader: &AccountFixture,
    contract: &str,
    private: &str,
) -> TestResult {
    authenticate(h, a, owner).await?;
    authenticate(h, b, peer).await?;
    assert!(!b.text("#history-contract").await?.contains(private));
    open(a, contract).await?;
    open(b, contract).await?;
    assert_eq!(a.text("#history-current-version").await?, "Versão 1");
    assert_eq!(b.text("#history-current-version").await?, "Versão 1");

    // Deixe a segunda aba na versão 1 mesmo depois de efetivar a primeira revisão.
    draft(b, SECOND, "Revisão do segundo gestor").await?;
    action(a, "#history-acquire").await?;
    assert!(
        a.text("#history-lease-status")
            .await?
            .contains("nesta aba até")
    );
    action(a, "#history-renew").await?;
    assert!(a.text("#history-feedback").await?.contains("renovado"));
    action(b, "#history-acquire").await?;
    assert!(
        b.text("#history-feedback")
            .await?
            .contains("Outra sessão reservou")
    );
    assert_eq!(b.property("#history-title", "value").await?, SECOND);
    draft(a, FIRST, "Revisão do primeiro gestor").await?;
    action(a, "#history-save").await?;
    assert_eq!(a.text("#history-current-version").await?, "Versão 2");
    assert!(
        a.text("#history-feedback")
            .await?
            .contains("Gravação confirmada")
    );

    action(b, "#history-acquire").await?;
    action(b, "#history-save").await?;
    b.wait(
        "return document.activeElement?.id === 'history-conflict-heading';",
        WAIT,
    )
    .await?;
    assert_eq!(b.text("#history-conflict-base").await?, ORIGINAL);
    assert_eq!(b.text("#history-conflict-saved").await?, FIRST);
    assert_eq!(b.text("#history-conflict-draft").await?, SECOND);
    assert_eq!(
        b.property("#history-reason", "value").await?,
        "Revisão do segundo gestor"
    );
    assert!(!b.enabled("#history-save").await?);
    let mut layouts = BTreeMap::new();
    for width in [1280, 360] {
        layouts.insert(width.to_string(), capture(h, b, width, "conflict").await?);
    }
    assert_eq!(
        b.execute(
            "return getComputedStyle(document.getElementById('history-draft-status')).overflowWrap;"
        )
        .await?,
        "anywhere"
    );

    // Rebase consciente, sem apagar o texto. Queda antes de encaminhar a
    // gravação deixa a tentativa pendente para consulta antes do reenvio.
    action(b, "#history-rebase").await?;
    assert_eq!(b.property("#history-title", "value").await?, SECOND);
    assert!(b.text("#history-draft-status").await?.contains(&peer.email));
    assert!(b.text("#history-draft-status").await?.contains("versão 2"));
    network_block(h, b, true).await?;
    action(b, "#history-save").await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 2");
    assert_eq!(b.property("#history-title", "value").await?, SECOND);
    assert!(!b.enabled("#history-save").await?);
    assert!(
        !b.text("#history-feedback")
            .await?
            .contains("Gravação confirmada")
    );
    network_block(h, b, false).await?;
    action(b, "#history-check").await?;
    assert!(
        b.text("#history-feedback")
            .await?
            .contains("Nenhuma gravação encontrada")
    );
    assert_eq!(b.text("#history-save").await?, "Repetir a mesma gravação");
    action(b, "#history-save").await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 3");

    // O proxy entrega o POST ao Axum, aguarda o commit e encerra a
    // resposta ao navegador. Consulte o recibo pela chave da mesma tentativa.
    let proxy = LossOfReply::start(h).await?;
    b.navigate(&proxy.origin).await?;
    browser_cookie(b, peer).await?;
    b.navigate(&format!("{}/historico", proxy.origin)).await?;
    ready(b).await?;
    open(b, contract).await?;
    action(b, "#history-acquire").await?;
    draft(b, THIRD, "Revisão para testar confirmação perdida").await?;
    action(b, "#history-save").await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 3");
    assert_eq!(b.property("#history-title", "value").await?, THIRD);
    assert!(
        !b.text("#history-feedback")
            .await?
            .contains("Gravação confirmada")
    );
    action(b, "#history-check").await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 4");
    assert!(
        b.text("#history-feedback")
            .await?
            .contains("Gravação confirmada")
    );
    proxy.check()?;

    // Uma ocorrência retroativa inclui hora brasileira e segundos pela interface.
    authenticate(h, b, peer).await?;
    open(b, contract).await?;
    action(b, "#history-acquire").await?;
    draft(
        b,
        "Praça e paisagismo final",
        "Ocorrência anterior registrada depois",
    )
    .await?;
    b.fill("#history-occurred", "01/01/2020 08:23:47").await?;
    action(b, "#history-save").await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 5");
    let revisions = b.text(".history-entries").await?;
    assert!(
        revisions.contains("Registro retroativo") && revisions.contains("01/01/2020 às 08:23:47")
    );
    let order =
        [ORIGINAL, FIRST, SECOND].map(|title| revisions.find(title).expect("título no histórico"));
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]));
    for width in [1280, 360] {
        capture(h, b, width, "recorded").await?;
    }
    let link = b.attribute("#history-report", "href").await?;
    let link =
        reqwest::Url::parse(&h.app_url)?.join(link.as_str().ok_or("Endereço do CSV ausente")?)?;
    let report = h
        .http
        .get(link)
        .header(
            "Cookie",
            format!("__Host-canteiro-session={}", peer.session),
        )
        .send()
        .await?
        .error_for_status()?;
    assert_eq!(report.headers()["cache-control"], "no-store");
    assert!(
        report.headers()["content-disposition"]
            .to_str()?
            .starts_with("attachment")
    );
    let csv = report.text().await?;
    for expected in [
        "Ocorrido em (UTC)",
        "Registrado em (UTC)",
        "2020-01-01T11:23:47",
        FIRST,
        SECOND,
        THIRD,
        "Praça e paisagismo final",
    ] {
        assert!(csv.contains(expected), "CSV omitiu {expected}");
    }
    assert_eq!(csv.lines().count(), 6);

    // Uma troca de identidade não retoma o rascunho do editor nem mostra
    // contrato vizinho. O leitor conserva consulta e exportação.
    draft(b, "Rascunho privado", "Não exibir a outra conta").await?;
    b.tab_to("a[href='/organizacoes']").await?;
    b.key(ENTER).await?;
    b.wait("return location.pathname === '/organizacoes';", WAIT)
        .await?;
    b.history_back().await?;
    ready(b).await?;
    assert_eq!(
        b.property("#history-title", "value").await?,
        "Rascunho privado"
    );
    browser_cookie(b, reader).await?;
    action(b, "#history-refresh").await?;
    open(b, contract).await?;
    assert_eq!(b.text("#history-current-version").await?, "Versão 5");
    assert_eq!(
        b.execute("return !document.querySelector('#history-editor');")
            .await?,
        true
    );
    assert!(!b.text(".history-page").await?.contains("Rascunho privado"));
    assert!(!b.text("#history-contract").await?.contains(private));

    write_json(
        &h.evidence.join("contract-history-interface.json"),
        &json!({
            "status":"passed","versions":b.versions,"environment":h.environment, "layouts":layouts,
            "scenarios":["keyboard navigation and conflict focus","lease exclusion and renewal","two authenticated viewers on version 1","server conflict preserves draft and shows three versions","network failure before commit and exact retry","response lost after server commit and authorized receipt recovery","two distinct instants and chronological CSV","route history preserves scoped draft","reader has no edit and identity switch cannot recover another draft"]
        }),
    )?;
    Ok(())
}

async fn capture(h: &Harness, b: &Browser, width: u32, variant: &str) -> TestResult<Value> {
    b.viewport(width, 900, width == 360).await?;
    b.chord(&["\u{e009}"], HOME).await?;
    b.wait("return scrollY === 0;", WAIT).await?;
    let layout = b.execute("return {width:innerWidth,contentWidth:document.documentElement.clientWidth,scroll:document.documentElement.scrollWidth,unlabeled:[...document.querySelectorAll('.history-page input,.history-page select')].filter(e=>e.getClientRects().length&&!e.labels.length).map(e=>e.id),overflow:[...document.querySelectorAll('body *')].filter(e=>e.getClientRects().length&&(e.getBoundingClientRect().right>document.documentElement.clientWidth+1||e.scrollWidth>e.clientWidth+2)).slice(0,20).map(e=>({tag:e.tagName,id:e.id,className:e.className,right:e.getBoundingClientRect().right,scroll:e.scrollWidth,client:e.clientWidth}))};").await?;
    assert_eq!(layout["width"], width, "{layout}");
    assert!(
        layout["scroll"].as_u64() <= layout["contentWidth"].as_u64(),
        "A tela transborda horizontalmente: {layout}"
    );
    assert_eq!(layout["unlabeled"], json!([]));
    let name = format!("contract-history-{variant}-{width}.png");
    b.full_screenshot(&h.evidence.join(name)).await?;
    Ok(layout)
}

/// Proxy HTTP local do teste. As conexões são fechadas após cada resposta para
/// tornar observável a perda da confirmação que segue um commit bem-sucedido.
struct LossOfReply {
    origin: String,
    dropped: Arc<Mutex<Vec<String>>>,
    looked_up: Arc<Mutex<Vec<String>>>,
    listener: tokio::task::JoinHandle<()>,
}

impl LossOfReply {
    async fn start(h: &Harness) -> TestResult<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let origin = format!("http://{}", listener.local_addr()?);
        let state = ProxyState {
            upstream: h.app_url.clone(),
            client: h.http.clone(),
            dropped: Arc::default(),
            looked_up: Arc::default(),
        };
        let dropped = state.dropped.clone();
        let looked_up = state.looked_up.clone();
        let listener = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(error) = state.forward(socket).await {
                        eprintln!("Falha no proxy local da evidência: {error}");
                    }
                });
            }
        });
        Ok(Self {
            origin,
            dropped,
            looked_up,
            listener,
        })
    }

    fn check(&self) -> TestResult {
        let dropped = self
            .dropped
            .lock()
            .map_err(|_| "Registro de respostas indisponível")?;
        let looked_up = self
            .looked_up
            .lock()
            .map_err(|_| "Registro de consultas indisponível")?;
        assert_eq!(
            dropped.len(),
            1,
            "Uma única gravação HTTP chegou com sucesso"
        );
        assert_eq!(
            dropped.as_slice(),
            looked_up.as_slice(),
            "A consulta usou a chave da confirmação perdida"
        );
        Ok(())
    }
}

impl Drop for LossOfReply {
    fn drop(&mut self) {
        self.listener.abort();
    }
}

#[derive(Clone)]
struct ProxyState {
    client: reqwest::Client,
    upstream: String,
    dropped: Arc<Mutex<Vec<String>>>,
    looked_up: Arc<Mutex<Vec<String>>>,
}

impl ProxyState {
    async fn forward(self, mut socket: tokio::net::TcpStream) -> TestResult {
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
        let mut connection = BufReader::new(&mut socket);
        let mut first = String::new();
        connection.read_line(&mut first).await?;
        let mut words = first.split_whitespace();
        let Some(method) = words.next() else {
            return Ok(());
        };
        let path = words.next().ok_or("O proxy recebeu uma URL inválida")?;
        let mut request = self.client.request(
            reqwest::Method::from_bytes(method.as_bytes())?,
            format!("{}{path}", self.upstream),
        );
        let mut length = 0;
        loop {
            let mut line = String::new();
            if connection.read_line(&mut line).await? == 0 || line == "\r\n" {
                break;
            }
            let (name, value) = line
                .split_once(':')
                .ok_or("Cabeçalho HTTP inválido no proxy")?;
            let value = value.trim();
            match name.to_ascii_lowercase().as_str() {
                "content-length" => length = value.parse::<usize>()?,
                "origin" => request = request.header(name, &self.upstream),
                "host" | "connection" | "accept-encoding" => {}
                _ => request = request.header(name, value),
            }
        }
        let mut body = vec![0; length];
        connection.read_exact(&mut body).await?;
        let attempted_save = if method == "POST" && path == "/api/reliability/command" {
            let json: Value = serde_json::from_slice(&body)?;
            (json["command"] == "save").then(|| json["key"].as_str().unwrap_or_default().to_owned())
        } else {
            None
        };
        if let Some((_, operation)) = path.split_once("/operations/") {
            self.looked_up
                .lock()
                .map_err(|_| "Registro de consultas indisponível")?
                .push(operation.split('?').next().unwrap_or_default().to_owned());
        }
        let mut response = request.body(body).send().await?;
        if let Some(key) = attempted_save
            && response.status().is_success()
        {
            let reply = response.json::<ReliabilityReply>().await?;
            if matches!(reply, ReliabilityReply::Saved { .. }) {
                self.dropped
                    .lock()
                    .map_err(|_| "Registro de respostas indisponível")?
                    .push(key);
                return Ok(());
            }
            return Err("O proxy esperava um recibo de gravação".into());
        }
        let status = response.status();
        let mut headers = format!(
            "HTTP/1.1 {} {}\r\nConnection: close\r\n",
            status.as_u16(),
            status.canonical_reason().unwrap_or("HTTP")
        );
        for (name, value) in response.headers() {
            use std::fmt::Write;
            if !matches!(
                name.as_str(),
                "connection" | "content-length" | "transfer-encoding"
            ) {
                writeln!(headers, "{name}: {}\r", value.to_str()?)?;
            }
        }
        let mut result = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            result.extend_from_slice(&chunk);
        }
        use std::fmt::Write;
        write!(headers, "Content-Length: {}\r\n\r\n", result.len())?;
        socket.write_all(headers.as_bytes()).await?;
        socket.write_all(&result).await?;
        Ok(())
    }
}
