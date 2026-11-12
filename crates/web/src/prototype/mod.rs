//! Issue 3: accessible presentation prototypes with explicitly fictitious data.
mod components;
mod model;
mod pages;

use components::{Badge, close_dialog, focus, show_dialog};
use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::{
    NavigateOptions,
    hooks::{use_location, use_navigate, use_params_map},
};
use model::{Company, DemoState, Task};
use pages::{Approvals, Budget, Dashboard, Documents, Measurement};

#[derive(Clone, Copy)]
pub struct PrototypeStore {
    state: RwSignal<DemoState>,
    density: RwSignal<String>,
    last_context_url: RwSignal<Option<String>>,
}

/// The store lives above the router, preserving a draft while visiting another
/// task in this tab. It is tagged by company and never persisted to the server.
pub fn provide_prototype_store() {
    let store = PrototypeStore {
        state: RwSignal::new(DemoState::new(Company::Aurora)),
        density: RwSignal::new("comfortable".into()),
        last_context_url: RwSignal::new(None),
    };
    provide_context(store);

    // This lives with the draft, above route components. Visiting the public
    // home page must not remove the warning for an edited demonstration.
    #[cfg(feature = "hydrate")]
    {
        let handle = window_event_listener(leptos::ev::beforeunload, move |event| {
            if store.state.get_untracked().has_edits() {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || handle.remove());
    }
}

/// Validate public demonstration URLs without implying business authorization.
/// The HTTP boundary and the rendered route use the same context vocabulary.
pub fn is_supported_prototype_route(path: &str) -> bool {
    let mut parts = path.split('/');
    matches!(parts.next(), Some(""))
        && matches!(parts.next(), Some("prototipo"))
        && parts.next().and_then(Company::parse).is_some()
        && parts.next().and_then(Task::parse).is_some()
        && parts.next().is_none()
}

#[derive(Clone, Copy)]
pub(super) struct DemoContext {
    company: Memo<Company>,
    ready: RwSignal<bool>,
    store: PrototypeStore,
}

impl DemoContext {
    fn data(self) -> DemoState {
        self.store.state.get().for_company(self.company.get())
    }
    fn update(self, change: impl FnOnce(&mut DemoState)) {
        let company = self.company.get_untracked();
        self.store.state.update(|state| {
            if state.company != company {
                *state = DemoState::new(company);
            }
            change(state);
        });
    }
}

#[component]
pub fn PrototypePage() -> impl IntoView {
    let params = use_params_map();
    let valid = Memo::new(move |_| {
        params.with(|params| {
            Company::parse(&params.get("organization").unwrap_or_default()).is_some()
                && Task::parse(&params.get("task").unwrap_or_default()).is_some()
        })
    });
    view! { <Show when=move || valid.get() fallback=crate::NotFoundPage><PrototypeShell/></Show> }
}

#[component]
fn PrototypeShell() -> impl IntoView {
    let params = use_params_map();
    let location = use_location();
    let company = Memo::new(move |_| {
        Company::parse(&params.read().get("organization").unwrap_or_default()).unwrap_or_default()
    });
    let task = Memo::new(move |_| {
        Task::parse(&params.read().get("task").unwrap_or_default()).unwrap_or(Task::Dashboard)
    });
    let ready = RwSignal::new(false);
    let store = expect_context::<PrototypeStore>();
    let context = DemoContext {
        company,
        ready,
        store,
    };
    provide_context(context);
    let display_state = RwSignal::new("content".to_string());
    let pending_company = RwSignal::new(None::<Company>);
    let pending_url = RwSignal::new(None::<String>);
    let navigate = use_navigate();
    let navigate_confirm = navigate.clone();
    let navigate_select = navigate.clone();
    let navigate_guard = navigate.clone();
    let current_company = RwSignal::new(company.get_untracked());

    Effect::new(move |_| {
        let selected = company.get();
        let search = location.search.get();
        let hash = location.hash.get();
        let destination = format!(
            "{}{}{}{}",
            location.pathname.get(),
            if search.is_empty() { "" } else { "?" },
            search,
            hash
        );
        let previous_state = store.state.get_untracked();
        if selected != previous_state.company && previous_state.has_edits() {
            // `beforeunload` does not run on same-document Back/Forward. Keep
            // the draft, restore its last URL (including filters/fragment), and
            // ask before allowing the context change from any entry point.
            let restore = store
                .last_context_url
                .get_untracked()
                .unwrap_or_else(|| previous_state.company.url(Task::Measurement));
            pending_company.set(Some(selected));
            pending_url.set(Some(destination));
            // Push the restored context rather than overwriting the other
            // company's history entry. After cancel, Back can ask again.
            navigate_guard(
                &restore,
                NavigateOptions {
                    replace: false,
                    scroll: false,
                    ..Default::default()
                },
            );
            #[cfg(feature = "hydrate")]
            request_animation_frame(move || {
                if pending_company.try_get_untracked().flatten().is_some() {
                    show_dialog("context-dialog", "cancel-context");
                }
            });
            ready.set(true);
            return;
        }
        if selected != previous_state.company {
            store.state.set(DemoState::new(selected));
        }
        if current_company.get_untracked() != selected {
            current_company.set(selected);
            display_state.set("content".into());
        }
        store.last_context_url.set(Some(destination));
        ready.set(true);
    });

    let change_company = move |requested: Company| {
        if requested == company.get_untracked() {
            return;
        }
        if context.data().has_edits() {
            pending_company.set(Some(requested));
            pending_url.set(Some(requested.url(Task::Dashboard)));
            show_dialog("context-dialog", "cancel-context");
        } else {
            store.state.set(DemoState::new(requested));
            navigate_select(&requested.url(Task::Dashboard), Default::default());
        }
    };

    view! {
        <Title text=move || format!("{} | {} | Canteiro 360", task.get().label(), company.get().work())/>
        <section id="prototype-shell" class="prototype-shell"
            data-ready=move || ready.get().to_string()
            data-density=move || store.density.get()
            on:keydown=move |event| {
                if event.alt_key() && event.shift_key() && !event.ctrl_key() && !event.meta_key()
                    && event.key().eq_ignore_ascii_case("m") {
                    #[cfg(feature = "hydrate")]
                    {
                        use wasm_bindgen::JsCast;
                        let editing = event.target().and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                            .is_some_and(|element| matches!(element.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT") || element.get_attribute("contenteditable").as_deref() == Some("true"));
                        if !editing { event.prevent_default(); focus("prototype-main"); }
                    }
                }
            }
        >
            <header class="prototype-header">
                <div class="page-heading">
                    <div><p class="eyebrow">"Canteiro 360"</p><h2>"Sua obra, em contexto"</h2></div>
                    <Badge tone="neutral">"Demonstração"</Badge>
                </div>
                <p class="prototype-note">"Dados fictícios. As ações abaixo permitem experimentar a interface; não criam contratos, medições ou pagamentos reais."</p>
                <div class="context-grid" aria-label="Contexto atual">
                    <div class="field">
                        <label for="organization">"Empresa"</label>
                        <select id="organization" disabled=move || !ready.get() prop:value=move || company.get().key()
                            on:change=move |event| {
                                if let Some(requested) = Company::parse(&event_target_value(&event)) { change_company(requested); }
                            }
                        >
                            <option value="aurora" selected=move || company.get() == Company::Aurora>"Construtora Aurora"</option>
                            <option value="horizonte" selected=move || company.get() == Company::Horizonte>"Obras Horizonte"</option>
                        </select>
                    </div>
                    <div class="context-item"><span>"Obra"</span><strong id="current-work">{move || company.get().work()}</strong></div>
                    <div class="context-item"><span>"Contrato"</span><strong id="current-contract">{move || company.get().contract()}</strong></div>
                    <div class="context-item"><span>"Versão do orçamento"</span><strong id="current-version">{move || format!("Revisão {}", company.get().version())}</strong></div>
                </div>
                <details class="prototype-controls">
                <summary id="prototype-controls-toggle">"Opções da demonstração e ajuda"</summary>
                <div class="prototype-toolbar">
                    <div class="field"><label for="density">"Densidade"</label>
                        <select id="density" prop:value=move || store.density.get() disabled=move || !ready.get()
                            on:change=move |event| store.density.set(event_target_value(&event))>
                            <option value="comfortable">"Confortável"</option><option value="compact">"Compacta"</option>
                        </select>
                    </div>
                    <div class="field"><label for="display-state">"Cenário da demonstração"</label>
                        <select id="display-state" prop:value=move || display_state.get() disabled=move || !ready.get()
                            on:change=move |event| display_state.set(event_target_value(&event))>
                            <option value="content">"Com dados"</option><option value="empty">"Sem registros"</option>
                            <option value="loading">"Carregando"</option><option value="error">"Falha de leitura"</option>
                            <option value="forbidden">"Sem permissão"</option>
                        </select>
                    </div>
                    <details class="shortcut-help"><summary id="shortcuts-toggle">"Ajuda e teclado"</summary>
                        <p>"Use Tab para percorrer os controles, Enter para acionar e Escape para sair de diálogos. Alt + Shift + M leva ao conteúdo quando o foco está fora dos campos."</p>
                        <p>"O navegador também permite voltar à tarefa anterior. Seus textos permanecem nesta aba ao navegar entre tarefas da mesma empresa."</p>
                    </details>
                </div>
                </details>
                <div class="interaction-status prototype-readiness" data-ready=move || ready.get().to_string()>
                    <div hidden=move || ready.get()><p role="status">"Preparando os controles. As consultas abaixo já podem ser lidas."</p>
                        <div class="delayed-help"><p>"Os controles ainda não carregaram. Confira sua conexão."</p><a class="retry-link" href=move || company.get().url(task.get())>"Recarregar a página"</a></div>
                    </div>
                    <p hidden=move || !ready.get() role="status">"Demonstração pronta para usar."</p>
                    <noscript>"Ative os scripts para editar os exemplos e abrir diálogos. A consulta e os links permanecem disponíveis."</noscript>
                </div>
            </header>
            <nav class="task-nav" aria-label="Tarefas da obra">
                {Task::ALL.into_iter().map(|entry| view! {
                    <a id=format!("nav-{}", entry.key()) class="task-link"
                        href=move || company.get().url(entry)
                        aria-current=move || (task.get() == entry).then_some("page")
                    ><span class="task-symbol" aria-hidden="true">{entry.symbol()}</span>{entry.label()}</a>
                }).collect_view()}
            </nav>
            <section id="prototype-main" class="prototype-main" tabindex="-1" aria-keyshortcuts="Alt+Shift+M" aria-label="Conteúdo da tarefa">
                <Show when=move || display_state.get() == "content" fallback=move || view! { <DisplayState state=display_state/> }>
                    {move || match task.get() {
                        Task::Dashboard => view! { <Dashboard/> }.into_any(),
                        Task::Budget => view! { <Budget/> }.into_any(),
                        Task::Measurement => view! { <Measurement/> }.into_any(),
                        Task::Approvals => view! { <Approvals/> }.into_any(),
                        Task::Documents => view! { <Documents/> }.into_any(),
                    }}
                </Show>
            </section>
            <dialog id="context-dialog" class="prototype-dialog" aria-labelledby="context-title" aria-describedby="context-description"
                on:close=move |_| {
                    pending_company.set(None);
                    pending_url.set(None);
                    #[cfg(feature = "hydrate")]
                    {
                        use wasm_bindgen::JsCast;
                        if let Some(select) = web_sys::window().and_then(|w|w.document()).and_then(|d|d.get_element_by_id("organization"))
                            .and_then(|e|e.dyn_into::<web_sys::HtmlSelectElement>().ok()) { select.set_value(company.get_untracked().key()); }
                    }
                    focus("organization");
                }
            >
                <h2 id="context-title">"Trocar de empresa?"</h2>
                <p id="context-description">"Ao trocar, o rascunho e os resultados desta demonstração serão descartados. Nada foi enviado ao servidor."</p>
                <p>{move || pending_company.get().map(|next| format!("Destino: {} · {}.", next.name(), next.work())).unwrap_or_default()}</p>
                <div class="dialog-actions">
                    <button id="cancel-context" class="secondary-button" type="button" on:click=move |_| close_dialog("context-dialog")>"Continuar nesta empresa"</button>
                    <button id="confirm-context" class="primary-button" type="button" on:click=move |_| {
                        if let Some(next) = pending_company.get_untracked() {
                            let destination = pending_url.get_untracked().unwrap_or_else(|| next.url(Task::Dashboard));
                            store.state.set(DemoState::new(next));
                            close_dialog("context-dialog");
                            navigate_confirm(&destination, Default::default());
                        }
                    }>"Descartar e trocar empresa"</button>
                </div>
            </dialog>
        </section>
    }
}

#[component]
fn DisplayState(state: RwSignal<String>) -> impl IntoView {
    view! {
        {move || match state.get().as_str() {
            "empty" => view! { <div id="state-empty" class="empty-state"><h1>"Nenhum registro por aqui"</h1><p>"Este cenário está vazio. Abra os dados de exemplo para conhecer as tarefas e os documentos da obra."</p><button class="primary-button" on:click=move |_| state.set("content".into())>"Abrir dados de exemplo"</button></div> }.into_any(),
            "loading" => view! { <div id="state-loading" class="empty-state"><div role="status" aria-live="polite"><h1>"Carregando os registros"</h1><p>"Estamos simulando uma consulta. Nenhuma alteração está sendo enviada."</p></div><button id="finish-loading" class="secondary-button" on:click=move |_| state.set("content".into())>"Concluir carregamento simulado"</button></div> }.into_any(),
            "error" => view! { <div id="state-error" class="empty-state"><div role="alert"><h1>"Não foi possível consultar os registros"</h1><p>"Nenhuma gravação foi tentada. Seu rascunho nesta aba foi preservado. Tente consultar novamente."</p></div><button id="retry-content" class="primary-button" on:click=move |_| state.set("content".into())>"Tentar novamente"</button></div> }.into_any(),
            _ => view! { <div id="state-forbidden" class="empty-state"><h1>"Você não tem permissão para esta tarefa"</h1><p>"Solicite ao responsável da empresa o vínculo e o papel necessários nesta obra. Neste cenário, os registros não são exibidos."</p><p>"Este é um exemplo visual de bloqueio; não altera permissões reais."</p><button class="secondary-button" on:click=move |_| state.set("content".into())>"Voltar ao cenário com dados"</button></div> }.into_any(),
        }}
    }
}
