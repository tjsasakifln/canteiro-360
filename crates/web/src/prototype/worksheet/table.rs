use super::super::{DemoContext, components::focus};
use super::{TableState, network, page_link, selected_copy};
use canteiro_contracts::forms::{
    MeasurementPage, MeasurementQuery, MeasurementRow, format_decimal_pt_br,
};
use leptos::prelude::*;

#[component]
pub(super) fn MeasurementTable(
    page: MeasurementPage,
    query: Memo<MeasurementQuery>,
) -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let selection = expect_context::<TableState>();
    let page = StoredValue::new(page);
    let total = page.with_value(|page| page.total);
    let row_ids = StoredValue::new(
        page.with_value(|page| page.rows.iter().map(|row| row.id).collect::<Vec<_>>()),
    );
    let selected_count = move || {
        if selection.all_results.get() {
            total as usize
        } else {
            selection.selected.with(|ids| ids.len())
        }
    };
    let checked_page = move || {
        selection.all_results.get()
            || row_ids.with_value(|ids| {
                !ids.is_empty()
                    && ids
                        .iter()
                        .all(|id| selection.selected.with(|chosen| chosen.contains(id)))
            })
    };
    view! {
        <section class="measurement-table-section" aria-labelledby="table-summary" data-company=page.with_value(|page| page.organization.clone())>
            <div class="table-summary-row"><h2 id="table-summary" tabindex="-1" role="status" aria-live="polite">{page.with_value(|page| if page.total == 0 {
                "Nenhum serviço encontrado".to_string()
            } else {
                format!("Serviços {} a {} de {} · Página {} de {}", (page.page - 1) * page.page_size + 1, (page.page - 1) * page.page_size + page.rows.len() as u32, page.total, page.page, page.total_pages)
            })}</h2><span class="status-badge neutral">"Base de referência"</span></div>
            <p class="field-hint">"Os valores da grade são a referência fictícia. Abrir uma edição não altera a base. Quantidades usam a unidade identificada em cada serviço."</p>
            <Show when=move || { total > 0 } fallback=move || view! { <p class="empty-state">"Revise a busca ou remova o filtro. Seus rascunhos continuam nesta aba."</p> }>
                <div class="selection-bar">
                    <label class="checkbox-label" for="select-page"><input id="select-page" type="checkbox" disabled=move || !context.ready.get() || selection.all_results.get() prop:checked=checked_page
                        on:change=move |event| {
                            let checked = event_target_checked(&event);
                            selection.selected.update(|chosen| { for id in row_ids.get_value() { if checked { chosen.insert(id); } else { chosen.remove(&id); } } });
                            selection.copy_data.set(None);
                        }/>"Selecionar somente esta página"</label>
                    <button id="select-all-results" class="secondary-button" type="button" disabled=move || !context.ready.get() || selection.all_results.get() on:click=move |_| { selection.all_results.set(true); selection.copy_data.set(None); }>{format!("Selecionar os {total} resultados da busca")}</button>
                    <button id="clear-selection" class="text-button" type="button" disabled=move || !context.ready.get() on:click=move |_| { selection.selected.update(|chosen| chosen.clear()); selection.all_results.set(false); selection.copy_data.set(None); }>"Limpar seleção"</button>
                    <p id="selection-status" role="status" aria-live="polite">{move || if selection.all_results.get() { format!("Todos os {total} resultados desta busca selecionados, incluindo outras páginas.") } else { format!("{} serviço(s) selecionado(s) somente nesta página. Nenhum resultado de outra página está selecionado.", selected_count()) }}</p>
                    <button id="copy-selection" class="secondary-button" type="button" disabled=move || !context.ready.get() || selected_count() == 0 on:click=move |_| {
                        if selection.all_results.get_untracked() {
                            selection.copy_data.set(None);
                            focus("export-selection");
                        } else {
                            selection.copy_data.set(Some(page.with_value(|data| selected_copy(data, &selection.selected.get_untracked()))));
                            request_animation_frame(move || {
                                focus("copy-data");
                                #[cfg(feature = "hydrate")]
                                {
                                    use wasm_bindgen::JsCast;
                                    if let Some(textarea) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.get_element_by_id("copy-data")).and_then(|e| e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) { textarea.select(); }
                                }
                            });
                        }
                    }>{move || if selection.all_results.get() { "Ir à exportação da seleção" } else { "Preparar dados para copiar" }}</button>
                    <Show when=move || selection.all_results.get()>
                        <a id="export-selection" class="primary-button" href=move || format!("/api/prototipo/{}/medicoes/export?{}", context.company.get().key(), network::query_string(&query.get())) download="medicao-referencia.tsv">{format!("Baixar os {total} resultados selecionados (.tsv)")}</a>
                        <p>"O arquivo contém os valores da base de referência correspondentes a esta busca. Rascunhos locais não entram na exportação."</p>
                    </Show>
                </div>
                <Show when=move || selection.copy_data.get().is_some()>
                    <div class="copy-panel"><label for="copy-data">"Dados selecionados nesta página, prontos para copiar"</label>
                        <p>"Use Ctrl+C ou a função Copiar do seu dispositivo. Os dados são da base de referência, sem alterações dos rascunhos."</p>
                        <textarea id="copy-data" readonly rows="5" prop:value=move || selection.copy_data.get().unwrap_or_default()/>
                    </div>
                </Show>
                <div class="measurement-scroll" role="region" aria-label="Grade de medição com rolagem horizontal, use as setas para consultar todas as colunas" tabindex="0">
                    <table id="measurement-grid" class="data-table measurement-grid">
                        <caption class="sr-only">"Serviços da medição. Anterior e atual são acumulados; diferença representa a quantidade do período. Cada quantidade usa a unidade da linha."</caption>
                        <colgroup><col class="selection-column"/><col class="service-column"/><col span="8"/></colgroup>
                        <thead><tr>
                            <th scope="col">"Selecionar"</th><th scope="col" class="service-heading">"Serviço, unidade e edição"</th>
                            <th scope="col" class="number">"Contratado (unid. da linha)"</th>
                            <th scope="col" class="number">"Anterior acumulado (unid. da linha)"</th>
                            <th scope="col" class="number">"Atual acumulado (unid. da linha)"</th>
                            <th scope="col" class="number">"Diferença no período (unid. da linha)"</th>
                            <th scope="col" class="number">"Preço unitário (R$)"</th>
                            <th scope="col" class="number">"Valor do período (R$)"</th>
                            <th scope="col" class="number">"Saldo (unid. da linha)"</th>
                            <th scope="col" class="number">"Saldo (R$)"</th>
                        </tr></thead>
                        <tbody>{page.with_value(|page| page.rows.iter().cloned().map(|row| view! { <MeasurementLine row=row/> }).collect_view())}</tbody>
                    </table>
                </div>
                <crate::form_components::PaginationControls
                    page=page.with_value(|page| page.page) pages=page.with_value(|page| page.total_pages)
                    previous=Signal::derive(move || page_link(context, &query.get(), page.with_value(|page| page.page.saturating_sub(1).max(1))))
                    next=Signal::derive(move || page_link(context, &query.get(), page.with_value(|page| (page.page + 1).min(page.total_pages))))/>

            </Show>
        </section>
    }
}

