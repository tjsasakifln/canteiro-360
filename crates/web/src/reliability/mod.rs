//! Contratos, histórico e recuperação de uma edição confirmada no servidor.
mod draft;
mod model;
mod network;
mod views;

use leptos::prelude::*;
use leptos_meta::Title;
use model::History;

pub(crate) fn provide_draft_store() {
    model::provide_draft_store();
}

#[component]
pub fn HistoryPage() -> impl IntoView {
    let history = History::new();
    provide_context(history);
    Effect::new(move |_| history.load());
    #[cfg(feature = "hydrate")]
    {
        let focus = window_event_listener(leptos::ev::focus, move |_| history.revalidate());
        let shown = window_event_listener(leptos::ev::pageshow, move |_| history.revalidate());
        let hidden = window_event_listener(leptos::ev::pagehide, move |_| history.hide());
        let timer = set_interval_with_handle(
            move || history.elapsed_lease(),
            std::time::Duration::from_secs(1),
        );
        on_cleanup(move || {
            focus.remove();
            shown.remove();
            hidden.remove();
            if let Ok(timer) = timer {
                timer.clear();
            }
        });
    }
    view! {
        <Title text="Histórico de contratos | Canteiro 360"/>
        <section class="prototype-shell history-page" aria-labelledby="history-heading"
            data-history-ready=move || (history.context.get().is_some() && !history.busy.get()).to_string()
            data-history-busy=move || history.busy.get().to_string()>
            <div class="access-heading">
                <div><h1 id="history-heading">"Histórico de contratos"</h1>
                    <p>"Consulte as versões do título cadastral e quem registrou cada mudança."</p></div>
                <button id="history-refresh" type="button" class="secondary-button"
                    disabled=move || history.busy.get() on:click=move |_| history.load()>"Atualizar acesso"</button>
            </div>
            <p id="history-feedback" tabindex="-1" aria-live="polite" aria-atomic="true"
                class=move || if history.error.get().is_some() { "notice danger" } else { "notice success" }
                hidden=move || history.error.get().is_none() && history.message.get().is_empty()>
                {move || history.error.get().map(|value| value.message).unwrap_or_else(|| history.message.get())}
            </p>
            <p class="history-progress" role="status">{move || if history.busy.get() { "Aguardando confirmação do servidor…" } else { "" }}</p>
            <Show when=move || history.context.get().is_none() && !history.busy.get()>
                <div class="section-card">
                    <h2>"Confirme seu acesso"</h2>
                    <p>"Entre e selecione a empresa para consultar os contratos disponíveis para sua conta."</p>
                    <div class="history-actions"><a class="secondary-button" href="/acesso">"Entrar"</a>
                        <a class="text-link" href="/organizacoes">"Escolher empresa"</a></div>
                </div>
            </Show>
            <noscript><p class="notice warning">"Ative os scripts do navegador para consultar o histórico e editar contratos."</p></noscript>
            <Show when=move || history.context.with(|c| c.as_ref().is_some_and(|c| c.organization.is_none()))>
                <div class="section-card">
                    <h2>"Selecione uma empresa"</h2>
                    <a class="text-link" href="/organizacoes">"Escolher empresa"</a>
                </div>
            </Show>
            <Show when=move || history.context.with(|c| c.as_ref().is_some_and(|c| c.organization.is_some()))>
                <views::ContractSelection/>
            </Show>
            <Show when=move || history.detail.get().is_some()>
                <views::ContractWorkspace/>
            </Show>
        </section>
    }.into_any()
}

#[cfg(feature = "hydrate")]
fn focus(id: &str) {
    use wasm_bindgen::JsCast;
    if let Some(element) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id(id))
        .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
    {
        let _ = element.focus();
    }
}
