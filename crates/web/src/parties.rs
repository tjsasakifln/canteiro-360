use canteiro_contracts::{auth::AuthFailure, organizations::OrganizationScope, parties::*};
use leptos::{prelude::*, task::spawn_local};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy)]
pub(crate) struct RegistryLifetime(StoredValue<Arc<AtomicBool>>);

impl RegistryLifetime {
    pub(crate) fn new() -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        let cleanup = alive.clone();
        on_cleanup(move || cleanup.store(false, Ordering::Release));
        Self(StoredValue::new(alive))
    }
    pub(crate) fn capture(self) -> Arc<AtomicBool> {
        self.0.get_value()
    }
}

pub(crate) fn url(value: &str) -> String {
    value
        .bytes()
        .map(|c| {
            if c.is_ascii_alphanumeric() || b"-_.~".contains(&c) {
                (c as char).to_string()
            } else {
                format!("%{c:02X}")
            }
        })
        .collect()
}
pub(crate) fn scope_query(scope: &OrganizationScope) -> String {
    format!(
        "organization_id={}&context_version={}&organization_version={}",
        url(&scope.organization_id),
        scope.context_version,
        scope.organization_version
    )
}
fn unavailable() -> AuthFailure {
    AuthFailure{code:"network_unavailable".into(),message:"Não foi possível confirmar a resposta. Os campos continuam nesta aba. Confira a conexão e tente novamente.".into()}
}
pub(crate) async fn request<T: DeserializeOwned>(
    path: &str,
    body: Option<String>,
    csrf: Option<&str>,
) -> Result<T, String> {
    crate::http::request(
        path,
        body,
        crate::http::RequestOptions {
            csrf,
            ..Default::default()
        },
        unavailable,
    )
    .await
    .map_err(|e| e.details.message)
}
pub(crate) fn attempt_key<T: Serialize>(
    attempt: RwSignal<Option<(String, String)>>,
    input: &T,
) -> Result<String, String> {
    let value =
        serde_json::to_string(input).map_err(|_| "Confira os dados antes de salvar.".to_owned())?;
    if let Some((old, key)) = attempt.get_untracked()
        && old == value
    {
        return Ok(key);
    }
    let key = crate::http::operation_id().map_err(|e| e.message)?;
    attempt.set(Some((value, key.clone())));
    Ok(key)
}

#[component]
pub(crate) fn RegistryField(
    id: &'static str,
    label: &'static str,
    #[prop(into)] value: Signal<String>,
    on_input: Callback<String>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(optional)] required: bool,
) -> impl IntoView {
    view! {<div class="registry-field"><label for=id>{label}</label><input id=id type=kind prop:value=move||value.get() required=required maxlength="500" on:input=move|e|on_input.run(event_target_value(&e))/></div>}
}
fn fresh() -> PartyInput {
    PartyInput {
        kind: PartyKind::Company,
        name: String::new(),
        tax_id: None,
        roles: vec![PartyRole::Customer],
        contacts: vec![],
        address: String::new(),
        professional: None,
        active: true,
    }
}
fn fresh_bank() -> BankAccount {
    BankAccount {
        bank: String::new(),
        branch: String::new(),
        account: String::new(),
        holder: String::new(),
        holder_tax_id: String::new(),
    }
}
async fn refresh(
    context: RwSignal<Option<PartiesContext>>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    query: String,
    page: u32,
    alive: Arc<AtomicBool>,
) {
    if !alive.load(Ordering::Acquire) {
        return;
    }
    busy.set(true);
    let result = request(
        &format!("/api/parties/context?query={}&page={page}", url(&query)),
        None,
        None,
    )
    .await;
    if !alive.load(Ordering::Acquire) {
        return;
    }
    match result {
        Ok(value) => context.set(Some(value)),
        Err(e) => {
            context.set(None);
            message.set(e);
        }
    }
    busy.set(false);
}
fn import_rows(value: &str) -> Result<Vec<PartyInput>, String> {
    let mut rows = vec![];
    for (index, line) in value.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<_> = line.split(';').map(str::trim).collect();
        if cells.len() != 4 {
            return Err(format!(
                "Linha {}: use tipo;nome;CPF ou CNPJ;funções.",
                index + 1
            ));
        }
        let kind = match cells[0].to_lowercase().as_str() {
            "pessoa" => PartyKind::Person,
            "empresa" => PartyKind::Company,
            _ => {
                return Err(format!(
                    "Linha {}: o tipo deve ser pessoa ou empresa.",
                    index + 1
                ));
            }
        };
        let mut roles = vec![];
        for role in cells[3].split(',') {
            roles.push(match role.trim().to_lowercase().as_str() {
                "cliente" => PartyRole::Customer,
                "fornecedor" => PartyRole::Supplier,
                "profissional" => PartyRole::Professional,
                _ => {
                    return Err(format!(
                        "Linha {}: use cliente, fornecedor ou profissional.",
                        index + 1
                    ));
                }
            });
        }
        let profile = roles
            .contains(&PartyRole::Professional)
            .then_some(Professional {
                council: String::new(),
                registration: String::new(),
                uf: String::new(),
                valid_from: None,
                valid_until: None,
            });
        rows.push(
            PartyInput {
                kind,
                name: cells[1].into(),
                tax_id: (!cells[2].is_empty()).then(|| cells[2].into()),
                roles,
                contacts: vec![],
                address: String::new(),
                professional: profile,
                active: true,
            }
            .validated()
            .map_err(|e| format!("Linha {}: {}", index + 1, e))?,
        );
    }
    if rows.is_empty() || rows.len() > 100 {
        return Err("Cole entre uma e cem linhas.".into());
    }
    Ok(rows)
}

