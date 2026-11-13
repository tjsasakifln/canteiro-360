//! Company management uses authenticated, versioned responses from the server.
mod model;
mod network;
mod views;

use leptos::prelude::*;
use leptos_meta::Title;
use model::Organizations;
use views::{ActiveWorkspace, ConfirmationDialog, JoinPanels, SelectionPanel};

#[component]
pub fn OrganizationsPage() -> impl IntoView {
    let organizations = Organizations::new();
    provide_context(organizations);
    Effect::new(move |_| organizations.load(false));
    #[cfg(feature = "hydrate")]
    {
        let focus_listener =
            window_event_listener(leptos::ev::focus, move |_| organizations.load(false));
        let page_listener =
            window_event_listener(leptos::ev::pageshow, move |_| organizations.load(false));
        let hide_listener =
            window_event_listener(leptos::ev::pagehide, move |_| organizations.clear_private());
        let leave_listener = window_event_listener(leptos::ev::beforeunload, move |event| {
            if organizations.dirty() {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || {
            focus_listener.remove();
            page_listener.remove();
            hide_listener.remove();
            leave_listener.remove();
        });
    }
    view! {
        <Title text="Empresas e equipe | Canteiro 360"/>
        <section class="prototype-shell organizations-page" aria-labelledby="org-title"
            data-org-ready=move || (organizations.ready.get() && !organizations.validating.get()).to_string()
            data-org-busy=move || organizations.busy.get().to_string()
            data-org-context=move || organizations.state.get().context_version.to_string()>
            <div class="access-heading">
                <div><p class="eyebrow">"Sua equipe, no contexto certo"</p><h1 id="org-title">"Empresas e acesso"</h1>
                    <p>"Escolha a empresa em que vai trabalhar. Controle convites e a participação da equipe."</p></div>
                <a id="org-account-link" class="text-link" href="/acesso" target="_blank" rel="noopener">"Conta e proteção ↗"</a>
            </div>
            <div id="org-feedback" tabindex="-1" class=move || if organizations.error.get().is_some() { "notice danger access-feedback" } else { "notice success access-feedback" }
                role=move || if organizations.error.get().is_some() { "alert" } else { "status" }
                aria-live="polite" aria-atomic="true" hidden=move || organizations.error.get().is_none() && organizations.message.get().is_empty()>
                <p>{move || organizations.error.get().map(|e| e.message).unwrap_or_else(|| organizations.message.get())}</p>
            </div>
            <div class="org-connection" hidden=move || organizations.ready.get() && !organizations.validating.get() && !organizations.needs_refresh()>
                <h2>"Conecte sua conta"</h2><p>"Entre com a mesma conta usada no convite. Depois, atualize o acesso nesta página."</p>
                <div class="form-actions"><a class="primary-button" href="/acesso" target="_blank" rel="noopener">"Abrir acesso em outra aba ↗"</a>
                    <button id="org-reconnect" type="button" class="secondary-button" disabled=move || organizations.busy.get() on:click=move |_| organizations.load(true)>"Atualizar acesso"</button></div>
                <noscript><p>"Ative os scripts para consultar e alterar o acesso às empresas. Nenhum formulário será enviado enquanto a interação estiver indisponível."</p></noscript>
            </div>
            <p class="org-progress" role="status" aria-live="polite">{move || if organizations.busy.get() { "Aguarde a confirmação do servidor." } else { "" }}</p>
            <Show when=move || organizations.ready.get() && !organizations.validating.get()>
                <SelectionPanel/>
                <Show when=move || organizations.suspended.get().is_some()><p class="notice warning">"Seu rascunho foi preservado nesta aba. Selecione a mesma empresa para retomá-lo. Os códigos privados foram removidos."</p></Show>
                <Show when=move || organizations.state.get().active.is_some()><ActiveWorkspace/></Show>
                <JoinPanels/>
            </Show>
            <p class="org-footnote">"A participação em uma empresa não libera automaticamente suas obras ou contratos. Esses acessos exigem permissões específicas."</p>
            <ConfirmationDialog/>
        </section>
    }.into_any()
}

pub(super) fn focus(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

fn open_dialog() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(dialog) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("org-confirm-dialog"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDialogElement>().ok())
            && dialog.show_modal().is_ok()
        {
            focus("org-confirm-cancel");
        }
    }
}

fn close_dialog() {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(dialog) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id("org-confirm-dialog"))
            .and_then(|e| e.dyn_into::<web_sys::HtmlDialogElement>().ok())
        {
            dialog.close();
        }
    }
}

pub(super) fn select_code(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(field) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        {
            let _ = field.focus();
            field.select();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

/// Native modal backgrounds are inert, but Chromium can still move Tab from
/// the final action to browser chrome. These two fixed actions form a circle.
pub(super) fn trap_dialog_tab(event: leptos::ev::KeyboardEvent) {
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
        let current = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.active_element())
            .map(|e| e.id());
        event.prevent_default();
        let next = match current.as_deref() {
            Some("org-confirm-cancel") => "org-confirm-submit",
            Some("org-confirm-submit") => "org-confirm-cancel",
            _ if event.shift_key() => "org-confirm-submit",
            _ => "org-confirm-cancel",
        };
        focus(next);
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = event;
}
