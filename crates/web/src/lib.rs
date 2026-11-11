//! Public presentation for the first implementation stage.
//!
//! The only interaction changes a local disclosure. It neither persists data nor
//! represents authentication, an approval, or a financial operation.

use leptos::prelude::*;
use leptos_meta::{Meta, MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    components::{Route, Router, Routes},
    path,
};

/// Complete document rendered by the server. Only Leptos generates bootstrap JS.
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="pt-BR">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="color-scheme" content="light"/>
                <link rel="icon" href="data:,"/>
                <HydrationScripts options=options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// Shared route tree for server rendering and browser hydration.
#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="canteiro-style" href="/pkg/canteiro-web.css"/>
        <Title text="Canteiro 360 | Gestão de obras e projetos"/>
        <Meta
            name="description"
            content="Canteiro 360: gestão de obras e projetos privados. Ambiente em desenvolvimento, ainda sem operação comercial."
        />
        <Router>
            <a class="skip-link" href="#conteudo">"Pular para o conteúdo"</a>
            <header class="site-header">
                <div class="page-width header-inner">
                    <a class="brand" href="/" aria-label="Canteiro 360, página inicial">
                        <span class="brand-symbol" aria-hidden="true">"C"</span>
                        <span>"Canteiro "<strong>"360"</strong></span>
                    </a>
                    <span class="environment-label">"Em desenvolvimento"</span>
                </div>
            </header>
            <main id="conteudo" class="page-width main-content" tabindex="-1">
                <Routes fallback=NotFoundPage>
                    <Route path=path!("/") view=HomePage/>
                </Routes>
            </main>
            <footer class="site-footer page-width">
                <p>"Canteiro 360 · Gestão de obras e projetos privados"</p>
                <p>"Uma solução da CONFENGE"</p>
            </footer>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    // Keep the first browser render equal to SSR. Effects run only after the
    // shared view exists, so readiness never changes the hydration structure.
    let (ready, set_ready) = signal(false);
    let (expanded, set_expanded) = signal(false);
    Effect::new(move |_| set_ready.set(true));

    view! {
        <section class="intro" aria-labelledby="page-title">
            <p class="eyebrow">"Do planejamento ao encerramento"</p>
            <h1 id="page-title">"Cada etapa da obra, com clareza."</h1>
            <p class="intro-copy">
                "O Canteiro 360 está sendo preparado para conectar obras, projetos, "
                "contratos e acompanhamento financeiro em um só lugar."
            </p>
        </section>

        <section class="foundation-card" aria-labelledby="stage-title">
            <div class="card-heading">
                <span class="stage-marker" aria-hidden="true">"01"</span>
                <div>
                    <p class="eyebrow">"Primeira etapa"</p>
                    <h2 id="stage-title">"A base está em construção"</h2>
                </div>
            </div>
            <p class="stage-description">
                "Esta página apresenta o início do produto. Cadastro, acesso às obras "
                "e registros financeiros ainda não estão disponíveis."
            </p>

            <div class="scope-note">
                <strong>"Ambiente de desenvolvimento"</strong>
                <p>"Ainda não está liberado para operação comercial ou inclusão de dados de clientes."</p>
            </div>

            <div class="interaction-status" data-ready=move || ready.get().to_string()>
                <div class="waiting-state" hidden=move || ready.get()>
                    <p class="loading-label" role="status" aria-live="polite">
                        <span class="status-dot pending" aria-hidden="true"></span>
                        "Preparando a interação desta página…"
                    </p>
                    <div class="delayed-help" role="status">
                        <p>"O carregamento ainda não terminou. Confira sua conexão e tente novamente."</p>
                        <a class="retry-link" href="/">"Recarregar a página"</a>
                    </div>
                </div>
                <p class="ready-state" hidden=move || !ready.get() role="status" aria-live="polite">
                    <span class="status-dot available" aria-hidden="true"></span>
                    "Página pronta para interação."
                </p>
                <noscript class="noscript-help">
                    "Os scripts estão desativados no navegador. Você pode ler esta página; para abrir os detalhes abaixo, permita os scripts e recarregue."
                </noscript>
            </div>

            <button
                id="foundation-details-toggle"
                class="primary-button"
                type="button"
                disabled=move || !ready.get()
                aria-expanded=move || expanded.get().to_string()
                aria-controls="foundation-details"
                on:click=move |_| set_expanded.update(|value| *value = !*value)
            >
                {move || if expanded.get() {
                    "Ocultar detalhes desta etapa"
                } else {
                    "Ver o que já está disponível"
                }}
            </button>

            <div id="foundation-details" class="details-panel" hidden=move || !expanded.get()>
                <h3>"O que você pode conferir agora"</h3>
                <ul>
                    <li>"Ler a apresentação antes de terminar o carregamento da interação."</li>
                    <li>"Abrir e fechar estes detalhes com mouse, toque ou teclado."</li>
                    <li>"Consultar esta página também em telas pequenas."</li>
                </ul>
                <p>"Abrir estes detalhes apenas muda a apresentação nesta aba. Nenhum registro é criado."</p>
            </div>
        </section>

        <section class="next-stage" aria-labelledby="next-title">
            <h2 id="next-title">"Próximas etapas do produto"</h2>
            <p>
                "Organizações, controle de acesso e os módulos de gestão serão entregues "
                "em etapas, com conferência das regras antes da liberação."
            </p>
        </section>
    }
}

#[component]
fn NotFoundPage() -> impl IntoView {
    view! {
        <Title text="Página não encontrada | Canteiro 360"/>
        <section class="not-found" aria-labelledby="not-found-title">
            <p class="eyebrow">"Página não encontrada"</p>
            <h1 id="not-found-title">"Este endereço não está disponível."</h1>
            <p>"Confira o endereço ou volte à apresentação do Canteiro 360."</p>
            <a class="primary-button" href="/">"Voltar ao início"</a>
        </section>
    }
}

/// Called by Leptos' generated loader after the WebAssembly module is available.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
