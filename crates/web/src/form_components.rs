//! Presentation primitives independent of any domain, store or endpoint.
use leptos::prelude::*;

#[component]
pub fn LabeledTextInput(
    id: &'static str,
    name: &'static str,
    label: &'static str,
    hint: &'static str,
    message_prefix: String,
    value: Signal<String>,
    error: Signal<Option<String>>,
    interpretation: Signal<String>,
    canonical: Signal<Option<String>>,
    on_input: Callback<String>,
    on_blur: Callback<()>,
    #[prop(default = "text")] input_mode: &'static str,
    #[prop(default = 128)] max_length: u32,
    #[prop(default = true)] required: bool,
) -> impl IntoView {
    let help_id = format!("{message_prefix}-help");
    let interpretation_id = format!("{message_prefix}-interpretation");
    let error_id = format!("{message_prefix}-error");
    let descriptions = format!("{help_id} {interpretation_id} {error_id}");
    view! {
        <div class="field validated-field">
            <label for=id>{label}<span class="field-required">{if required { "Obrigatório" } else { "Opcional" }}</span></label>
            <input id=id name=name type="text" inputmode=input_mode autocomplete="off" maxlength=max_length
                value=value.get_untracked() prop:value=move || value.get()
                aria-required=required.to_string() aria-invalid=move || error.get().is_some().to_string()
                aria-describedby=descriptions
                on:input=move |event| on_input.run(event_target_value(&event))
                on:blur=move |_| on_blur.run(())/>
            <p id=help_id class="field-hint">{hint}</p>
            <p id=interpretation_id class="field-interpretation" data-canonical=move || canonical.get().unwrap_or_default()>{move || interpretation.get()}</p>
            <p id=error_id class="field-error" hidden=move || error.get().is_none()>{move || error.get().unwrap_or_default()}</p>
        </div>
    }
}

/// Display values arrive already formatted by their domain-specific adapter.
#[derive(Clone, Debug)]
pub struct ComparedValue {
    pub label: String,
    pub value: String,
    pub id: Option<String>,
}

#[component]
pub fn ValueComparison(
    id: &'static str,
    title_id: &'static str,
    title: &'static str,
    description: &'static str,
    note: &'static str,
    values: Vec<ComparedValue>,
) -> impl IntoView {
    view! {
        <section id=id class="before-after" tabindex="-1" aria-labelledby=title_id>
            <h3 id=title_id>{title}</h3><p>{description}</p>
            <dl class="comparison-grid">{values.into_iter().map(|entry| view! {
                <div><dt>{entry.label}</dt><dd id=entry.id>{entry.value}</dd></div>
            }).collect_view()}</dl>
            <p>{note}</p>
        </section>
    }
}

#[component]
pub fn PaginationControls(
    page: u32,
    pages: u32,
    previous: Signal<String>,
    next: Signal<String>,
) -> impl IntoView {
    view! {
        <nav class="table-pagination" aria-label="Páginas de serviços">
            <a id="table-prev" class="secondary-button" href=move || previous.get() aria-disabled=(page <= 1).to_string() tabindex={if page <= 1 { -1 } else { 0 }}
                on:click=move |event| { if page <= 1 { event.prevent_default(); } }>"Página anterior"</a>
            <span>{format!("Página {page} de {pages}")}</span>
            <a id="table-next" class="secondary-button" href=move || next.get() aria-disabled=(page >= pages).to_string() tabindex={if page >= pages { -1 } else { 0 }}
                on:click=move |event| { if page >= pages { event.prevent_default(); } }>"Próxima página"</a>
        </nav>
    }
}
