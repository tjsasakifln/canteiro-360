use leptos::prelude::*;

#[component]
pub fn Badge(tone: &'static str, children: Children) -> impl IntoView {
    view! { <span class=format!("status-badge {tone}")>{children()}</span> }
}

#[component]
pub fn Heading(title: &'static str, description: &'static str) -> impl IntoView {
    view! {
        <div class="page-heading">
            <div><p class="eyebrow">"Rotina da obra"</p><h1 id="task-title">{title}</h1><p>{description}</p></div>
        </div>
    }
}

#[component]
pub fn Metric(label: &'static str, value: String, help: &'static str) -> impl IntoView {
    view! { <div class="metric-card"><h3>{label}</h3><p class="metric-value">{value}</p><p>{help}</p></div> }
}

#[component]
pub fn Notice(tone: &'static str, title: &'static str, children: Children) -> impl IntoView {
    view! { <div class=format!("notice {tone}")><strong>{title}</strong><div>{children()}</div></div> }
}

/// DOM methods are presentation interop, never authorization or persistence.
pub fn focus(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(element) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(id))
            .and_then(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = element.focus();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}

pub fn show_dialog(id: &str, first: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(dialog) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(id))
            .and_then(|element| element.dyn_into::<web_sys::HtmlDialogElement>().ok())
            && dialog.show_modal().is_ok()
        {
            focus(first);
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (id, first);
}

pub fn close_dialog(id: &str) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(dialog) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.get_element_by_id(id))
            .and_then(|element| element.dyn_into::<web_sys::HtmlDialogElement>().ok())
        {
            dialog.close();
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
}
