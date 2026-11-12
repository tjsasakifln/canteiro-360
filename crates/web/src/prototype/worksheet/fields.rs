use super::editor::EditContext;
use canteiro_contracts::forms::format_decimal_pt_br;
use leptos::prelude::*;

pub fn input_id(field: &str) -> &'static str {
    match field {
        "current" => "form-current",
        "unit_price" => "form-money",
        "percent" => "form-percent",
        "date" => "form-date",
        "unit" => "form-unit",
        "notes" => "form-notes",
        _ => "form-errors",
    }
}

pub fn field_label(field: &str) -> &'static str {
    match field {
        "current" => "Acumulado atual",
        "unit_price" => "Preço unitário",
        "percent" => "Percentual demonstrativo",
        "date" => "Data da medição",
        "unit" => "Unidade contratada",
        "notes" => "Observações",
        _ => "Rascunho",
    }
}

/// A labelled, unmasked text control. It keeps the exact text while presenting
/// its interpretation separately, rather than rewriting ambiguous input.
#[component]
pub fn ValidatedField(
    field: &'static str,
    suffix: &'static str,
    label: &'static str,
    hint: &'static str,
    #[prop(default = "decimal")] input_mode: &'static str,
    #[prop(default = 64)] max_length: u32,
) -> impl IntoView {
    let edit = expect_context::<EditContext>();
    let id = input_id(field);
    let error = Memo::new(move |_| {
        let draft = edit.draft();
        let visible = draft
            .as_ref()
            .is_some_and(|draft| draft.submitted || draft.touched.contains(field));
        visible
            .then(|| {
                edit.validation
                    .get()
                    .and_then(|result| result.errors.into_iter().find(|error| error.field == field))
            })
            .flatten()
    });
    let canonical = Memo::new(move |_| {
        edit.validation
            .get()
            .and_then(|result| result.canonical.get(field).cloned())
    });
    let interpretation = Signal::derive(move || {
        canonical
            .get()
            .map(|value| {
                let display = if field == "date" && value.len() == 10 {
                    format!("{}/{}/{}", &value[8..10], &value[5..7], &value[..4])
                } else if field == "unit" {
                    value
                } else {
                    format_decimal_pt_br(&value)
                };
                match field {
                    "unit_price" => format!("Interpretado: R$ {display} por unidade."),
                    "percent" => format!("Interpretado: {display}%."),
                    _ => format!("Interpretado: {display}."),
                }
            })
            .unwrap_or_else(|| "A interpretação aparecerá após uma entrada válida.".into())
    });
    view! {
        <crate::form_components::LabeledTextInput id=id name=field label=label hint=hint
            message_prefix=format!("field-{suffix}") input_mode=input_mode max_length=max_length
            value=Signal::derive(move || edit.draft().map(|draft| draft.value(field)).unwrap_or_default())
            error=Signal::derive(move || error.get().map(|error| error.message))
            interpretation=interpretation canonical=canonical.into()
            on_input=Callback::new(move |value| edit.change(field, value))
            on_blur=Callback::new(move |_| edit.touch(field))/>
    }
}

pub fn focus_field(field: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        // A correction target inside progressive disclosure must be visible.
        if matches!(field, "percent" | "notes")
            && let Some(details) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("form-advanced"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlDetailsElement>().ok())
        {
            details.set_open(true);
        }
    }
    super::super::components::focus(input_id(field));
}

pub fn focus_row_or_summary(id: u32) {
    let row = format!("edit-row-{id}");
    #[cfg(feature = "hydrate")]
    let exists = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id(&row))
        .is_some();
    #[cfg(not(feature = "hydrate"))]
    let exists = false;
    super::super::components::focus(if exists { &row } else { "table-summary" });
}

/// Finish keyboard navigation without interrupting work started while waiting.
pub fn focus_query_summary() {
    #[cfg(feature = "hydrate")]
    {
        let active = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element());
        let can_move = active.is_none_or(|element| {
            element.tag_name() == "BODY"
                || matches!(
                    element.id().as_str(),
                    "table-submit" | "table-prev" | "table-next" | "table-retry"
                )
        });
        if can_move {
            super::super::components::focus("table-summary");
        }
    }
}
