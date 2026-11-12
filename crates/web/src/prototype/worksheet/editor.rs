use super::super::{DemoContext, components::*};
use super::{fields::*, network, state::RowDraft};
use canteiro_contracts::forms::{
    FormPreview, MeasurementRow, format_decimal_pt_br, validate_measurement_form,
};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(super) struct EditContext {
    pub context: DemoContext,
    pub row_id: Memo<Option<u32>>,
    pub validation: Memo<Option<FormPreview>>,
}

impl EditContext {
    pub fn draft(self) -> Option<RowDraft> {
        let id = self.row_id.get()?;
        self.context
            .with(|state| state.worksheet.drafts.get(&id).cloned())
    }
    pub fn update(self, change: impl FnOnce(&mut RowDraft)) {
        if let Some(id) = self.row_id.get_untracked() {
            self.context.update(|state| {
                if let Some(draft) = state.worksheet.drafts.get_mut(&id) {
                    change(draft);
                }
            });
        }
    }
    pub fn change(self, field: &str, value: String) {
        self.update(|draft| draft.change(field, value));
    }
    pub fn touch(self, field: &str) {
        self.update(|draft| {
            draft.touched.insert(field.to_owned());
        });
    }
}

#[component]
pub(super) fn MeasurementEditor() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let row_id = Memo::new(move |_| context.with(|state| state.worksheet.editing));
    let validation = Memo::new(move |_| {
        context.with(|state| {
            let draft = state.worksheet.drafts.get(&state.worksheet.editing?)?;
            validate_measurement_form(state.company.key(), &draft.input).ok()
        })
    });
    let edit = EditContext {
        context,
        row_id,
        validation,
    };
    provide_context(edit);
    let pending = RwSignal::new(false);
    let server_error = RwSignal::new(None::<String>);
    let errors = Memo::new(move |_| {
        if !edit.draft().is_some_and(|draft| draft.submitted) {
            return Vec::new();
        }
        let local = validation
            .get()
            .map(|result| result.errors)
            .unwrap_or_default();
        if !local.is_empty() {
            return local;
        }
        edit.draft()
            .and_then(|draft| draft.checked)
            .map(|result| result.errors)
            .unwrap_or_default()
    });
    let needs_note = Memo::new(move |_| {
        edit.draft().is_some_and(|draft| {
            validation
                .get()
                .and_then(|result| result.canonical.get("unit_price").cloned())
                .is_some_and(|price| price != draft.original.unit_price)
        })
    });
    let submit = move |event: leptos::ev::SubmitEvent| {
        event.prevent_default();
        if pending.get_untracked() {
            return;
        }
        edit.update(|draft| draft.submitted = true);
        server_error.set(None);
        if let Some(first) = validation
            .get_untracked()
            .and_then(|result| result.errors.first().cloned())
        {
            focus_field(&first.field);
            return;
        }
        let Some(draft) = edit.draft() else {
            return;
        };
        let input = draft.input;
        let organization = context.company.get_untracked();
        let store = context.store.state;
        pending.set(true);
        leptos::task::spawn_local(async move {
            let response = network::validate_form(organization.key(), &input).await;
            let still_current = store.try_get_untracked().is_some_and(|state| {
                state.company == organization
                    && state.worksheet.editing == Some(input.row_id)
                    && state
                        .worksheet
                        .drafts
                        .get(&input.row_id)
                        .is_some_and(|draft| draft.input == input)
            });
            if still_current {
                match response {
                    Ok(result) => {
                        let first_error = result.errors.first().map(|error| error.field.clone());
                        store.try_update(|state| {
                            if let Some(draft) = state.worksheet.drafts.get_mut(&input.row_id) {
                                draft.checked = Some(result);
                                draft.checked_input = Some(input);
                            }
                        });
                        if let Some(field) = first_error {
                            focus_field(&field);
                        } else {
                            request_animation_frame(move || focus("form-comparison"));
                        }
                    }
                    Err(message) => {
                        server_error.try_set(Some(message));
                    }
                }
            }
            pending.try_set(false);
        });
    };

    view! {
        <section class="section-card measurement-editor" aria-labelledby="form-title">
            <div class="editor-heading"><div><p class="eyebrow">"Edição explícita de um serviço"</p>
                <h2 id="form-title" tabindex="-1">{move || edit.draft().map(|draft| format!("{} · {}", draft.original.code, draft.original.description)).unwrap_or_default()}</h2>
                <p>{move || edit.draft().map(|draft| format!("Contratado: {} {}. Acumulado anterior: {} {}.", format_decimal_pt_br(&draft.original.contracted), draft.original.unit, format_decimal_pt_br(&draft.original.previous), draft.original.unit)).unwrap_or_default()}</p>
            </div><button id="form-cancel" class="secondary-button" type="button" on:click=move |_| {
                let id = row_id.get_untracked().unwrap_or(0);
                context.update(|state| state.worksheet.editing = None);
                request_animation_frame(move || focus_row_or_summary(id));
            }>"Voltar à grade e manter rascunho"</button></div>
            <DraftStatus/>
            <form id="measurement-form" novalidate on:submit=submit>
                <Show when=move || !errors.get().is_empty()>
                    <div id="form-errors" class="notice danger" role="alert" tabindex="-1">
                        <h3>"Revise os campos indicados"</h3><p>"Os demais valores e textos foram preservados."</p>
                        <ul>{move || errors.get().into_iter().map(|error| {
                            let field = error.field.clone();
                            view! { <li><a href=format!("#{}", input_id(&error.field)) on:click=move |event| { event.prevent_default(); focus_field(&field); }>{format!("{}: {}", field_label(&error.field), error.message)}</a></li> }
                        }).collect_view()}</ul>
                    </div>
                </Show>
                <div class="form-grid">
                    <ValidatedField field="current" suffix="current" label="Acumulado atual (na unidade contratada)" hint="Informe o total executado até esta medição, incluindo o que já foi medido. Use vírgula para decimais."/>
                    <ValidatedField field="unit" suffix="unit" label="Unidade contratada" hint="A unidade precisa coincidir com a do serviço. Exemplos: m², m³, m, kg ou un." input_mode="text" max_length=12/>
                    <ValidatedField field="unit_price" suffix="money" label="Preço unitário demonstrativo (R$)" hint="Use 1.234,56 ou 1234,56. Até seis casas decimais. A prévia não altera o preço do contrato."/>
                    <ValidatedField field="date" suffix="date" label="Data da medição" hint="Dia, mês e ano. Exemplo: 12/11/2026." input_mode="text" max_length=10/>
                </div>
                <details id="form-advanced" class="form-advanced"><summary id="form-advanced-toggle">"Percentual e observações"</summary>
                    <p>"O percentual serve para conferir o preenchimento; não altera o valor desta prévia."</p>
                    <ValidatedField field="percent" suffix="percent" label="Percentual demonstrativo (%)" hint="De 0 a 100. Use vírgula quando houver casas decimais."/>
                    <div class="field"><label for="form-notes">{move || if needs_note.get() { "Observações (obrigatórias para preço diferente)" } else { "Observações (opcional)" }}</label>
                        <p id="field-notes-help">{move || if needs_note.get() { "Explique o preço usado nesta simulação. O preço do contrato não será alterado." } else { "Registre o contexto desta conferência, se necessário." }}</p>
                        <textarea id="form-notes" name="notes" rows="3" maxlength="1000" aria-describedby="field-notes-help field-notes-error" aria-required=move || needs_note.get().to_string()
                            aria-invalid=move || errors.get().iter().any(|error| error.field == "notes").to_string()
                            prop:value=move || edit.draft().map(|draft| draft.input.notes).unwrap_or_default()
                            on:input=move |event| edit.change("notes", event_target_value(&event)) on:blur=move |_| edit.touch("notes")/>
                        <p id="field-notes-error" class="field-error">{move || errors.get().into_iter().find(|error| error.field == "notes").map(|error| error.message).unwrap_or_default()}</p>
                    </div>
                </details>
                <Show when=move || needs_note.get()><p class="notice warning">"Este preço difere da referência. Preencha as observações em Percentual e observações para justificar a simulação."</p></Show>
                <Show when=move || !edit.draft().is_some_and(|draft| draft.checked_for_current_input())>
                    {move || validation.get().and_then(|result| result.preview).map(|preview| view! {
                        <aside id="form-local-preview" class="notice neutral" aria-label="Prévia local da edição">
                            <strong>"Prévia enquanto você edita"</strong>
                            <p>{format!("{} para {} {}: {} {} no período; valor de R$ {}; saldo de {} {}.", format_decimal_pt_br(&preview.previous), format_decimal_pt_br(&preview.current), preview.unit, format_decimal_pt_br(&preview.difference), preview.unit, format_decimal_pt_br(&preview.period_value), format_decimal_pt_br(&preview.balance), preview.unit)}</p>
                            <p>"Aguarda conferência no servidor. Nada foi gravado."</p>
                        </aside>
                    })}
                </Show>
                <Show when=move || server_error.get().is_some()><p id="form-server-error" class="notice danger" role="alert">{move || server_error.get().unwrap_or_default()}</p></Show>
                <div class="form-actions">
                    <button id="form-preview" class="primary-button" type="submit" disabled=move || pending.get()>{move || if pending.get() { "Conferindo no servidor…" } else { "Conferir valores e diferenças" }}</button>
                    <button id="form-discard" class="secondary-button" type="button" on:click=move |_| show_dialog("draft-discard-dialog", "cancel-draft-discard")>"Descartar este rascunho"</button>
                    <p role="status">{move || if pending.get() { "Aguardando conferência. Nenhum registro foi gravado." } else { "" }}</p>
                </div>
            </form>
            <Show when=move || edit.draft().is_some_and(|draft| draft.checked_for_current_input())>
                {move || edit.draft().and_then(|draft| draft.checked.and_then(|result| result.preview).map(|preview| view! { <BeforeAfter before=draft.original after=preview/> }))}
                <button id="form-save" class="primary-button" type="button" disabled=move || pending.get() || !edit.draft().is_some_and(|draft| draft.checked_for_current_input()) on:click=move |_| {
                    edit.update(|draft| {
                        if draft.checked_for_current_input() {
                            draft.saved_input = Some(draft.input.clone());
                            draft.saved_at = Some(network::saved_time());
                            draft.revision = draft.revision.saturating_add(1);
                        }
                    });
                    focus("draft-status");
                }>"Guardar rascunho nesta aba"</button>
            </Show>
            <dialog id="draft-discard-dialog" class="prototype-dialog" aria-labelledby="discard-title"
                on:keydown=move |event| trap_dialog_tab(event, "draft-discard-dialog", ["cancel-draft-discard", "confirm-draft-discard"])
                on:close=move |_| focus("form-discard")>
                <h2 id="discard-title">"Descartar este rascunho?"</h2>
                <p>"As entradas e a revisão local deste serviço serão removidas. Os outros rascunhos serão mantidos."</p>
                <div class="dialog-actions">
                    <button id="cancel-draft-discard" class="secondary-button" type="button" on:click=move |_| close_dialog("draft-discard-dialog")>"Continuar editando"</button>
                    <button id="confirm-draft-discard" class="primary-button" type="button" on:click=move |_| {
                        let id = row_id.get_untracked().unwrap_or(0);
                        close_dialog("draft-discard-dialog");
                        context.update(|state| { state.worksheet.drafts.remove(&id); state.worksheet.editing = None; });
                        request_animation_frame(move || focus_row_or_summary(id));
                    }>"Descartar rascunho"</button>
                </div>
            </dialog>
        </section>
    }
}

