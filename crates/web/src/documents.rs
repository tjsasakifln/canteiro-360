//! Interface documental com estados explícitos de recebimento, quarentena e versão.
use crate::http::{RequestOptions, request};
use crate::organizations::network::query_component;
use canteiro_contracts::{auth::AuthFailure, documents::*};
use leptos::prelude::*;
use leptos_meta::Title;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy)]
struct Documents {
    context: RwSignal<Option<DocumentsContext>>,
    selected: RwSignal<String>,
    list: RwSignal<Option<DocumentsList>>,
    busy: RwSignal<bool>,
    feedback: RwSignal<String>,
    error: RwSignal<bool>,
    description: RwSignal<String>,
    category: RwSignal<String>,
    target: RwSignal<String>,
    file_name: RwSignal<String>,
    file_content: RwSignal<Option<String>>,
    operation_key: RwSignal<Option<String>>,
    epoch: RwSignal<u64>,
    link: RwSignal<Option<DocumentLink>>,
    signature_document: RwSignal<Option<DocumentVersion>>,
    signer: RwSignal<String>,
    signed_at: RwSignal<String>,
    evidence: RwSignal<String>,
    alive: StoredValue<Arc<AtomicBool>>,
}

fn unavailable() -> AuthFailure {
    AuthFailure{code:"network_unavailable".into(),message:"A resposta não chegou. Os campos continuam nesta página. Tente novamente com o mesmo envio.".into()}
}

