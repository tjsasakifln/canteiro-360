use super::{
    draft::{Confirmation, instant},
    model::{History, Reservation},
    network,
};
use canteiro_contracts::reliability::HistoryEntry;
use leptos::prelude::*;

#[component]
pub(super) fn ContractSelection() -> impl IntoView {
    let history = expect_context::<History>();
    let contracts = move || {
        history
            .context
            .get()
            .and_then(|c| c.organization)
            .map(|o| o.contracts)
            .unwrap_or_default()
    };
    view! {
        <section class="section-card" aria-labelledby="history-company">
            <div class="history-section-heading">
                <div><p class="history-meta">"Empresa ativa"</p>
                    <h2 id="history-company">{move || history.context.get().and_then(|c| c.organization).map(|o| o.name).unwrap_or_default()}</h2>
                </div>
                <a class="text-link" href="/organizacoes">"Escolher empresa"</a>
            </div>
            <Show when=move || contracts().is_empty()>
                <p>"Sua conta ainda não tem contrato disponível nesta empresa. Solicite o acesso à equipe responsável."</p>
            </Show>
            <Show when=move || !contracts().is_empty()>
                <form method="post" action="/historico" class="history-select"
                    on:submit=move |event| { event.prevent_default(); history.open(); }>
                    <div class="field"><label for="history-contract">"Contrato"</label>
                        <select id="history-contract" disabled=move || history.busy.get()
                            on:change=move |event| history.selection.set(event_target_value(&event))>
                            <option value="" selected=move || history.selection.get().is_empty()>"Selecione um contrato"</option>
                            <For each=contracts key=|contract| (contract.id.clone(), contract.title.clone()) children=move |contract| {
                                let selected_id = contract.id.clone();
                                view! { <option value=contract.id selected=move || history.selection.with(|id| id == &selected_id)>{contract.title}</option> }
                            }/>
                        </select>
                    </div>
                    <button id="history-open" type="submit" class="primary-button" disabled=move || !history.can_open()>"Abrir contrato"</button>
                </form>
            </Show>
        </section>
    }.into_any()
}

#[component]
pub(super) fn ContractWorkspace() -> impl IntoView {
    let history = expect_context::<History>();
    view! {
        <section class="section-card" aria-labelledby="history-contract-heading">
            <div class="history-section-heading">
                <div><p class="history-meta">"Título salvo no servidor"</p>
                    <h2 id="history-contract-heading" tabindex="-1">{move || history.detail.get().map(|d| d.contract.title).unwrap_or_default()}</h2>
                    <p id="history-current-version" class="history-meta">{move || history.detail.get().map(|d| format!("Versão {}", d.contract.version))}</p>
                </div>
                <button id="history-update-contract" type="button" class="secondary-button" disabled=move || history.busy.get()
                    on:click=move |_| history.refresh_detail(false)>"Atualizar contrato"</button>
            </div>
        </section>
        <Show when=move || history.draft.get().is_some()>
            <Editor/>
        </Show>
        <RevisionList/>
    }.into_any()
}

