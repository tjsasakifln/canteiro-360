//! Permission management only displays authorization returned by the server.
mod format;
mod model;
mod network;
mod views;

use leptos::prelude::*;
use leptos_meta::Title;
use model::Access;

pub(crate) fn provide_draft_store() {
    model::provide_draft_store();
}

#[component]
pub fn PermissionsPage() -> impl IntoView {
    let access = Access::new();
    provide_context(access);
    Effect::new(move |_| access.load(false));
    #[cfg(feature = "hydrate")]
    {
        let focused = window_event_listener(leptos::ev::focus, move |_| access.load(false));
        let shown = window_event_listener(leptos::ev::pageshow, move |_| access.load(false));
        let hidden = window_event_listener(leptos::ev::pagehide, move |_| access.hide());
        let leaving = window_event_listener(leptos::ev::beforeunload, move |event| {
            if access.dirty() {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || {
            focused.remove();
            shown.remove();
            hidden.remove();
            leaving.remove();
        });
    }
    view! {
        <Title text="Permissões e aprovações | Canteiro 360"/>
        <section class="prototype-shell permissions-page" aria-labelledby="perm-title"
            data-perm-ready=move || (access.ready.get()&&!access.validating.get()).to_string()
            data-perm-busy=move || access.busy.get().to_string()>
            <div class="access-heading"><div><p class="eyebrow">"Cada pessoa, com o acesso necessário"</p>
                <h1 id="perm-title">"Permissões e aprovações"</h1>
                <p>"Defina quem pode atuar em cada obra, contrato e orçamento, até qual valor e por quanto tempo."</p>
            </div><a class="text-link" href="/organizacoes">"Empresas e equipe"</a></div>
            <div id="perm-feedback" class=move || if access.error.get().is_some(){"permission-notice error"}else{"permission-notice"}
                hidden=move || access.error.get().is_none()&&access.message.get().is_empty()
                role=move || if access.error.get().is_some(){"alert"}else{"status"}
                aria-live="polite" aria-atomic="true" tabindex="-1">
                <p>{move || access.error.get().map(|e|e.message).unwrap_or_else(||access.message.get())}</p>
            </div>
            <div class="permission-toolbar">
                <button id="perm-refresh" type="button" class="secondary-button" disabled=move || access.busy.get()
                    on:click=move |_| access.load(true)>"Atualizar acesso"</button>
                <p role="status" aria-live="polite">{move || if access.busy.get(){"Aguarde a confirmação do servidor."}else{""}}</p>
            </div>
            <Show when=move || !access.ready.get()||access.validating.get()>
                <div class="permission-empty"><h2>"Confirme seu acesso"</h2>
                    <p>"Entre na sua conta e selecione uma empresa. Ao voltar, atualize o acesso nesta página."</p>
                    <div class="form-actions"><a class="primary-button" href="/acesso" target="_blank" rel="noopener">"Abrir minha conta ↗"</a><a class="text-link" href="/organizacoes">"Escolher empresa"</a></div>
                    <noscript><p>"Ative os scripts do navegador para consultar e alterar permissões. Nenhum formulário será enviado enquanto a interação estiver indisponível."</p></noscript>
                </div>
            </Show>
            <Show when=move || access.ready.get()&&!access.validating.get()>
                <views::ContextPanel/>
                <Show when=move || access.state.with(|s|s.as_ref().is_some_and(|s|s.organization.is_some()))>
                    <views::ScopePanel/>
                    <Show when=move || !access.draft.get().selected_scope.is_empty()>
                        <views::Workspace/>
                    </Show>
                </Show>
            </Show>
            <p class="permission-help">"Administrar a empresa não dá aprovação técnica ou financeira automática. Toda decisão depende de vínculo, tarefa, valor, prazo e regra de aprovação."</p>
            <views::ConfirmationDialog/>
        </section>
    }.into_any()
}

pub(super) fn focus(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(e) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = e.focus();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}
pub(super) fn select_value(id: &str, value: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(select) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlSelectElement>().ok())
        {
            select.set_value(value);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (id, value);
}
#[cfg(feature = "hydrate")]
fn open_dialog() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(d) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("perm-confirm-dialog"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDialogElement>().ok())
            && d.show_modal().is_ok()
        {
            focus("perm-confirm-cancel");
        }
    }
}
fn close_dialog() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(d) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("perm-confirm-dialog"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDialogElement>().ok())
        {
            d.close();
        }
    }
}
#[cfg(feature = "hydrate")]
pub(super) fn open_details(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(d) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDetailsElement>().ok())
        {
            d.set_open(true);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}
pub(super) fn trap_tab(event: leptos::ev::KeyboardEvent) {
    #[cfg(feature = "hydrate")]
    {
        if event.key() != "Tab"
            || event.ctrl_key()
            || event.alt_key()
            || event.meta_key()
            || event.default_prevented()
        {
            return;
        }
        let id = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .map(|e| e.id());
        event.prevent_default();
        focus(match id.as_deref() {
            Some("perm-confirm-cancel") => "perm-confirm-submit",
            Some("perm-confirm-submit") => "perm-confirm-cancel",
            _ if event.shift_key() => "perm-confirm-submit",
            _ => "perm-confirm-cancel",
        });
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = event;
}
