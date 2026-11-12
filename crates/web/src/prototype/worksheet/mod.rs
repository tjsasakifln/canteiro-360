mod editor;
mod fields;
mod network;
mod state;
mod table;

pub use state::WorksheetState;

use super::{DemoContext, components::*, model::Task};
use canteiro_contracts::forms::{MeasurementPage, MeasurementQuery};
use leptos::prelude::*;
use leptos_router::{
    NavigateOptions,
    hooks::{use_navigate, use_query_map},
};
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
struct TableState {
    selected: RwSignal<BTreeSet<u32>>,
    all_results: RwSignal<bool>,
    copy_data: RwSignal<Option<String>>,
    selection_notice: RwSignal<String>,
}

impl TableState {
    fn clear(self) {
        let had_selection =
            self.all_results.get_untracked() || !self.selected.get_untracked().is_empty();
        self.selected.set(BTreeSet::new());
        self.all_results.set(false);
        self.copy_data.set(None);
        if had_selection {
            self.selection_notice.set(
                "Seleção limpa ao mudar a consulta. Os rascunhos desta empresa foram preservados."
                    .into(),
            );
        }
    }
}

#[component]
pub(super) fn Worksheet() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let params = use_query_map();
    let query = Memo::new(move |_| {
        params.with(|p| MeasurementQuery {
            page: p.get("page").map_or(1, |v| v.parse().unwrap_or(0)),
            page_size: p.get("page_size").map_or(25, |v| v.parse().unwrap_or(0)),
            search: p.get("search").unwrap_or_default(),
            sort: p.get("sort").unwrap_or_else(|| "code".into()),
            direction: p.get("direction").unwrap_or_else(|| "asc".into()),
        })
    });
    let table_state = TableState {
        selected: RwSignal::new(BTreeSet::new()),
        all_results: RwSignal::new(false),
        copy_data: RwSignal::new(None),
        selection_notice: RwSignal::new(String::new()),
    };
    provide_context(table_state);
    let loading = RwSignal::new(false);
    let request_generation = RwSignal::new(0_u64);
    let page = Resource::new_blocking(
        move || (context.company.get().key().to_owned(), query.get()),
        move |(organization, query)| {
            let generation = request_generation.get_untracked().wrapping_add(1);
            request_generation.set(generation);
            loading.set(true);
            async move {
                let result = network::load_page(organization, query).await;
                if request_generation.try_get_untracked() == Some(generation) {
                    loading.try_set(false);
                }
                result
            }
        },
    );
    let was_loading = RwSignal::new(false);
    Effect::new(move |_| {
        if loading.get() {
            was_loading.set(true);
        } else if was_loading.get_untracked() {
            was_loading.set(false);
            request_animation_frame(move || focus("table-summary"));
        }
    });
    let initial = RwSignal::new(true);
    Effect::new(move |_| {
        let _ = query.get();
        let _ = context.company.get();
        if !initial.get_untracked() {
            table_state.clear();
        }
        initial.set(false);
    });

    view! {
        <section id="forms-shell" data-ready=move || context.ready.get().to_string()>
            <Heading title="Conferir a planilha de medição" description="Confira os acumulados de cada serviço e prepare seu rascunho com os valores do período."/>
            <p class="worksheet-intro">"Exemplo: passar de 20 para 35 m² representa 15 m² no período. A grade consulta uma base fictícia de 10.000 serviços; seus rascunhos ficam separados nesta aba."</p>
            <Toolbar query=query/>
            <p class="sr-only" role="status">{move || table_state.selection_notice.get()}</p>
            <Show when=move || context.with(|state| state.worksheet.editing.is_some())>
                <editor::MeasurementEditor/>
            </Show>
            <div id="table-content" aria-busy=move || loading.get().to_string()>
                <p class="notice neutral" role="status" hidden=move || !loading.get()>"Consultando serviços… Os rascunhos desta aba estão preservados."</p>
                <Suspense fallback=move || view! { <p id="table-loading" role="status" class="notice neutral">"Consultando serviços… Os rascunhos desta aba estão preservados."</p> }>
                    {move || page.get().map(|result| match result {
                        Ok(data) if data.organization == context.company.get().key() => view! {
                            <table::MeasurementTable page=data query=query/>
                        }.into_any(),
                        Ok(_) => view! { <p role="status">"Consultando a empresa selecionada…"</p> }.into_any(),
                        Err(message) => view! {
                            <section id="table-error" class="notice danger" role="alert">
                                <h2>"Não foi possível consultar a grade"</h2><p>{message}</p>
                                <button id="table-retry" class="primary-button" type="button" on:click=move |_| page.refetch()>"Tentar consulta novamente"</button>
                                <a class="text-link" href=move || context.company.get().url(Task::Worksheet)>"Restaurar consulta inicial"</a>
                            </section>
                        }.into_any(),
                    })}
                </Suspense>
            </div>
        </section>
    }
}

