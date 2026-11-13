use any_spawner::Executor;
use leptos_server::Resource;
use reactive_graph::{
    computed::Memo,
    effect::{Effect, RenderEffect},
    owner::{Owner, on_cleanup},
    signal::RwSignal,
    traits::*,
};
use tachys::{
    reactive_graph::OwnedView,
    renderer::types::{Element, Node},
    view::{Mountable, Render},
};
use std::any::Any;

struct WorksheetView {
    _effects: Vec<RenderEffect<bool>>,
    _page: RenderEffect<Vec<RenderEffect<bool>>>,
}

struct TaskView(Option<Box<dyn Any>>);
impl Render for TaskView {
    type State = Self;
    fn build(self) -> Self { self }
    fn rebuild(self, state: &mut Self) { *state = self; }
}
impl Mountable for TaskView {
    fn unmount(&mut self) {}
    fn mount(&mut self, _: &Element, _: Option<&Node>) {}
    fn insert_before_this(&self, _: &mut dyn Mountable) -> bool { false }
    fn elements(&self) -> Vec<Element> { Vec::new() }
}

fn worksheet(route: RwSignal<(bool, String)>, ready: RwSignal<bool>) -> WorksheetView {
    let query = Memo::new(move |_| route.with(|r| r.1.clone()));
    let loading = RwSignal::new(false);
    let all_results = RwSignal::new(false);
    let request_generation = RwSignal::new(0_u64);
    on_cleanup(|| println!("cleanup Worksheet signals"));
    let page = Resource::new_blocking(
        move || query.get(),
        move |query| {
            let generation = request_generation.get_untracked().wrapping_add(1);
            request_generation.set(generation);
            println!("fetch {query:?}; loading true");
            loading.set(true);
            async move {
                tokio::task::yield_now().await;
                if request_generation.try_get_untracked() == Some(generation) {
                    println!("fetch done; loading false");
                    loading.try_set(false);
                }
                25_u32
            }
        },
    );
    let was_loading = RwSignal::new(false);
    Effect::new(move |_| {
        println!("read loading Effect");
        if loading.get() {
            was_loading.set(true);
        } else if was_loading.get_untracked() {
            was_loading.set(false);
        }
    });
    let initial = RwSignal::new(true);
    Effect::new(move |_| {
        let _ = query.get();
        if !initial.get_untracked() {
            println!("clear all_results");
            all_results.set(false);
        }
        initial.set(false);
    });
    let mut effects = Vec::new();
    for name in ["aria-busy", "hidden"] {
        effects.push(RenderEffect::new(move |_| {
            println!("read loading {name}");
            loading.get()
        }));
    }
    let page_effect = RenderEffect::new(move |previous| {
        let data = page.get();
        println!("page render {data:?}");
        let mut rows = Vec::new();
        if data.is_some() {
            for index in 0..25 {
                rows.push(RenderEffect::new(move |_| {
                    println!("read checkbox {index}");
                    !ready.get() || all_results.get()
                }));
            }
        }
        drop(previous);
        rows
    });
    WorksheetView { _effects: effects, _page: page_effect }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    _ = Executor::init_tokio();
    tokio::task::LocalSet::new().run_until(async {
        let root = Owner::new();
        root.set();
        let route = RwSignal::new((true, "AUR-00002".to_string()));
        let owner = root.child();
        owner.set();
        on_cleanup(|| println!("cleanup shell signals"));
        let task = Memo::new(move |_| route.with(|r| r.0));
        let ready = RwSignal::new(false);
        let idempotent = std::env::args().any(|arg| arg == "--idempotent-ready");
        Effect::new(move |_| {
            let _ = route.get();
            println!("shell marks ready");
            if !idempotent || !ready.get_untracked() { ready.set(true); }
        });
        for _ in 0..2 {Executor::tick().await;}
        let shell_owner = owner.clone();
        let stable = std::env::args().any(|arg| arg == "--stable-owner");
        println!("stable owner variant: {stable}");
        let view = RenderEffect::new(move |previous| {
            println!("task render {}", task.get());
            let task_owner = if stable {shell_owner.child()} else {Owner::new()};
            let next = task_owner.with(|| TaskView(if task.get() {Some(Box::new(worksheet(route, ready)) as Box<dyn Any>)} else {None}));
            let next = OwnedView::new_with_owner(next, task_owner);
            match previous {
                Some(mut state) => {next.rebuild(&mut state);state},
                None => next.build(),
            }
        });
        let mut route_view = OwnedView::new_with_owner(TaskView(Some(Box::new(view))), owner).build();
        for _ in 0..8 {Executor::tick().await;}
        assert!(ready.get_untracked());
        println!("--- query page2 ---");
        route.set((true, "page=2&search=AUR".into()));
        for _ in 0..8 {Executor::tick().await;}
        assert!(ready.get_untracked());
        println!("--- navigate document ---");
        route.set((false, "".into()));
        for _ in 0..8 {Executor::tick().await;}
        assert!(ready.get_untracked());
        println!("--- navigate history ---");
        route.set((true, "AUR-00002".into()));
        for _ in 0..8 {Executor::tick().await;}
        println!("--- navigate home ---");
        route.set((true, "home".into()));
        let home_owner = root.child();
        OwnedView::new_with_owner(TaskView(None), home_owner).rebuild(&mut route_view);
        for _ in 0..8 {Executor::tick().await;}
        println!("--- done ---");
    }).await;
}
