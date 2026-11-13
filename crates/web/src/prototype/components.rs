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

/// Keep the two actual dialog actions in a circular Tab sequence. Native
/// modal dialogs make the background inert, but Chromium can still send Tab
/// from their last control to browser chrome (reported as BODY). Each caller
/// lists its actions in DOM order; disabled or hidden actions are excluded.
/// Escape and browser shortcuts remain native. No business state is changed.
pub fn trap_dialog_tab(event: leptos::ev::KeyboardEvent, dialog_id: &str, action_ids: [&str; 2]) {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;

        if event.key() != "Tab"
            || event.ctrl_key()
            || event.alt_key()
            || event.meta_key()
            || event.default_prevented()
        {
            return;
        }
        let Some(document) = web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let Some(dialog) = document
            .get_element_by_id(dialog_id)
            .and_then(|element| element.dyn_into::<web_sys::HtmlDialogElement>().ok())
            .filter(|dialog| dialog.open())
        else {
            return;
        };
        let actions = action_ids
            .into_iter()
            .filter_map(|id| document.get_element_by_id(id))
            .filter_map(|element| element.dyn_into::<web_sys::HtmlElement>().ok())
            .filter(|element| {
                dialog.contains(Some(element.as_ref()))
                    && element.tab_index() >= 0
                    && !element.matches(":disabled, [hidden]").unwrap_or(true)
            })
            .collect::<Vec<_>>();
        event.prevent_default();
        if actions.is_empty() {
            let _ = dialog.focus();
            return;
        }
        let focused = document.active_element();
        let current = actions.iter().position(|element| {
            focused
                .as_ref()
                .is_some_and(|active| active.is_same_node(Some(element.as_ref())))
        });
        let next = if event.shift_key() {
            current
                .and_then(|index| index.checked_sub(1))
                .unwrap_or(actions.len() - 1)
        } else {
            current.map_or(0, |index| (index + 1) % actions.len())
        };
        let _ = actions[next].focus();
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = (event, dialog_id, action_ids);
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