#[component]
fn Editor() -> impl IntoView {
    let history = expect_context::<History>();
    let pending = move || {
        history
            .draft
            .with(|draft| draft.as_ref().and_then(|d| d.pending.as_ref()).is_some())
    };
    let draft_label = move || {
        let actor = history
            .context
            .get()
            .map(|c| c.actor_label)
            .unwrap_or_default();
        history.draft.get().map(|draft| {
            if draft.dirty() {
                format!(
                    "Rascunho nesta aba · versão {} · {}{}",
                    draft.base_version,
                    actor,
                    draft
                        .edited_at
                        .as_deref()
                        .map(|time| format!(" · {} (Brasília)", instant(time)))
                        .unwrap_or_default(),
                )
            } else {
                format!("Base da edição · versão {} · {}", draft.base_version, actor)
            }
        })
    };
    view! {
        <section class="section-card history-editor" aria-labelledby="history-editor">
            <h2 id="history-editor">"Editar o título cadastral"</h2>
            <p>"Prepare o rascunho nesta aba. Para salvar, reserve a edição pela sua sessão."</p>
            <LeaseControls/>
            <div id="history-draft-status" class="history-meta">{draft_label}</div>
            <Show when=move || history.conflicting()>
                <Conflict/>
            </Show>
            <Show when=pending>
                <div id="history-pending" class="notice warning" tabindex="-1">
                    <Show when=move || history.confirmation() == Some(Confirmation::KeyConflict) fallback=move || view! {
                        <h3>"Gravação sem confirmação"</h3>
                        <p>"Consulte o resultado desta tentativa. Enquanto aguarda, seus campos ficam preservados."</p>
                        <button id="history-check" type="button" class="secondary-button" disabled=move || history.busy.get()
                            on:click=move |_| history.check_operation()>"Consultar resultado"</button>
                    }>
                        <h3>"A identificação da tentativa já está em uso"</h3>
                        <p>"Confira abaixo o que consta no histórico. Você pode voltar aos campos e iniciar outra gravação."</p>
                        <button id="history-resolve-key" type="button" class="secondary-button" disabled=move || history.busy.get()
                            on:click=move |_| history.resolve_key_conflict()>"Retomar rascunho"</button>
                    </Show>
                </div>
            </Show>
            <form method="post" action="/historico" novalidate
                on:submit=move |event| { event.prevent_default(); history.save(); }>
                <div class="history-fields">
                    <div class="field validated-field">
                        <label for="history-title">"Título "<span class="field-required">"Obrigatório"</span></label>
                        <input id="history-title" type="text" required maxlength="160" autocomplete="off"
                            disabled=move || history.busy.get() || pending()
                            aria-invalid=move || (history.invalid_field.get() == Some("history-title")).to_string()
                            aria-describedby=move || (history.invalid_field.get() == Some("history-title")).then_some("history-feedback")
                            prop:value=move || history.draft.get().map(|d| d.title).unwrap_or_default()
                            on:input=move |event| history.set_field(|draft| draft.title = event_target_value(&event))/>
                    </div>
                    <div class="field validated-field">
                        <label for="history-reason">"Motivo da alteração "<span class="field-required">"Obrigatório"</span></label>
                        <input id="history-reason" type="text" required maxlength="200" autocomplete="off"
                            disabled=move || history.busy.get() || pending()
                            aria-invalid=move || (history.invalid_field.get() == Some("history-reason")).to_string()
                            aria-describedby=move || (history.invalid_field.get() == Some("history-reason")).then_some("history-feedback")
                            prop:value=move || history.draft.get().map(|d| d.reason).unwrap_or_default()
                            on:input=move |event| history.set_field(|draft| draft.reason = event_target_value(&event))/>
                    </div>
                    <div class="field validated-field">
                        <label for="history-occurred">"Data e hora da ocorrência "<span class="field-required">"Opcional"</span></label>
                        <input id="history-occurred" type="text" maxlength="19" autocomplete="off"
                            disabled=move || history.busy.get() || pending()
                            aria-describedby=move || if history.invalid_field.get() == Some("history-occurred") { "history-occurred-help history-feedback" } else { "history-occurred-help" }
                            aria-invalid=move || (history.invalid_field.get() == Some("history-occurred")).to_string()
                            prop:value=move || history.draft.get().map(|d| d.occurred_at).unwrap_or_default()
                            on:input=move |event| history.set_field(|draft| draft.occurred_at = event_target_value(&event))/>
                        <p id="history-occurred-help" class="field-hint">"Ocorrência anterior, no horário de Brasília. Exemplo 14/11/2026 09:30."</p>
                    </div>
                </div>
                <div class="history-actions">
                    <button id="history-save" type="submit" class="primary-button" disabled=move || !history.can_submit()>
                        {move || if pending() { "Repetir a mesma gravação" } else { "Salvar título" }}
                    </button>
                    <button id="history-discard" type="button" class="secondary-button"
                        disabled=move || history.busy.get() || pending() || history.draft.with(|d| d.as_ref().is_none_or(|d| !d.dirty()))
                        on:click=move |_| history.discard()>"Descartar rascunho"</button>
                </div>
            </form>
        </section>
    }.into_any()
}

#[component]
fn LeaseControls() -> impl IntoView {
    let history = expect_context::<History>();
    view! {
        <div class="history-lease">
            <p id="history-lease-status" tabindex="-1">
                {move || {
                    if let Some(until) = history.mine() {
                        format!("Edição reservada nesta aba até {} (Brasília).", instant(&until))
                    } else if let Some(until) = history.detail.get().and_then(|d| d.lease_until) {
                        format!("Há uma reserva ativa até {} (Brasília). Se foi feita nesta aba, retome a reserva para salvar.", instant(&until))
                    } else {
                        "Edição disponível para reserva por dois minutos.".to_owned()
                    }
                }}
            </p>
            <div class="history-actions">
                <Show when=move || history.mine().is_none()>
                    <button id="history-acquire" type="button" class="secondary-button" disabled=move || history.busy.get()
                        on:click=move |_| history.reserve(Reservation::Acquire)>
                        {move || if history.detail.with(|d|d.as_ref().is_some_and(|d| d.lease_until.is_some())) { "Retomar ou tentar reserva" } else { "Reservar edição" }}
                    </button>
                </Show>
                <Show when=move || history.mine().is_some()>
                    <button id="history-renew" type="button" class="secondary-button" disabled=move || history.busy.get()
                        on:click=move |_| history.reserve(Reservation::Renew)>"Renovar reserva"</button>
                    <button id="history-release" type="button" class="secondary-button" disabled=move || history.busy.get()
                        on:click=move |_| history.reserve(Reservation::Release)>"Liberar reserva"</button>
                </Show>
            </div>
        </div>
    }.into_any()
}

