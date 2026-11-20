//! Account access and accessible previews. Prototype interactions remain fictitious.

// Leptos composes nested HTML into Rust types; the form tree exceeds the
// compiler's default query depth. This changes compilation only, not runtime.
#![recursion_limit = "256"]

use leptos::prelude::*;
use leptos_meta::{Meta, MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    SsrMode,
    components::{Route, Router, Routes},
    path,
};

mod access;
mod auth;
mod calculations;
mod documents;
pub mod form_components;
mod http;
#[cfg(feature = "ssr")]
pub mod obligations;
mod organizations;
mod parties;
mod portfolio;
#[cfg(feature = "ssr")]
pub mod projects;
mod prototype;
mod reliability;
pub use prototype::is_supported_prototype_route;

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
    prototype::provide_prototype_store();
    access::provide_draft_store();
    reliability::provide_draft_store();

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
                    <nav class="site-account-nav" aria-label="Conta"><a class="account-link" href="/organizacoes">"Empresas"</a><a class="account-link" href="/permissoes">"Permissões"</a><a class="account-link" href="/historico">"Histórico"</a><a class="account-link" href="/acesso">"Entrar"</a><span class="environment-label">"Em desenvolvimento"</span></nav>
                </div>
            </header>
            <nav class="page-width workspace-navigation" aria-label="Rotina da empresa">
                <a href="/carteira">"Carteira de obras"</a>
                <a href="/projetos" rel="external">"Projetos e entregas"</a>
                <a href="/documentos">"Documentos"</a>
                <a href="/obrigacoes" rel="external">"Obrigações técnicas"</a>
                <a href="/cadastros">"Pessoas e empresas"</a>
            </nav>
            <main id="conteudo" class="page-width main-content" tabindex="-1">
                <Routes fallback=NotFoundPage>
                    <Route path=path!("/") view=HomePage/>
                    <Route path=path!("/acesso") view=auth::AccessPage/>
                    <Route path=path!("/organizacoes") view=organizations::OrganizationsPage/>
                    <Route path=path!("/permissoes") view=access::PermissionsPage/>
                    <Route path=path!("/historico") view=reliability::HistoryPage/>
                    <Route path=path!("/documentos") view=documents::DocumentsPage/>
                    <Route path=path!("/cadastros") view=parties::PartiesPage/>
                    <Route path=path!("/carteira") view=portfolio::PortfolioPage/>
                    <Route path=path!("/conferencia") view=calculations::CalculationPage/>
                    // Resolve the page on the server before emitting its HTML.
                    // OutOfOrder streaming places suspended rows in an inert
                    // template that requires JavaScript to become visible.
                    <Route path=path!("/prototipo/:organization/:task") view=prototype::PrototypePage ssr=SsrMode::Async/>
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
            <p class="prototype-entry"><a class="primary-button" href="/prototipo/aurora/painel">"Experimentar os percursos da obra"</a></p>
            <p><a class="text-link" href="/conferencia">"Conferir valores e prazos"</a></p>
        </section>

        <section class="foundation-card" aria-labelledby="stage-title">
            <div class="card-heading">
                <span class="stage-marker" aria-hidden="true">"01"</span>
                <div>
                    <p class="eyebrow">"Primeira etapa"</p>
                    <h2 id="stage-title">"Conheça a base e os percursos da obra"</h2>
                </div>
            </div>
            <p class="stage-description">
                "Os exemplos de painel, orçamento, medição, aprovação e documentos já podem ser "
                "experimentados. A conta e a gestão de empresas estão disponíveis para testes; obras reais e registros financeiros ainda estão em preparação."
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
                    <li>"Consultar os exemplos da rotina da obra com dados fictícios."</li>
                    <li>"Navegar, conferir um rascunho e revisar uma decisão com teclado, mouse ou toque."</li>
                    <li>"Conferir o contexto da empresa e os estados de carregamento, erro e permissão."</li>
                </ul>
                <p>"Abrir estes detalhes apenas muda a apresentação nesta aba. Nenhum registro é criado."</p>
            </div>
        </section>

        <section class="next-stage" aria-labelledby="next-title">
            <h2 id="next-title">"Próximas etapas do produto"</h2>
            <p>
                "Permissões por obra e os módulos de gestão serão entregues "
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
