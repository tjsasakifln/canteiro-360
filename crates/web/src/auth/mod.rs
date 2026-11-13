//! Account access. Server responses determine every authenticated state.
//! Component boundaries erase nested view types so SSR code generation remains
//! bounded when account forms are composed with the application route tree.
mod model;
mod network;
mod views;

use canteiro_contracts::auth::AuthState;
use leptos::prelude::*;
use leptos_meta::Title;
use model::Access;
use views::{Account, AnonymousForms, MfaChallenge};

#[component]
pub fn AccessPage() -> impl IntoView {
    let access = Access::new();
    provide_context(access);
    Effect::new(move |_| access.load());
    view! {
        <Title text="Acesso à conta | Canteiro 360"/>
        <section class="prototype-shell access-page" aria-labelledby="access-title"
            data-auth-ready=move || access.ready.get().to_string()
            data-auth-state=move || match access.context.get().state {
                AuthState::Anonymous => "anonymous",
                AuthState::MfaPending => "mfa_pending",
                AuthState::Authenticated => "authenticated",
            }>
            <div class="access-heading">
                <div><p class="eyebrow">"Sua conta"</p><h1 id="access-title">"Acesso ao Canteiro 360"</h1>
                    <p>"Entre, recupere seu acesso ou confira a proteção da sua conta."</p></div>
                <a class="text-link" href="/">"Voltar ao início"</a>
            </div>
            <div class="access-layout">
                <div class="access-workspace">
                    <div class="access-connection" hidden=move || access.ready.get()>
                        <p role="status">{move || if access.busy.get() { "Preparando o acesso…" } else { "O acesso precisa de conexão com o servidor." }}</p>
                        <noscript><p>"Ative os scripts para enviar os formulários de acesso. Nenhuma senha será enviada enquanto a interação estiver indisponível."</p></noscript>
                    </div>
                    <div id="auth-feedback" tabindex="-1" class=move || if access.error.get().is_some() { "notice danger access-feedback" } else { "notice success access-feedback" }
                        role=move || if access.error.get().is_some() { "alert" } else { "status" }
                        aria-live="polite" aria-atomic="true"
                        hidden=move || access.error.get().is_none() && access.message.get().is_empty()>
                        <p>{move || access.error.get().map(|e| e.message).unwrap_or_else(|| access.message.get())}</p>
                        <Show when=move || access.needs_refresh()>
                            <p>"Restabeleça o acesso antes de tentar de novo. O formulário desta aba foi preservado; nenhuma operação será reenviada automaticamente."</p>
                        </Show>
                        <Show when=move || access.needs_refresh() || !access.ready.get()>
                            <button id="auth-reconnect" type="button" class="secondary-button" disabled=move || access.busy.get()
                                on:click=move |_| access.load()>"Restabelecer acesso"</button>
                        </Show>
                    </div>
                    <Show when=move || access.context.get().state == AuthState::Anonymous>
                        <AnonymousForms/>
                    </Show>
                    <Show when=move || access.context.get().state == AuthState::MfaPending>
                        <MfaChallenge/>
                    </Show>
                    <Show when=move || access.context.get().state == AuthState::Authenticated>
                        <Account/>
                    </Show>
                    <p class="access-progress" role="status" aria-live="polite">{move || if access.busy.get() { "Aguarde a confirmação do servidor." } else { "" }}</p>
                </div>
                <aside class="access-guidance" aria-labelledby="access-guidance-title">
                    <span class="access-guidance-symbol" aria-hidden="true">"360"</span>
                    <h2 id="access-guidance-title">"Sua rotina, com acesso protegido"</h2>
                    <ul>
                        <li>"Use seu gerenciador de senhas e cole os códigos recebidos."</li>
                        <li>"A recuperação de senha mantém a verificação em duas etapas."</li>
                        <li>"Você pode encerrar as sessões que não reconhece."</li>
                    </ul>
                    <div class="access-development"><strong>"Ambiente em desenvolvimento"</strong><p>"Use somente dados fictícios autorizados para os testes. Os módulos de obras reais ainda estão em preparação."</p></div>
                </aside>
            </div>
        </section>
    }.into_any()
}

fn focus(id: &str) {
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