#[component]
fn MeasurementLine(row: MeasurementRow) -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let selection = expect_context::<TableState>();
    let id = row.id;
    let row = StoredValue::new(row);
    let fmt = format_decimal_pt_br;
    view! {
        <tr data-row-id=id data-selected=move || (selection.all_results.get() || selection.selected.with(|ids| ids.contains(&id))).to_string()>
            <td><label class="row-selection-target" for=format!("select-row-{id}")>
                <span class="sr-only">{row.with_value(|row| format!("Selecionar serviço {}", row.code))}</span>
                <input id=format!("select-row-{id}") type="checkbox" disabled=move || !context.ready.get() || selection.all_results.get()
                prop:checked=move || selection.all_results.get() || selection.selected.with(|ids| ids.contains(&id)) on:change=move |event| {
                    let checked = event_target_checked(&event);
                    selection.selected.update(|ids| { if checked { ids.insert(id); } else { ids.remove(&id); } }); selection.copy_data.set(None);
                }/></label></td>
            <th scope="row" class="service-cell"><strong>{row.with_value(|row| row.code.clone())}</strong><span>{row.with_value(|row| row.description.clone())}</span><span class="unit-label">{row.with_value(|row| format!("Unidade: {}", row.unit))}</span>
                <Show when=move || context.with(|state| state.worksheet.drafts.get(&id).is_some_and(|draft| draft.has_edits()))><span class="status-badge warning">"Rascunho nesta aba"</span></Show>
                <button id=format!("edit-row-{id}") class="secondary-button" type="button" disabled=move || !context.ready.get() aria-label=row.with_value(|row| format!("Editar rascunho do serviço {}", row.code)) on:click=move |_| {
                    context.update(|state| state.worksheet.open(row.get_value()));
                    request_animation_frame(move || focus("form-title"));
                }>"Editar rascunho"</button>
            </th>
            <td class="number" data-col="contracted">{row.with_value(|row| fmt(&row.contracted))}</td>
            <td class="number" data-col="previous">{row.with_value(|row| fmt(&row.previous))}</td>
            <td class="number" data-col="current">{row.with_value(|row| fmt(&row.current))}</td>
            <td class="number" data-col="difference">{row.with_value(|row| fmt(&row.difference))}</td>
            <td class="number" data-col="unit-price">{row.with_value(|row| fmt(&row.unit_price))}</td>
            <td class="number" data-col="period-value">{row.with_value(|row| fmt(&row.period_value))}</td>
            <td class="number" data-col="balance">{row.with_value(|row| fmt(&row.balance))}</td>
            <td class="number" data-col="balance-value">{row.with_value(|row| fmt(&row.balance_value))}</td>
        </tr>
    }
}