#[component]
fn Conflict() -> impl IntoView {
    let history = expect_context::<History>();
    view! {
        <div class="notice warning" id="history-conflict" aria-labelledby="history-conflict-heading">
            <h3 id="history-conflict-heading" tabindex="-1">"Outra alteração foi salva"</h3>
            <p>"Confira o que mudou. Seu rascunho continua nesta aba e só será gravado se você escolher salvar novamente."</p>
            <dl class="history-diff">
                <div><dt>{move || history.draft.get().map(|d| format!("Base · versão {}", d.base_version))}</dt>
                    <dd id="history-conflict-base">{move || history.draft.get().map(|d| d.base_title)}</dd></div>
                <div><dt>{move || history.detail.get().map(|d| format!("Salvo no servidor · versão {}", d.contract.version))}</dt>
                    <dd id="history-conflict-saved">{move || history.detail.get().map(|d| d.contract.title)}</dd></div>
                <div><dt>"Seu rascunho"</dt><dd id="history-conflict-draft">{move || history.draft.get().map(|d| d.title)}</dd></div>
            </dl>
            <button id="history-rebase" type="button" class="secondary-button"
                disabled=move || history.busy.get() || history.draft.with(|d| d.as_ref().is_none_or(|d| d.pending.is_some()))
                on:click=move |_| history.rebase()><span>"Manter rascunho e usar a versão salva como base"</span></button>
        </div>
    }.into_any()
}

#[component]
fn RevisionList() -> impl IntoView {
    let history = expect_context::<History>();
    view! {
        <section class="section-card" aria-labelledby="history-revisions">
            <div class="history-section-heading">
                <div><h2 id="history-revisions">"Revisões registradas"</h2>
                    <p class="history-meta">"Ordem de registro. Horários de Brasília."</p></div>
                <a id="history-report" class="secondary-button" download="historico-do-contrato.csv"
                    href=move || history.scope().and_then(|scope| history.detail.get().map(|d| network::history_url(&scope, &d.contract.id))).unwrap_or_default()>
                    "Baixar histórico CSV"
                </a>
            </div>
            <Show when=move || history.detail.with(|d| d.as_ref().is_some_and(|d| d.history.is_empty()))>
                <p>"Ainda não há revisões registradas para este contrato."</p>
            </Show>
            <ol class="history-entries">
                <For each=move || history.detail.get().map(|d| {
                    d.history.into_iter().map(|entry| {
                        let notice = d.notifications.iter().any(|n| n.event_id == entry.event_id);
                        (entry, notice)
                    }).collect::<Vec<_>>()
                }).unwrap_or_default()
                    key=|(entry, notice)| (entry.event_id.clone(), *notice)
                    children=move |(entry, notice)| view! { <RevisionItem entry notice/> }/>
            </ol>
            <p id="history-new-entries" class="history-meta" tabindex="-1">{move || history.detail.get().map(|d| format!("{} revisões exibidas", d.history.len()))}</p>
            <Show when=move || history.detail.with(|d| d.as_ref().is_some_and(|d| d.next_after.is_some()))>
                <button id="history-more" type="button" class="secondary-button" disabled=move || history.busy.get()
                    on:click=move |_| history.refresh_detail(true)>"Carregar mais revisões"</button>
            </Show>
        </section>
    }.into_any()
}

#[component]
fn RevisionItem(entry: HistoryEntry, notice: bool) -> impl IntoView {
    let label = match entry.action.as_str() {
        "contract_reference_created" => "Título cadastrado",
        "contract_reference_renamed" => "Título alterado",
        _ => "Revisão registrada",
    };
    let occurrence = instant(&entry.occurred_at);
    let recorded = instant(&entry.recorded_at);
    let origin = match entry.origin.as_str() {
        "cadastro" => "Cadastro",
        "web" => "Aplicação web",
        "migração" => "Migração",
        _ => "Origem registrada",
    };
    view! {
        <li class="history-entry">
            <div class="history-section-heading">
                <div><h3>{format!("Versão {} · {label}", entry.version)}</h3>
                    <p class="history-meta">{format!("{} · {}", entry.actor_label, origin)}</p></div>
                {entry.late.then(|| view! { <strong class="history-late">"Registro retroativo"</strong> })}
            </div>
            <dl class="history-diff">
                <div><dt>"Ocorrência"</dt><dd><time datetime=entry.occurred_at>{occurrence}</time></dd></div>
                <div><dt>"Registro"</dt><dd><time datetime=entry.recorded_at>{recorded}</time></dd></div>
            </dl>
            <dl class="history-diff">
                <div><dt>"Título anterior"</dt><dd>{entry.before.unwrap_or_else(|| "Primeiro cadastro".into())}</dd></div>
                <div><dt>"Título registrado"</dt><dd>{entry.after}</dd></div>
                <div><dt>"Motivo"</dt><dd>{entry.reason}</dd></div>
            </dl>
            {notice.then(|| view! { <p class="history-meta">"Confirmação desta gravação disponível para a sua conta."</p> })}
        </li>
    }.into_any()
}