#[component]
fn DraftStatus() -> impl IntoView {
    let edit = expect_context::<EditContext>();
    view! {
        <div id="draft-status" class="notice neutral draft-status" tabindex="-1" role="status" data-state=move || if edit.draft().is_some_and(|draft| draft.is_saved()) { "saved" } else { "edited" }>
            <strong>{move || if edit.draft().is_some_and(|draft| draft.is_saved()) { "Rascunho guardado nesta aba" } else { "Rascunho em edição nesta aba" }}</strong>
            <p>{move || edit.draft().and_then(|draft| draft.saved_at.map(|time| format!("Autor demonstrativo: Pessoa da demonstração. Revisão local {}. Última cópia: {} (UTC).", draft.revision, time))).unwrap_or_else(|| "Autor demonstrativo: Pessoa da demonstração. Ainda sem cópia guardada.".into())}</p>
            <p>"A navegação nesta empresa preserva as entradas. Recarregar ou fechar a aba perde este rascunho. A grade de referência e o contrato não foram alterados."</p>
        </div>
    }
}

/// Common comparison layout; each label names the unit and effect explicitly.
#[component]
fn BeforeAfter(before: MeasurementRow, after: MeasurementRow) -> impl IntoView {
    use crate::form_components::{ComparedValue, ValueComparison};
    let fmt = format_decimal_pt_br;
    let values = [
        (
            "Acumulado anterior já medido",
            format!("{} {}", fmt(&after.previous), after.unit),
            None,
        ),
        (
            "Atual antes desta edição",
            format!("{} {}", fmt(&before.current), before.unit),
            None,
        ),
        (
            "Atual proposto",
            format!("{} {}", fmt(&after.current), after.unit),
            None,
        ),
        (
            "Diferença no período",
            format!("{} {}", fmt(&after.difference), after.unit),
            Some("preview-difference"),
        ),
        (
            "Valor do período antes",
            format!("R$ {}", fmt(&before.period_value)),
            None,
        ),
        (
            "Valor do período proposto",
            format!("R$ {}", fmt(&after.period_value)),
            Some("preview-value"),
        ),
        (
            "Saldo de quantidade após a prévia",
            format!("{} {}", fmt(&after.balance), after.unit),
            None,
        ),
        (
            "Saldo financeiro demonstrativo",
            format!("R$ {}", fmt(&after.balance_value)),
            None,
        ),
    ]
    .into_iter()
    .map(|(label, value, id)| ComparedValue {
        label: label.into(),
        value,
        id: id.map(str::to_owned),
    })
    .collect();
    view! {
        <ValueComparison id="form-comparison" title_id="comparison-title" title="Conferência do rascunho"
            description="Prévia conferida no servidor. Nenhuma medição foi aprovada ou gravada."
            note="O valor do período é a diferença entre os valores acumulados, calculados em centavos. O preço informado vale somente para esta simulação."
            values=values/>
    }
}