impl Documents {
    fn new() -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        let cleanup = alive.clone();
        on_cleanup(move || cleanup.store(false, Ordering::Release));
        Self {
            alive: StoredValue::new(alive),
            context: RwSignal::new(None),
            selected: RwSignal::new(String::new()),
            list: RwSignal::new(None),
            busy: RwSignal::new(false),
            feedback: RwSignal::new(String::new()),
            error: RwSignal::new(false),
            description: RwSignal::new(String::new()),
            category: RwSignal::new("technical".into()),
            target: RwSignal::new(String::new()),
            file_name: RwSignal::new(String::new()),
            file_content: RwSignal::new(None),
            operation_key: RwSignal::new(None),
            epoch: RwSignal::new(0),
            link: RwSignal::new(None),
            signature_document: RwSignal::new(None),
            signer: RwSignal::new(String::new()),
            signed_at: RwSignal::new(String::new()),
            evidence: RwSignal::new(String::new()),
        }
    }
    fn report(self, message: String, error: bool) {
        self.feedback.set(message);
        self.error.set(error);
        focus_feedback();
    }
    fn scope(self) -> Option<DocumentScope> {
        let context = self.context.get_untracked()?;
        let selected = self.selected.get_untracked();
        if selected.is_empty() {
            return None;
        }
        Some(DocumentScope {
            organization_id: context.organization_id?,
            context_version: context.context_version,
            scope_id: selected,
        })
    }
    fn can_upload(self) -> bool {
        self.context.with(|c| {
            c.as_ref().is_some_and(|c| {
                c.scopes
                    .iter()
                    .any(|s| s.id == self.selected.get() && s.can_upload)
            })
        })
    }
    fn csrf(self) -> String {
        self.context
            .with_untracked(|c| c.as_ref().map(|c| c.csrf.clone()).unwrap_or_default())
    }
    fn changed(self) {
        self.operation_key.set(None);
        self.link.set(None);
    }
    fn load(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.epoch.update(|v| *v += 1);
        let epoch = self.epoch.get_untracked();
        let alive = self.alive.get_value();
        leptos::task::spawn_local(async move {
            let result = request::<DocumentsContext>(
                "/api/documents/context",
                None,
                RequestOptions::default(),
                unavailable,
            )
            .await;
            if !alive.load(Ordering::Acquire) || self.epoch.get_untracked() != epoch {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(context) => {
                    let old = self.context.get_untracked();
                    let changed = old.as_ref().is_some_and(|old| {
                        old.organization_id != context.organization_id
                            || old.context_version != context.context_version
                    });
                    if changed {
                        self.selected.set(String::new());
                        self.list.set(None);
                        self.file_content.set(None);
                        self.file_name.set(String::new());
                        reset_file_picker();
                        self.description.set(String::new());
                        self.target.set(String::new());
                        self.signature_document.set(None);
                        self.changed();
                        self.report(
                            "A empresa ativa mudou. Selecione o vínculo para continuar.".into(),
                            false,
                        );
                    }
                    self.context.set(Some(context));
                    if self.scope().is_some() {
                        self.refresh(None);
                    }
                }
                Err(error) => {
                    self.context.set(None);
                    self.list.set(None);
                    self.report(error.details.message, true);
                }
            }
        });
    }
    fn choose(self, value: String) {
        self.epoch.update(|v| *v += 1);
        self.selected.set(value);
        self.list.set(None);
        self.target.set(String::new());
        self.file_content.set(None);
        self.file_name.set(String::new());
        reset_file_picker();
        self.description.set(String::new());
        self.signature_document.set(None);
        self.changed();
        self.refresh(None);
    }
    fn refresh(self, before: Option<String>) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.busy.set(true);
        let epoch = self.epoch.get_untracked();
        let mut url = format!(
            "/api/documents?organization_id={}&context_version={}&scope_id={}",
            query_component(&scope.organization_id),
            scope.context_version,
            query_component(&scope.scope_id)
        );
        if let Some(before) = before {
            url.push_str("&before=");
            url.push_str(&query_component(&before));
        }
        let alive = self.alive.get_value();
        leptos::task::spawn_local(async move {
            let result =
                request::<DocumentsList>(&url, None, RequestOptions::default(), unavailable).await;
            if !alive.load(Ordering::Acquire) || self.epoch.get_untracked() != epoch {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(value) => self.list.set(Some(value)),
                Err(error) => {
                    self.list.set(None);
                    self.report(error.details.message, true);
                }
            }
        });
    }
    fn upload(self) {
        if self.busy.get_untracked() {
            return;
        }
        let Some(scope) = self.scope() else {
            self.report(
                "Selecione a obra, o contrato, o orçamento ou o projeto.".into(),
                true,
            );
            return;
        };
        let Some(content) = self.file_content.get_untracked() else {
            self.report("Selecione o arquivo antes de enviar.".into(), true);
            return;
        };
        let key = if let Some(key) = self.operation_key.get_untracked() {
            key
        } else {
            match crate::http::operation_id() {
                Ok(key) => {
                    self.operation_key.set(Some(key.clone()));
                    key
                }
                Err(error) => {
                    self.report(error.message, true);
                    return;
                }
            }
        };
        let target = self.target.get_untracked();
        let (document_id, expected_version) = if let Some((id, version)) = target.split_once('@') {
            (Some(id.to_owned()), version.parse().unwrap_or(0))
        } else {
            (None, 0)
        };
        let input = UploadDocument {
            scope,
            operation_key: key,
            document_id,
            expected_version,
            file_name: self.file_name.get_untracked(),
            description: self.description.get_untracked(),
            category: self.category.get_untracked(),
            content_base64: content,
        };
        self.submit_upload("/api/documents/upload", serde_json::to_string(&input).ok());
    }
    fn retry(self, id: String) {
        let Some(scope) = self.scope() else {
            return;
        };
        self.submit_upload(
            "/api/documents/retry",
            serde_json::to_string(&RetryDocumentUpload {
                scope,
                upload_id: id,
            })
            .ok(),
        );
    }
    fn submit_upload(self, path: &'static str, body: Option<String>) {
        if self.busy.get_untracked() {
            return;
        }
        let Some(body) = body else {
            self.report(unavailable().message, true);
            return;
        };
        self.busy.set(true);
        self.report(
            "Recebendo e verificando o arquivo. Aguarde a confirmação.".into(),
            false,
        );
        let epoch = self.epoch.get_untracked();
        let csrf = self.csrf();
        let alive = self.alive.get_value();
        leptos::task::spawn_local(async move {
            let result = request::<DocumentUploadResult>(
                path,
                Some(body),
                RequestOptions {
                    csrf: Some(&csrf),
                    ..RequestOptions::default()
                },
                unavailable,
            )
            .await;
            if !alive.load(Ordering::Acquire) || self.epoch.get_untracked() != epoch {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(value) => {
                    self.report(value.message, value.status == UploadStatus::Rejected);
                    if value.status == UploadStatus::Accepted {
                        self.file_content.set(None);
                        self.file_name.set(String::new());
                        reset_file_picker();
                        self.target.set(String::new());
                        self.operation_key.set(None);
                    }
                    self.refresh(None);
                }
                Err(error) => self.report(error.details.message, true),
            }
        });
    }
    fn open(self, version: DocumentVersion, preview: bool) {
        let Some(scope) = self.scope() else {
            return;
        };
        if self.busy.get_untracked() {
            return;
        }
        let input = DocumentLinkRequest {
            scope,
            document_id: version.document_id,
            version: version.version,
            preview,
        };
        let Ok(body) = serde_json::to_string(&input) else {
            return;
        };
        let csrf = self.csrf();
        let epoch = self.epoch.get_untracked();
        self.busy.set(true);
        self.link.set(None);
        let alive = self.alive.get_value();
        leptos::task::spawn_local(async move {
            let result = request::<DocumentLink>(
                "/api/documents/link",
                Some(body),
                RequestOptions {
                    csrf: Some(&csrf),
                    ..RequestOptions::default()
                },
                unavailable,
            )
            .await;
            if !alive.load(Ordering::Acquire) || self.epoch.get_untracked() != epoch {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(link) => {
                    self.link.set(Some(link));
                    self.report("Link disponível abaixo por dois minutos. O acesso será conferido novamente ao abrir.".into(),false);
                }
                Err(error) => self.report(error.details.message, true),
            }
        });
    }
    fn sign(self) {
        let Some(scope) = self.scope() else {
            return;
        };
        let Some(document) = self.signature_document.get_untracked() else {
            return;
        };
        if self.busy.get_untracked() {
            return;
        }
        let input = RecordDocumentSignature {
            scope,
            document_id: document.document_id,
            version: document.version,
            sha256: document.sha256,
            signer: self.signer.get_untracked(),
            signed_at: signature_instant(&self.signed_at.get_untracked()),
            evidence: self.evidence.get_untracked(),
        };
        let Ok(body) = serde_json::to_string(&input) else {
            return;
        };
        let csrf = self.csrf();
        let epoch = self.epoch.get_untracked();
        self.busy.set(true);
        let alive = self.alive.get_value();
        leptos::task::spawn_local(async move {
            let result = request::<serde_json::Value>(
                "/api/documents/signature",
                Some(body),
                RequestOptions {
                    csrf: Some(&csrf),
                    ..RequestOptions::default()
                },
                unavailable,
            )
            .await;
            if !alive.load(Ordering::Acquire) || self.epoch.get_untracked() != epoch {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(_) => {
                    self.report("Evidência de assinatura registrada nesta versão. Isso não verifica sua autenticidade nem aprova o documento internamente.".into(),false);
                    self.signature_document.set(None);
                    self.refresh(None);
                }
                Err(error) => self.report(error.details.message, true),
            }
        });
    }
}