#[component]
fn Toolbar(query: Memo<MeasurementQuery>) -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let search = RwSignal::new(query.get_untracked().search);
    let sort = RwSignal::new(query.get_untracked().sort);
    let direction = RwSignal::new(query.get_untracked().direction);
    let page_size = RwSignal::new(query.get_untracked().page_size);
    Effect::new(move |_| {
        let query = query.get();
        search.set(query.search);
        sort.set(query.sort);
        direction.set(query.direction);
        page_size.set(query.page_size);
    });
    let navigate = use_navigate();
    view! {
        <form class="table-toolbar section-card" method="get" action=move || context.company.get().url(Task::Worksheet)
            on:submit=move |event| {
                event.prevent_default();
                let next = MeasurementQuery { page: 1, page_size: page_size.get_untracked(), search: search.get_untracked(), sort: sort.get_untracked(), direction: direction.get_untracked() };
                navigate(&format!("{}?{}", context.company.get_untracked().url(Task::Worksheet), network::query_string(&next)), NavigateOptions { scroll: false, ..Default::default() });
            }>
            <div class="field table-search-field"><label for="table-search">"Buscar código ou serviço"</label>
                <input id="table-search" name="search" type="search" maxlength="80" value=search.get_untracked() prop:value=move || search.get()
                    on:input=move |event| search.set(event_target_value(&event))/>
            </div>
            <div class="field"><label for="table-sort">"Ordenar por"</label>
                <select id="table-sort" name="sort" prop:value=move || sort.get() on:change=move |event| sort.set(event_target_value(&event))>
                    <option value="code" selected=move || sort.get() == "code">"Código"</option>
                    <option value="description" selected=move || sort.get() == "description">"Serviço"</option>
                    <option value="current" selected=move || sort.get() == "current">"Acumulado atual"</option>
                    <option value="period_value" selected=move || sort.get() == "period_value">"Valor do período"</option>
                </select>
            </div>
            <div class="field"><label for="table-direction">"Ordem"</label>
                <select id="table-direction" name="direction" prop:value=move || direction.get() on:change=move |event| direction.set(event_target_value(&event))>
                    <option value="asc" selected=move || direction.get() == "asc">"Crescente"</option>
                    <option value="desc" selected=move || direction.get() == "desc">"Decrescente"</option>
                </select>
            </div>
            <div class="field"><label for="table-page-size">"Serviços por página"</label>
                <select id="table-page-size" name="page_size" prop:value=move || page_size.get().to_string() on:change=move |event| page_size.set(event_target_value(&event).parse().unwrap_or(25))>
                    <option value="25" selected=move || page_size.get() == 25>"25"</option>
                    <option value="50" selected=move || page_size.get() == 50>"50"</option>
                    <option value="100" selected=move || page_size.get() == 100>"100"</option>
                </select>
            </div>
            <button id="table-submit" class="primary-button" type="submit">"Consultar serviços"</button>
        </form>
    }
}

fn page_link(context: DemoContext, query: &MeasurementQuery, page: u32) -> String {
    let mut next = query.clone();
    next.page = page;
    format!(
        "{}?{}#table-summary",
        context.company.get_untracked().url(Task::Worksheet),
        network::query_string(&next)
    )
}

fn selected_copy(page: &MeasurementPage, selected: &BTreeSet<u32>) -> String {
    let mut lines = vec!["Código\tServiço\tUnidade\tContratado\tAnterior acumulado\tAtual acumulado\tDiferença no período\tPreço unitário (R$)\tValor do período (R$)\tSaldo (quantidade)".to_string()];
    for row in page.rows.iter().filter(|row| selected.contains(&row.id)) {
        let numbers = [
            &row.contracted,
            &row.previous,
            &row.current,
            &row.difference,
            &row.unit_price,
            &row.period_value,
            &row.balance,
        ]
        .map(|value| canteiro_contracts::forms::format_decimal_pt_br(value));
        lines.push(format!(
            "{}\t{}\t{}\t{}",
            row.code,
            row.description,
            row.unit,
            numbers.join("\t")
        ));
    }
    lines.join("\n")
}