#[component]
pub fn PartiesPage() -> impl IntoView {
    let lifetime = RegistryLifetime::new();
    let context = RwSignal::new(None::<PartiesContext>);
    let draft = RwSignal::new(fresh());
    let selected = RwSignal::new(None::<(String, i64)>);
    let message = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let page = RwSignal::new(0_u32);
    let reason = RwSignal::new("Cadastro inicial".to_owned());
    let attempt = RwSignal::new(None::<(String, String)>);
    let editor_tenant = RwSignal::new(None::<String>);
    let import_text = RwSignal::new(String::new());
    let bank = RwSignal::new(fresh_bank());
    let bank_history = RwSignal::new(None::<BankHistory>);
    let history = RwSignal::new(Vec::<PartyRevision>::new());
    Effect::new(move |_| {
        spawn_local(refresh(
            context,
            message,
            busy,
            String::new(),
            0,
            lifetime.capture(),
        ))
    });
    Effect::new(move |_| {
        if let Some(tenant) = context
            .get()
            .and_then(|c| c.scope.map(|s| s.organization_id))
        {
            if editor_tenant.get_untracked().is_none() {
                editor_tenant.set(Some(tenant));
            } else if editor_tenant.get_untracked().as_ref() != Some(&tenant) {
                message.set("A empresa mudou. Seu rascunho foi preservado. Selecione um cadastro ou clique em Novo cadastro para trabalhar na empresa atual.".into());
            }
        }
    });
    let save = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let Some(ctx) = context.get_untracked() else {
            return;
        };
        let Some(scope) = ctx.scope else {
            return;
        };
        if editor_tenant.get_untracked().as_deref() != Some(scope.organization_id.as_str()) {
            message.set("A empresa mudou. Selecione um cadastro ou clique em Novo cadastro antes de continuar.".into());
            return;
        }
        let input = match draft.get_untracked().validated() {
            Ok(v) => v,
            Err(e) => {
                message.set(e.to_string());
                return;
            }
        };
        let chosen = selected.get_untracked();
        let mut command = PartyCommand::Save {
            context: scope.clone(),
            key: String::new(),
            id: chosen.as_ref().map(|c| c.0.clone()),
            expected_version: chosen.map(|c| c.1),
            input,
            reason: reason.get_untracked(),
        };
        let key = match attempt_key(attempt, &command.idempotency_payload()) {
            Ok(v) => v,
            Err(e) => {
                message.set(e);
                return;
            }
        };
        if let PartyCommand::Save { key: target, .. } = &mut command {
            *target = key;
        }
        busy.set(true);
        let alive = lifetime.capture();
        spawn_local(async move {
            let body = serde_json::to_string(&command).ok();
            let result =
                request::<PartyReceipt>("/api/parties/command", body, Some(&ctx.csrf)).await;
            if !alive.load(Ordering::Acquire) {
                return;
            }
            match result {
                Ok(reply) => {
                    message.set(reply.message);
                    attempt.set(None);
                    if let Some(id) = reply.ids.first() {
                        let row = request::<PartyView>(
                            &format!("/api/parties/{id}?{}", scope_query(&scope)),
                            None,
                            None,
                        )
                        .await;
                        if !alive.load(Ordering::Acquire) {
                            return;
                        }
                        if let Ok(row) = row {
                            selected.set(Some((row.id, row.version)));
                            draft.set(row.input);
                            reason.set("Atualização cadastral".into());
                        }
                    }
                    refresh(
                        context,
                        message,
                        busy,
                        query.get_untracked(),
                        page.get_untracked(),
                        alive.clone(),
                    )
                    .await;
                }
                Err(e) => message.set(e),
            }
            if alive.load(Ordering::Acquire) {
                busy.set(false);
            }
        });
    };
    view! {
        <section class="registry-page" aria-labelledby="parties-title">
            <p class="eyebrow">"Cadastros"</p><h1 id="parties-title">"Clientes, fornecedores e profissionais"</h1>
            <p>"Reutilize os contatos da empresa e preserve o histórico. Cadastrar um profissional não cria acesso ao sistema."</p>
            <p class="registry-context">{move||context.get().map(|c|if c.scope.is_some(){format!("Empresa: {}",c.organization_name)}else{"Selecione uma empresa para consultar os cadastros.".into()}).unwrap_or_else(||"Carregando a empresa…".into())}</p>
            <p><a href="/organizacoes">"Escolher empresa"</a></p>
            <p id="registry-message" class="registry-message" role="status" aria-live="polite" tabindex="-1">{move||message.get()}</p>
            <p role="status" hidden=move||!busy.get()>"Conferindo e carregando os dados…"</p>
            <form class="registry-search" on:submit=move|ev|{ev.prevent_default();if busy.get_untracked(){return;}page.set(0);spawn_local(refresh(context,message,busy,query.get_untracked(),0,lifetime.capture()));}>
                <RegistryField id="party-search" label="Buscar nome, CPF ou CNPJ" value=Signal::derive(move||query.get()) on_input=Callback::new(move|v|query.set(v))/>
                <button type="submit" class="primary-button" disabled=move||busy.get()>"Buscar"</button>
            </form>
            {move||context.get().and_then(|c|c.scope.map(|s|view!{<p><a href=format!("/api/parties/export.json?{}&query={}",scope_query(&s),url(&query.get()))>"Exportar cadastros com documentos em texto"</a></p>}))}
            <div class="registry-layout">
                <section class="registry-card" aria-label="Cadastros encontrados">
                    <h2>"Cadastros encontrados"</h2><p>{move||context.get().map(|c|format!("{} cadastros. Página {}.",c.total,c.page+1)).unwrap_or_default()}</p>
                    <ul class="registry-list">{move||context.get().map(|c|c.rows.into_iter().map(|row|{let title=row.input.name.clone();let kind=row.input.kind.label();let active=if row.input.active{"Ativo"}else{"Inativo"};view!{<li><button type="button" class="registry-list-button" disabled=move||busy.get() on:click=move |_|{editor_tenant.set(context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)));selected.set(Some((row.id.clone(),row.version)));draft.set(row.input.clone());reason.set("Atualização cadastral".into());attempt.set(None);history.set(vec![]);bank_history.set(None);bank.set(fresh_bank());message.set("Cadastro aberto. As alterações só serão gravadas ao salvar.".into());}><strong>{title}</strong><span>{format!("{kind} · {active} · versão {}",row.version)}</span></button></li>}}).collect_view())}</ul>
                    <p hidden=move||context.get().is_none_or(|c|c.total!=0)>"Nenhum cadastro encontrado. Ajuste a busca ou crie o primeiro cadastro."</p>
                    <div class="registry-actions"><button type="button" disabled=move||busy.get()||page.get()==0 on:click=move |_|{let p=page.get_untracked().saturating_sub(1);page.set(p);spawn_local(refresh(context,message,busy,query.get_untracked(),p,lifetime.capture()));}>"Anterior"</button><button type="button" disabled=move||busy.get()||context.get().is_none_or(|c|i64::from(page.get()+1)*50>=c.total) on:click=move |_|{let p=page.get_untracked()+1;page.set(p);spawn_local(refresh(context,message,busy,query.get_untracked(),p,lifetime.capture()));}>"Próxima"</button></div>
                </section>
                <section class="registry-card" aria-labelledby="party-editor-title">
                    <div class="registry-heading"><h2 id="party-editor-title">{move||if selected.get().is_some(){"Revisar cadastro"}else{"Novo cadastro"}}</h2><button type="button" disabled=move||busy.get() on:click=move |_|{editor_tenant.set(context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)));draft.set(fresh());selected.set(None);reason.set("Cadastro inicial".into());attempt.set(None);bank_history.set(None);history.set(vec![]);}>"Novo cadastro"</button></div>
                    <p hidden=move||context.get().is_some_and(|c|c.can_manage)>"A consulta está disponível aos membros. Para alterar, peça a um administrador com verificação em duas etapas."</p>
                    <form on:submit=save><fieldset disabled=move||busy.get()||context.get().is_none_or(|c|!c.can_manage)>
                        <legend>"Identificação"</legend>
                        <label for="party-kind">"Tipo"</label><select id="party-kind" prop:value=move||draft.get().kind.as_str() on:change=move|e|{let person=event_target_value(&e)=="person";draft.update(|v|{v.kind=if person{PartyKind::Person}else{PartyKind::Company};if !person{v.roles.retain(|r|*r!=PartyRole::Professional);v.professional=None;}});}>
                            <option value="company">"Empresa"</option><option value="person">"Pessoa"</option></select>
                        <RegistryField id="party-name" label="Nome" required=true value=Signal::derive(move||draft.get().name) on_input=Callback::new(move|v|draft.update(|d|d.name=v))/>
                        <RegistryField id="party-tax-id" label="CPF ou CNPJ (opcional neste cadastro)" value=Signal::derive(move||draft.get().tax_id.unwrap_or_default()) on_input=Callback::new(move|v|draft.update(|d|d.tax_id=Some(v)))/>
                        <p class="field-help">"CNPJ aceita letras nas primeiras 12 posições. Os dois últimos caracteres são dígitos verificadores."</p>
                        <div class="registry-checks">
                            <label><input type="checkbox" prop:checked=move||draft.get().roles.contains(&PartyRole::Customer) on:change=move|e|{let checked=event_target_checked(&e);draft.update(|d|{d.roles.retain(|r|*r!=PartyRole::Customer);if checked{d.roles.push(PartyRole::Customer);}});}/>"Cliente"</label>
                            <label><input type="checkbox" prop:checked=move||draft.get().roles.contains(&PartyRole::Supplier) on:change=move|e|{let checked=event_target_checked(&e);draft.update(|d|{d.roles.retain(|r|*r!=PartyRole::Supplier);if checked{d.roles.push(PartyRole::Supplier);}});}/>"Fornecedor"</label>
                            <label><input type="checkbox" disabled=move||draft.get().kind!=PartyKind::Person prop:checked=move||draft.get().professional.is_some() on:change=move|e|{let checked=event_target_checked(&e);draft.update(|d|{d.roles.retain(|r|*r!=PartyRole::Professional);d.professional=if checked{d.roles.push(PartyRole::Professional);Some(Professional{council:String::new(),registration:String::new(),uf:String::new(),valid_from:None,valid_until:None})}else{None};});}/>"Profissional"</label>
                        </div>
                        <RegistryField id="party-address" label="Endereço (opcional)" value=Signal::derive(move||draft.get().address) on_input=Callback::new(move|v|draft.update(|d|d.address=v))/>
                        <details open=move||draft.get().professional.is_some()><summary>"Registro e período de atuação do profissional"</summary>
                            <p>"Complete estes dados antes de designar o profissional. A conta de acesso é administrada separadamente."</p>
                            <RegistryField id="party-council" label="Conselho" value=Signal::derive(move||draft.get().professional.map(|p|p.council).unwrap_or_default()) on_input=Callback::new(move|v|draft.update(|d|if let Some(p)=&mut d.professional{p.council=v;}))/>
                            <RegistryField id="party-registration" label="Registro profissional" value=Signal::derive(move||draft.get().professional.map(|p|p.registration).unwrap_or_default()) on_input=Callback::new(move|v|draft.update(|d|if let Some(p)=&mut d.professional{p.registration=v;}))/>
                            <RegistryField id="party-uf" label="UF do registro" value=Signal::derive(move||draft.get().professional.map(|p|p.uf).unwrap_or_default()) on_input=Callback::new(move|v|draft.update(|d|if let Some(p)=&mut d.professional{p.uf=v;}))/>
                            <RegistryField id="party-from" label="Início da atuação" kind="date" value=Signal::derive(move||draft.get().professional.and_then(|p|p.valid_from).map(|d|d.to_string()).unwrap_or_default()) on_input=Callback::new(move|v:String|draft.update(|d|if let Some(p)=&mut d.professional{p.valid_from=v.parse().ok();}))/>
                            <RegistryField id="party-until" label="Fim da atuação (se houver)" kind="date" value=Signal::derive(move||draft.get().professional.and_then(|p|p.valid_until).map(|d|d.to_string()).unwrap_or_default()) on_input=Callback::new(move|v:String|draft.update(|d|if let Some(p)=&mut d.professional{p.valid_until=v.parse().ok();}))/>
                        </details>
                        <details><summary>"Contatos"</summary>
                            <For each={move || (0..draft.get().contacts.len()).collect::<Vec<_>>()} key={|i| *i} children={move |index| view!{<div class="registry-contact"><label for=format!("contact-name-{index}")>"Nome do contato"</label><input id=format!("contact-name-{index}") prop:value=move||draft.get().contacts.get(index).map(|c|c.name.clone()).unwrap_or_default() on:input=move|e|draft.update(|d|if let Some(c)=d.contacts.get_mut(index){c.name=event_target_value(&e);})/><label for=format!("contact-email-{index}")>"E-mail"</label><input id=format!("contact-email-{index}") type="email" prop:value=move||draft.get().contacts.get(index).map(|c|c.email.clone()).unwrap_or_default() on:input=move|e|draft.update(|d|if let Some(c)=d.contacts.get_mut(index){c.email=event_target_value(&e);})/><label for=format!("contact-phone-{index}")>"Telefone"</label><input id=format!("contact-phone-{index}") type="tel" prop:value=move||draft.get().contacts.get(index).map(|c|c.phone.clone()).unwrap_or_default() on:input=move|e|draft.update(|d|if let Some(c)=d.contacts.get_mut(index){c.phone=event_target_value(&e);})/></div>}}/>
                            <button id="party-add-contact" type="button" disabled={move||draft.get().contacts.len()>=10} on:click=move |_|draft.update(|d|d.contacts.push(Contact{name:String::new(),email:String::new(),phone:String::new()}))>"Adicionar contato"</button><button type="button" disabled=move||draft.get().contacts.is_empty() on:click=move |_|draft.update(|d|{d.contacts.pop();})>"Remover último contato"</button>
                        </details>
                        <label><input type="checkbox" prop:checked=move||draft.get().active on:change=move|e|draft.update(|d|d.active=event_target_checked(&e))/>"Cadastro ativo"</label><p class="field-help">"Inativar preserva os vínculos e as versões anteriores."</p>
                        <RegistryField id="party-reason" label="Motivo do cadastro ou da alteração" required=true value=Signal::derive(move||reason.get()) on_input=Callback::new(move|v|reason.set(v))/>
                        <button class="primary-button" type="submit">"Salvar cadastro"</button>
                    </fieldset></form>
                    <button type="button" disabled=move||busy.get()||selected.get().is_none() on:click=move |_|{let(Some(ctx),Some((id,_)))=(context.get_untracked(),selected.get_untracked())else{return;};let Some(scope)=ctx.scope else{return;};if editor_tenant.get_untracked().as_deref()!=Some(scope.organization_id.as_str()){message.set("A empresa mudou. Selecione um cadastro ou clique em Novo cadastro antes de continuar.".into());return;}busy.set(true);let alive=lifetime.capture();spawn_local(async move{
                        let result=request::<Vec<PartyRevision>>(&format!("/api/parties/{id}/history?{}",scope_query(&scope)),None,None).await;
                        if !alive.load(Ordering::Acquire){return;}
                        busy.set(false);
                        if selected.get_untracked().as_ref().is_none_or(|(current,_)|current!=&id)||context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)).as_deref()!=Some(scope.organization_id.as_str()){return;}
                        match result{Ok(rows)=>history.set(rows),Err(e)=>message.set(e)}
                    });}>"Consultar histórico"</button>
                    <ol class="registry-list">{move||history.get().into_iter().map(|h|view!{<li>{format!("Versão {} · {} · {}. {}. Nome salvo: {}.",h.version,h.recorded_at,h.actor,h.reason,h.after.name)}</li>}).collect_view()}</ol>
                </section>
            </div>
            <details class="registry-card"><summary>"Importar uma lista de cadastros"</summary><p>"Cole até 100 linhas, sem cabeçalho. Separe por ponto e vírgula: tipo;nome;CPF ou CNPJ;funções. Funções disponíveis: cliente, fornecedor, profissional. Para mais de uma função, use vírgula."</p><p>"Exemplo: empresa;Empresa Exemplo;12ABC34501DE35;cliente,fornecedor"</p><label for="party-import">"Lista de cadastros"</label><textarea id="party-import" rows="7" prop:value=move||import_text.get() on:input=move|e|import_text.set(event_target_value(&e))></textarea>
                <button class="primary-button" type="button" disabled=move||busy.get()||context.get().is_none_or(|c|!c.can_manage) on:click=move |_|{let Some(ctx)=context.get_untracked()else{return;};let Some(scope)=ctx.scope else{return;};if editor_tenant.get_untracked().as_deref()!=Some(scope.organization_id.as_str()){message.set("A empresa mudou. Selecione um cadastro ou clique em Novo cadastro antes de continuar.".into());return;}let rows=match import_rows(&import_text.get_untracked()){Ok(v)=>v,Err(e)=>{message.set(e);return;}};let mut command=PartyCommand::Import{context:scope,key:String::new(),rows};let key=match attempt_key(attempt,&command.idempotency_payload()){Ok(v)=>v,Err(e)=>{message.set(e);return;}};if let PartyCommand::Import{key:target,..}=&mut command{*target=key;}busy.set(true);let alive=lifetime.capture();spawn_local(async move{let result=request::<PartyReceipt>("/api/parties/command",serde_json::to_string(&command).ok(),Some(&ctx.csrf)).await;if !alive.load(Ordering::Acquire){return;}match result{Ok(reply)=>{message.set(reply.message);attempt.set(None);refresh(context,message,busy,query.get_untracked(),page.get_untracked(),alive.clone()).await;},Err(e)=>message.set(e)}if alive.load(Ordering::Acquire){busy.set(false);}});}>"Conferir e importar"</button>
            </details>
            <details class="registry-card" hidden=move||context.get().is_none_or(|c|!c.can_manage)||selected.get().is_none()||!draft.get().roles.contains(&PartyRole::Supplier)><summary>"Dados bancários restritos do fornecedor"</summary><p>"Somente administradores podem consultar e alterar. Cada gravação mantém a conta anterior no histórico."</p>
                <button type="button" disabled=move||busy.get() on:click=move |_|{let(Some(ctx),Some((id,_)))=(context.get_untracked(),selected.get_untracked())else{return;};let Some(scope)=ctx.scope else{return;};if editor_tenant.get_untracked().as_deref()!=Some(scope.organization_id.as_str()){message.set("A empresa mudou. Selecione um cadastro ou clique em Novo cadastro antes de continuar.".into());return;}busy.set(true);let alive=lifetime.capture();spawn_local(async move{let result=request::<BankHistory>(&format!("/api/parties/{id}/bank?{}",scope_query(&scope)),None,None).await;if !alive.load(Ordering::Acquire){return;}match result{Ok(rows)=>{if selected.get_untracked().as_ref().is_some_and(|(current,_)|current==&id) && context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)).as_deref()==Some(scope.organization_id.as_str()){if let Some(last)=rows.versions.first(){bank.set(last.account.clone());}bank_history.set(Some(rows));}},Err(e)=>message.set(e)}if alive.load(Ordering::Acquire){busy.set(false);}});}>"Consultar conta e histórico"</button>
                <fieldset disabled=move||busy.get()><legend>"Nova versão dos dados bancários"</legend><RegistryField id="bank-name" label="Banco" value=Signal::derive(move||bank.get().bank) on_input=Callback::new(move|v|bank.update(|d|d.bank=v))/>
                <RegistryField id="bank-branch" label="Agência" value=Signal::derive(move||bank.get().branch) on_input=Callback::new(move|v|bank.update(|d|d.branch=v))/>
                <RegistryField id="bank-account" label="Conta com dígito" value=Signal::derive(move||bank.get().account) on_input=Callback::new(move|v|bank.update(|d|d.account=v))/>
                <RegistryField id="bank-holder" label="Titular" value=Signal::derive(move||bank.get().holder) on_input=Callback::new(move|v|bank.update(|d|d.holder=v))/>
                <RegistryField id="bank-holder-tax" label="CPF ou CNPJ do titular" value=Signal::derive(move||bank.get().holder_tax_id) on_input=Callback::new(move|v|bank.update(|d|d.holder_tax_id=v))/>
                <RegistryField id="bank-reason" label="Motivo da alteração bancária" value=Signal::derive(move||reason.get()) on_input=Callback::new(move|v|reason.set(v))/>
                <button class="primary-button" type="button" disabled=move||busy.get() on:click=move |_|{let(Some(ctx),Some((id,version)))=(context.get_untracked(),selected.get_untracked())else{return;};let Some(scope)=ctx.scope else{return;};if editor_tenant.get_untracked().as_deref()!=Some(scope.organization_id.as_str()){message.set("A empresa mudou. Selecione um cadastro ou clique em Novo cadastro antes de continuar.".into());return;}let account=match bank.get_untracked().validated(){Ok(v)=>v,Err(e)=>{message.set(e.to_string());return;}};let mut command=PartyCommand::SetBank{context:scope,key:String::new(),id:id.clone(),expected_version:version,account,reason:reason.get_untracked()};let key=match attempt_key(attempt,&command.idempotency_payload()){Ok(v)=>v,Err(e)=>{message.set(e);return;}};if let PartyCommand::SetBank{key:target,..}=&mut command{*target=key;}busy.set(true);let alive=lifetime.capture();spawn_local(async move{let result=request::<PartyReceipt>("/api/parties/command",serde_json::to_string(&command).ok(),Some(&ctx.csrf)).await;if !alive.load(Ordering::Acquire){return;}match result{Ok(reply)=>{if let Some(v)=reply.versions.first(){selected.set(Some((id,*v)));}message.set(reply.message);attempt.set(None);bank_history.set(None);refresh(context,message,busy,query.get_untracked(),page.get_untracked(),alive.clone()).await;},Err(e)=>message.set(e)}if alive.load(Ordering::Acquire){busy.set(false);}});}>"Salvar nova versão bancária"</button></fieldset>
                <ol class="registry-list">{move||bank_history.get().map(|h|h.versions.into_iter().map(|v|view!{<li>{format!("Versão {} · {} · {}. Banco {}, agência {}, conta {}, titular {}. Motivo: {}.",v.version,v.recorded_at,v.actor,v.account.bank,v.account.branch,v.account.account,v.account.holder,v.reason)}</li>}).collect_view())}</ol>
            </details>
        </section>
    }.into_any()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_validates_every_row_before_submission() {
        assert_eq!(
            import_rows("empresa;Exemplo;12ABC34501DE35;cliente,fornecedor")
                .unwrap()
                .len(),
            1
        );
        assert!(
            import_rows("empresa;Exemplo;12ABC34501DE35;cliente\npessoa;Outra;00000000000;cliente")
                .is_err()
        );
    }
    #[test]
    fn url_encoder_keeps_query_boundaries() {
        assert_eq!(url("A&B +"), "A%26B%20%2B");
    }
}