#[component]
pub fn DocumentsPage() -> impl IntoView {
    let docs = Documents::new();
    Effect::new(move |_| docs.load());
    view!{
        <Title text="Documentos e versões | Canteiro 360"/>
        <section class="documents-page" aria-labelledby="documents-title" data-documents-ready=move ||docs.context.get().is_some().to_string()>
            <div class="access-heading"><div><h1 id="documents-title">"Documentos e versões"</h1><p>"Anexe arquivos ao vínculo correto e preserve cada versão."</p></div><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.load()>"Atualizar acesso"</button></div>
            <p id="documents-feedback" tabindex="-1" aria-live="polite" aria-atomic="true" class=move ||if docs.error.get(){"notice danger"}else{"notice success"} hidden=move ||docs.feedback.get().is_empty()>{move ||docs.feedback.get()}</p>
            <p role="status" hidden=move ||!docs.busy.get()>"Aguardando confirmação do servidor…"</p>
            <Show when=move ||docs.context.with(|c|c.as_ref().is_none_or(|c|c.organization_id.is_none()))><div class="section-card"><h2>"Selecione sua empresa"</h2><p>"Entre e escolha a empresa para consultar os vínculos permitidos."</p><a href="/acesso">"Entrar"</a>" · "<a href="/organizacoes">"Escolher empresa"</a></div></Show>
            <Show when=move ||docs.context.with(|c|c.as_ref().is_some_and(|c|c.organization_id.is_some()))>
                <div class="section-card"><h2>{move ||docs.context.get().and_then(|c|c.organization_name).unwrap_or_default()}</h2>
                    <label for="documents-scope">"Obra, contrato, orçamento ou projeto"</label>
                    <select id="documents-scope" prop:value=move ||docs.selected.get() disabled=move ||docs.busy.get() on:change=move |e|docs.choose(event_target_value(&e))>
                        <option value="">"Escolha o vínculo do documento"</option>
                        {move ||docs.context.get().map(|c|c.scopes.into_iter().map(|scope|view!{<option value=scope.id>{scope.title}</option>}).collect_view())}
                    </select>
                    <p>"A lista mostra somente os vínculos que permitem sua consulta."</p>
                </div>
            </Show>
            <Show when=move ||!docs.selected.get().is_empty()>
                <Show when=move ||docs.can_upload()>
                    <form class="section-card" on:submit=move |e|{e.prevent_default();docs.upload();}>
                        <h2>"Enviar documento"</h2><p>"PDF sem conteúdo ativo, PNG, TXT ou CSV, até 5 MiB. O arquivo só fica disponível após a verificação de segurança."</p>
                        <label for="documents-target">"Documento"</label><select id="documents-target" prop:value=move ||docs.target.get() disabled=move ||docs.busy.get() on:change=move |e|{docs.target.set(event_target_value(&e));docs.changed();}>
                            <option value="">"Novo documento"</option>
                            {move ||docs.list.get().map(|l|l.versions.into_iter().filter(|v|v.active).map(|v|view!{<option value=format!("{}@{}",v.document_id,v.version)>{format!("Nova versão de {} (atual: {})",v.file_name,v.version)}</option>}).collect_view())}
                        </select>
                        <label for="documents-description">"Descrição"</label><input id="documents-description" required minlength="2" maxlength="500" prop:value=move ||docs.description.get() disabled=move ||docs.busy.get() on:input=move |e|{docs.description.set(event_target_value(&e));docs.changed();}/>
                        <label for="documents-category">"Categoria"</label><select id="documents-category" prop:value=move ||docs.category.get() disabled=move ||docs.busy.get() on:change=move |e|{docs.category.set(event_target_value(&e));docs.changed();}><option value="technical">"Técnico"</option><option value="contractual">"Contratual"</option><option value="budget">"Orçamento"</option><option value="evidence">"Evidência"</option><option value="other">"Outro"</option></select>
                        <label for="documents-file">"Arquivo"</label><input id="documents-file" type="file" accept=".pdf,.png,.txt,.csv" disabled=move ||docs.busy.get() on:change=move |event|{
                            #[cfg(feature="hydrate")]
                            {
                                use wasm_bindgen::JsCast;
                                use base64::Engine;
                                let input=event.target().and_then(|target|target.dyn_into::<web_sys::HtmlInputElement>().ok());
                                let file=input.and_then(|input|input.files()).and_then(|files|files.get(0));
                                docs.file_content.set(None);docs.file_name.set(String::new());docs.changed();
                                if let Some(file)=file{
                                    if file.size() as u64>canteiro_domain::documents::MAX_DOCUMENT_BYTES as u64{docs.report("Selecione um arquivo com até 5 MiB.".into(),true);return;}
                                    let name=file.name();let epoch=docs.epoch.get_untracked();let alive=docs.alive.get_value();docs.busy.set(true);docs.report("Preparando o arquivo para envio…".into(),false);
                                    leptos::task::spawn_local(async move{
                                        let result=wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                                        if !alive.load(Ordering::Acquire)||docs.epoch.get_untracked()!=epoch{return;}docs.busy.set(false);
                                        match result{Ok(buffer)=>{let bytes=js_sys::Uint8Array::new(&buffer).to_vec();match canteiro_domain::documents::inspect_content(&name,&bytes){Ok(_)=>{docs.file_content.set(Some(base64::engine::general_purpose::STANDARD.encode(bytes)));docs.file_name.set(name);docs.report("Arquivo preparado. Confira a descrição e escolha Enviar para verificação.".into(),false);},Err(error)=>docs.report(error.to_string(),true)}},Err(_)=>docs.report("Não foi possível ler o arquivo. Selecione-o novamente.".into(),true)}
                                    });
                                }
                            }
                            #[cfg(not(feature="hydrate"))]
                            let _=event;
                        }/>
                        <p>{move ||if docs.file_name.get().is_empty(){"Nenhum arquivo preparado.".into()}else{format!("Arquivo preparado: {}",docs.file_name.get())}}</p>
                        <button class="primary-button" type="submit" disabled=move ||docs.busy.get()||docs.file_content.get().is_none()>"Enviar para verificação"</button>
                    </form>
                </Show>
                <Show when=move ||!docs.can_upload()><p class="notice">"Seu vínculo permite consultar. Para anexar ou substituir arquivos, peça a permissão correspondente ao responsável."</p></Show>
                <div class="section-card"><h2>"Envios em acompanhamento"</h2>
                    {move ||docs.list.get().map(|list|if list.uploads.is_empty(){view!{<p>"Nenhum envio pendente ou recusado."</p>}.into_any()}else{list.uploads.into_iter().map(|upload|{let id=StoredValue::new(upload.id.clone());let retry=upload.status==UploadStatus::Quarantine;view!{<article class="notice"><h3>{upload.file_name}</h3><p>{upload.message}</p><Show when=move ||retry&&docs.can_upload()><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.retry(id.get_value())>"Verificar novamente"</button></Show></article>}}).collect_view().into_any()})}
                </div>
                <div class="section-card"><h2>"Versões disponíveis"</h2><p>"Assinaturas informadas ficam ligadas à versão e ao resumo do arquivo. Uma nova versão não recebe a assinatura da anterior."</p>
                    {move ||docs.list.get().map(|list|if list.versions.is_empty(){view!{<p>"Nenhuma versão liberada neste vínculo."</p>}.into_any()}else{list.versions.into_iter().map(|version|{let download=version.clone();let preview=StoredValue::new(version.clone());let signature=StoredValue::new(version.clone());let can_preview=version.content_type!="application/pdf";view!{
                        <article class="document-version"><h3>{format!("{} · versão {}",version.file_name,version.version)}</h3><p><strong>{if version.active{"Versão ativa"}else{"Versão anterior preservada"}}</strong></p><p>{version.description}</p><p>{format!("{} bytes · {} · {}",version.size,version.author,display_instant(&version.created_at))}</p>
                            <details><summary>"Resumo do arquivo e assinaturas"</summary><p class="document-hash">{format!("SHA-256: {}",version.sha256)}</p>{if version.signatures.is_empty(){view!{<p>"Sem evidência de assinatura registrada nesta versão."</p>}.into_any()}else{version.signatures.into_iter().map(|s|view!{<p>{format!("{} · {} · {}. Evidência informada, sem autenticação pelo sistema.",s.signer,display_instant(&s.signed_at),s.evidence)}</p>}).collect_view().into_any()}}</details>
                            <div class="history-actions"><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.open(download.clone(),false)>"Preparar download"</button><Show when=move ||can_preview><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.open(preview.get_value(),true)>"Preparar prévia segura"</button></Show><Show when=move ||docs.can_upload()><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|{docs.signature_document.set(Some(signature.get_value()));docs.signer.set(String::new());docs.signed_at.set(String::new());docs.evidence.set(String::new());}>"Registrar evidência de assinatura"</button></Show></div>
                        </article>
                    }}).collect_view().into_any()})}
                    <Show when=move ||docs.list.with(|l|l.as_ref().is_some_and(|l|l.truncated))><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|{let before=docs.list.get_untracked().and_then(|l|l.versions.last().map(|v|v.created_at.clone()));docs.refresh(before);}>"Ver versões anteriores"</button></Show>
                    <button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.refresh(None)>"Mostrar versões recentes"</button>
                </div>
            </Show>
            <Show when=move ||docs.link.get().is_some()><div class="section-card" role="status"><h2>"Arquivo disponível"</h2>{move ||docs.link.get().map(|link|view!{<a class="primary-button" href=link.url target="_blank" rel="noopener noreferrer">"Abrir arquivo verificado"</a><p>{format!("Link válido até {}. Mantém o acesso restrito à sua sessão.",display_instant(&link.expires_at))}</p>})}</div></Show>
            <Show when=move ||docs.signature_document.get().is_some()><form class="section-card" on:submit=move |event|{event.prevent_default();docs.sign();}><h2>"Registrar evidência de assinatura"</h2><p>{move ||docs.signature_document.get().map(|v|format!("{} · versão {}",v.file_name,v.version)).unwrap_or_default()}</p><p>"Este registro documenta a evidência recebida. Não autentica assinatura nem equivale à aprovação interna."</p><label for="documents-signer">"Nome do signatário"</label><input id="documents-signer" required minlength="2" maxlength="160" prop:value=move ||docs.signer.get() on:input=move |e|docs.signer.set(event_target_value(&e))/><label for="documents-signed-at">"Data e horário da assinatura (Brasília)"</label><input id="documents-signed-at" type="datetime-local" required prop:value=move ||docs.signed_at.get() on:input=move |e|docs.signed_at.set(event_target_value(&e))/><label for="documents-signature-evidence">"Referência da evidência de assinatura"</label><input id="documents-signature-evidence" required minlength="10" maxlength="500" prop:value=move ||docs.evidence.get() on:input=move |e|docs.evidence.set(event_target_value(&e))/><button type="submit" class="primary-button" disabled=move ||docs.busy.get()>"Registrar nesta versão"</button><button type="button" class="secondary-button" disabled=move ||docs.busy.get() on:click=move |_|docs.signature_document.set(None)>"Cancelar"</button></form></Show>
            <noscript><p class="notice">"Ative os scripts do navegador para consultar e anexar documentos."</p></noscript>
        </section>
    }.into_any()
}

fn focus_feedback() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("documents-feedback"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    }
}

fn reset_file_picker() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(input) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("documents-file"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            input.set_value("");
        }
    }
}

fn display_instant(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%d/%m/%Y às %H:%M (Brasília)")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_owned())
}
fn signature_instant(value: &str) -> String {
    chrono::NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .ok()
        .and_then(|date| {
            date.and_local_timezone(chrono_tz::America::Sao_Paulo)
                .single()
        })
        .map(|date| date.to_rfc3339())
        .unwrap_or_default()
}
