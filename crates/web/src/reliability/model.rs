use super::{
    draft::{Confirmation, Draft, FieldError, Pending},
    network,
};
use crate::http::{RequestFailure, operation_id};
use canteiro_contracts::{
    auth::AuthFailure,
    reliability::{
        ContractDetail, ContractSummary, ReliabilityCommand, ReliabilityContext, ReliabilityReply,
        ReliabilityScope, SaveReceipt,
    },
};
use leptos::prelude::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

type DraftKey = (String, String);

#[derive(Default)]
struct Retained {
    actor: String,
    lease_id: Option<String>,
    last_draft: BTreeMap<String, String>,
    drafts: BTreeMap<DraftKey, Draft>,
}

impl Retained {
    fn bind(&mut self, actor: &str) {
        if self.actor != actor {
            *self = Self {
                actor: actor.to_owned(),
                ..Self::default()
            };
        }
    }
}

#[derive(Clone, Copy)]
struct DraftStore(RwSignal<Retained>);

pub(super) fn provide_draft_store() {
    let store = DraftStore(RwSignal::new(Retained::default()));
    provide_context(store);
    #[cfg(feature = "hydrate")]
    {
        let leaving = window_event_listener(leptos::ev::beforeunload, move |event| {
            if store.0.with_untracked(|r| !r.drafts.is_empty()) {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || leaving.remove());
    }
}

#[derive(Clone, Copy)]
pub(super) enum Reservation {
    Acquire,
    Renew,
    Release,
}

#[derive(Clone, Copy)]
pub(super) struct History {
    pub context: RwSignal<Option<ReliabilityContext>>,
    pub detail: RwSignal<Option<ContractDetail>>,
    pub draft: RwSignal<Option<Draft>>,
    pub selection: RwSignal<String>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<Option<AuthFailure>>,
    pub invalid_field: RwSignal<Option<&'static str>>,
    pub message: RwSignal<String>,
    mine: RwSignal<Option<String>>,
    active_id: RwSignal<Option<String>>,
    store: DraftStore,
    active: StoredValue<Arc<AtomicBool>>,
    generation: RwSignal<u64>,
}

struct Ticket {
    active: Arc<AtomicBool>,
    generation: u64,
}

impl History {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(true));
        let cleanup = Arc::clone(&active);
        on_cleanup(move || cleanup.store(false, Ordering::Release));
        Self {
            context: RwSignal::new(None),
            detail: RwSignal::new(None),
            draft: RwSignal::new(None),
            selection: RwSignal::new(String::new()),
            busy: RwSignal::new(false),
            error: RwSignal::new(None),
            invalid_field: RwSignal::new(None),
            message: RwSignal::new(String::new()),
            mine: RwSignal::new(None),
            active_id: RwSignal::new(None),
            store: expect_context(),
            active: StoredValue::new(active),
            generation: RwSignal::new(0),
        }
    }

    fn begin(self) -> Option<Ticket> {
        if self.busy.get_untracked() {
            return None;
        }
        self.busy.set(true);
        self.error.set(None);
        self.invalid_field.set(None);
        self.message.set(String::new());
        self.generation
            .update(|value| *value = value.wrapping_add(1));
        Some(Ticket {
            active: self.active.get_value(),
            generation: self.generation.get_untracked(),
        })
    }

    fn current(self, ticket: &Ticket) -> bool {
        ticket.active.load(Ordering::Acquire)
            && self.generation.get_untracked() == ticket.generation
    }

    fn finish(self) {
        self.busy.set(false);
    }

    #[cfg(feature = "hydrate")]
    pub fn revalidate(self) {
        if self.busy.get_untracked() {
            self.generation
                .update(|value| *value = value.wrapping_add(1));
            self.finish();
        }
        self.load();
    }

    #[cfg(feature = "hydrate")]
    pub fn hide(self) {
        self.generation
            .update(|value| *value = value.wrapping_add(1));
        self.busy.set(false);
        self.hide_private();
        self.error.set(None);
        self.message.set(String::new());
    }

    fn hide_private(self) {
        self.context.set(None);
        self.detail.set(None);
        self.draft.set(None);
        self.mine.set(None);
        self.selection.set(String::new());
    }

    pub fn load(self) {
        let selection = self.selection.get_untracked();
        let opened = self.active_id.get_untracked();
        let previous = self.context.get_untracked();
        let mine = self.mine.get_untracked();
        let Some(ticket) = self.begin() else {
            return;
        };
        self.hide_private();
        leptos::task::spawn_local(async move {
            let response = network::context().await;
            if !self.current(&ticket) {
                return;
            }
            match response {
                Ok(context) => {
                    self.store.0.update(|store| store.bind(&context.actor_id));
                    let same_scope = previous.as_ref().is_some_and(|previous| {
                        previous.actor_id == context.actor_id
                            && previous.organization.as_ref().map(|o| &o.scope)
                                == context.organization.as_ref().map(|o| &o.scope)
                    });
                    let mut candidate = None;
                    if let Some(organization) = &context.organization {
                        let allowed = |id: &str| organization.contracts.iter().any(|c| c.id == id);
                        candidate = same_scope
                            .then_some(opened.as_deref())
                            .flatten()
                            .filter(|id| allowed(id))
                            .map(str::to_owned)
                            .or_else(|| {
                                self.store.0.with_untracked(|store| {
                                    store
                                        .last_draft
                                        .get(&organization.scope.organization_id)
                                        .filter(|id| allowed(id))
                                        .cloned()
                                })
                            });
                        if same_scope && allowed(&selection) {
                            self.selection.set(selection);
                        }
                    }
                    self.active_id.set(candidate.clone());
                    self.context.set(Some(context));
                    if let Some(id) = candidate
                        && let Some(scope) = self.scope()
                    {
                        if !same_scope || self.selection.get_untracked().is_empty() {
                            self.selection.set(id.clone());
                        }
                        self.mine.set(if same_scope { mine } else { None });
                        self.read_detail(&ticket, &scope, &id, None).await;
                        if self.current(&ticket) && self.detail.get_untracked().is_none() {
                            self.active_id.set(None);
                        }
                    }
                }
                Err(failure) => {
                    self.active_id.set(None);
                    self.show_error(failure.details, None);
                }
            }
            if self.current(&ticket) {
                self.finish();
            }
        });
    }

    pub fn can_open(self) -> bool {
        !self.busy.get()
            && self.selection.with(|id| {
                !id.is_empty() && self.active_id.with(|active| active.as_ref() != Some(id))
            })
    }

    pub fn open(self) {
        let id = self.selection.get_untracked();
        let Some(scope) = self.scope() else { return };
        if id.is_empty() || self.active_id.get_untracked().as_ref() == Some(&id) {
            return;
        }
        let Some(ticket) = self.begin() else { return };
        self.active_id.set(Some(id.clone()));
        self.detail.set(None);
        self.draft.set(None);
        self.mine.set(None);
        leptos::task::spawn_local(async move {
            self.read_detail(&ticket, &scope, &id, None).await;
            if self.current(&ticket) {
                if self.detail.get_untracked().is_some() {
                    self.focus("history-contract-heading");
                } else {
                    self.active_id.set(None);
                }
                self.finish();
            }
        });
    }

    pub fn refresh_detail(self, more: bool) {
        let Some((scope, id)) = self.target() else {
            return;
        };
        let after = if more {
            let Some(cursor) = self
                .detail
                .with_untracked(|d| d.as_ref().and_then(|d| d.next_after))
            else {
                return;
            };
            Some(cursor)
        } else {
            None
        };
        let Some(ticket) = self.begin() else { return };
        leptos::task::spawn_local(async move {
            self.read_detail(&ticket, &scope, &id, after).await;
            if self.current(&ticket) {
                if more {
                    self.focus("history-new-entries");
                }
                self.finish();
            }
        });
    }

    async fn read_detail(
        self,
        ticket: &Ticket,
        scope: &ReliabilityScope,
        id: &str,
        after: Option<i64>,
    ) {
        let response = network::detail(scope, id, after).await;
        if !self.current(ticket) {
            return;
        }
        match response {
            Ok(mut result) => {
                let Some(key) = self.draft_key() else { return };
                if result.lease_until != self.mine.get_untracked() {
                    self.mine.set(None);
                }
                if after.is_some()
                    && let Some(previous) = self.detail.get_untracked()
                {
                    let mut history = previous.history;
                    history.append(&mut result.history);
                    result.history = history;
                }
                let draft = result.contract.can_edit.then(|| {
                    self.store
                        .0
                        .with_untracked(|store| store.drafts.get(&key).cloned())
                        .unwrap_or_else(|| Draft::new(&result.contract))
                });
                self.update_contract_summary(&result.contract);
                self.draft.set(draft);
                self.detail.set(Some(result));
            }
            Err(failure) => self.failed(failure),
        }
    }

    fn update_contract_summary(self, latest: &ContractSummary) {
        self.context.update(|context| {
            if let Some(organization) = context.as_mut().and_then(|c| c.organization.as_mut())
                && let Some(summary) = organization
                    .contracts
                    .iter_mut()
                    .find(|c| c.id == latest.id)
            {
                summary.clone_from(latest);
            }
        });
    }

    pub fn scope(self) -> Option<ReliabilityScope> {
        self.context.with_untracked(|context| {
            context
                .as_ref()
                .and_then(|context| context.organization.as_ref().map(|o| o.scope.clone()))
        })
    }

    fn target(self) -> Option<(ReliabilityScope, String)> {
        Some((self.scope()?, self.active_id.get_untracked()?))
    }

    fn draft_key(self) -> Option<DraftKey> {
        let (scope, id) = self.target()?;
        Some((scope.organization_id, id))
    }

    pub fn mine(self) -> Option<String> {
        self.mine.get()
    }

    #[cfg(feature = "hydrate")]
    pub fn elapsed_lease(self) {
        if self.busy.get_untracked() {
            return;
        }
        let Some(until) = self
            .detail
            .with_untracked(|d| d.as_ref().and_then(|d| d.lease_until.clone()))
        else {
            return;
        };
        if js_sys::Date::now() >= js_sys::Date::parse(&until) {
            self.update_lease(None);
        }
    }

    pub fn conflicting(self) -> bool {
        self.draft.with(|draft| {
            draft.as_ref().is_some_and(|draft| {
                draft.dirty()
                    && self.detail.with(|detail| {
                        detail
                            .as_ref()
                            .is_some_and(|d| d.contract.version != draft.base_version)
                    })
            })
        })
    }

    pub fn can_submit(self) -> bool {
        !self.busy.get()
            && self.draft.with(|draft| {
                draft.as_ref().is_some_and(|draft| {
                    if let Some(pending) = &draft.pending {
                        return pending.confirmation == Confirmation::Absent
                            && self.mine.get().is_some();
                    }
                    draft.dirty() && self.mine.get().is_some() && !self.conflicting()
                })
            })
    }

    pub fn set_field(self, change: impl FnOnce(&mut Draft)) {
        let Some(mut draft) = self.draft.get_untracked() else {
            return;
        };
        if draft.pending.is_some() || self.busy.get_untracked() {
            return;
        }
        change(&mut draft);
        #[cfg(feature = "hydrate")]
        {
            draft.edited_at = js_sys::Date::new_0().to_iso_string().as_string();
        }
        self.invalid_field.set(None);
        self.store_draft(draft);
    }

    fn store_draft(self, draft: Draft) {
        if let Some(key) = self.draft_key() {
            self.store.0.update(|store| {
                if draft.dirty() {
                    store.last_draft.insert(key.0.clone(), key.1.clone());
                    store.drafts.insert(key, draft.clone());
                } else {
                    store.drafts.remove(&key);
                    if store.last_draft.get(&key.0) == Some(&key.1) {
                        store.last_draft.remove(&key.0);
                        if let Some((_, other)) = store
                            .drafts
                            .keys()
                            .find(|(organization, _)| *organization == key.0)
                        {
                            store.last_draft.insert(key.0, other.clone());
                        }
                    }
                }
            });
        }
        self.draft.set(Some(draft));
    }

    pub fn rebase(self) {
        if let Some((mut draft, contract)) = self.editable()
            && draft.pending.is_none()
        {
            draft.rebase(&contract);
            self.store_draft(draft);
            self.error.set(None);
            self.message.set(format!(
                "Rascunho preservado sobre a versão {}. Confira os campos antes de salvar.",
                contract.version
            ));
            self.focus("history-title");
        }
    }

    pub fn discard(self) {
        if let Some((draft, contract)) = self.editable()
            && draft.pending.is_none()
        {
            self.store_draft(Draft::new(&contract));
            self.error.set(None);
            self.message.set("Rascunho descartado.".into());
            self.focus("history-title");
        }
    }

    fn editable(self) -> Option<(Draft, ContractSummary)> {
        if self.busy.get_untracked() {
            return None;
        }
        let contract = self.detail.get_untracked()?.contract;
        contract
            .can_edit
            .then_some((self.draft.get_untracked()?, contract))
    }

    fn lease_id(self) -> Result<String, AuthFailure> {
        if let Some(id) = self.store.0.with_untracked(|store| store.lease_id.clone()) {
            return Ok(id);
        }
        let id = operation_id()?;
        self.store
            .0
            .update(|store| store.lease_id = Some(id.clone()));
        Ok(id)
    }

    pub fn reserve(self, kind: Reservation) {
        let Some((scope, contract_id)) = self.target() else {
            return;
        };
        let Ok(lease_id) = self
            .lease_id()
            .inspect_err(|failure| self.show_error(failure.clone(), None))
        else {
            return;
        };
        let command = match kind {
            Reservation::Acquire => ReliabilityCommand::Acquire {
                scope: scope.clone(),
                contract_id: contract_id.clone(),
                lease_id,
            },
            Reservation::Renew => ReliabilityCommand::Renew {
                scope: scope.clone(),
                contract_id: contract_id.clone(),
                lease_id,
            },
            Reservation::Release => ReliabilityCommand::Release {
                scope: scope.clone(),
                contract_id: contract_id.clone(),
                lease_id,
            },
        };
        let Some((ticket, csrf)) = self.mutation() else {
            return;
        };
        leptos::task::spawn_local(async move {
            let result = network::command(&command, &csrf).await;
            if !self.current(&ticket) {
                return;
            }
            match result {
                Ok(ReliabilityReply::Lease { expires_at }) => {
                    let text = match kind {
                        Reservation::Acquire => "Reserva confirmada. Você já pode salvar o título.",
                        Reservation::Renew => "Prazo da reserva renovado.",
                        Reservation::Release => {
                            "Reserva liberada. Seu rascunho continua nesta aba."
                        }
                    };
                    self.message.set(text.into());
                    self.update_lease(expires_at);
                    self.focus(if matches!(kind, Reservation::Release) {
                        "history-acquire"
                    } else {
                        "history-lease-status"
                    });
                }
                Err(failure) => {
                    let refresh = lost_lease(&failure);
                    self.failed(failure);
                    if refresh {
                        self.read_detail(&ticket, &scope, &contract_id, None).await;
                    }
                }
                Ok(_) => self.show_error(network::unavailable(), None),
            }
            if self.current(&ticket) {
                self.finish();
            }
        });
    }

    fn mutation(self) -> Option<(Ticket, String)> {
        let csrf = self
            .context
            .with_untracked(|c| c.as_ref().map(|c| c.csrf.clone()))?;
        Some((self.begin()?, csrf))
    }

    fn update_lease(self, until: Option<String>) {
        self.mine.set(until.clone());
        self.detail.update(|value| {
            if let Some(detail) = value {
                detail.lease_until = until;
            }
        });
    }

    pub fn save(self) {
        if !self.can_submit() {
            return;
        }
        let Some((scope, id)) = self.target() else {
            return;
        };
        let Some(mut draft) = self.draft.get_untracked() else {
            return;
        };
        let retrying = draft.pending.is_some();
        let key = match draft.pending.as_ref() {
            Some(pending) => Ok(pending.key.clone()),
            None => operation_id(),
        };
        let Ok(key) = key.inspect_err(|failure| self.show_error(failure.clone(), None)) else {
            return;
        };
        let Ok(lease) = self
            .lease_id()
            .inspect_err(|failure| self.show_error(failure.clone(), None))
        else {
            return;
        };
        let command = match draft.save_command(scope.clone(), id.clone(), lease, key.clone()) {
            Ok(command) => command,
            Err(FieldError { field, message }) => {
                self.show_error(
                    AuthFailure {
                        code: "invalid_input".into(),
                        message: message.into(),
                    },
                    Some(field),
                );
                return;
            }
        };
        let Some((ticket, csrf)) = self.mutation() else {
            return;
        };
        draft.pending = Some(Pending {
            key,
            confirmation: Confirmation::Unconfirmed,
        });
        self.store_draft(draft);
        leptos::task::spawn_local(async move {
            let response = network::command(&command, &csrf).await;
            if !self.current(&ticket) {
                return;
            }
            match response {
                Ok(ReliabilityReply::Saved { receipt }) => {
                    if self.record_receipt(&id, receipt) {
                        self.read_detail(&ticket, &scope, &id, None).await;
                    }
                }
                Ok(ReliabilityReply::VersionConflict { current }) => {
                    self.settle_unsaved();
                    self.detail.update(|value| {
                        if let Some(detail) = value {
                            detail.contract = current;
                        }
                    });
                    self.focus("history-conflict-heading");
                    self.read_detail(&ticket, &scope, &id, None).await;
                }
                Err(failure) => {
                    let refresh = lost_lease(&failure);
                    if failure.details.code == "idempotency_conflict" {
                        self.set_confirmation(Confirmation::KeyConflict);
                    } else if !retrying && (matches!(failure.status, Some(400 | 429)) || refresh) {
                        self.settle_unsaved();
                    }
                    self.failed(failure);
                    if refresh {
                        self.read_detail(&ticket, &scope, &id, None).await;
                    }
                }
                Ok(_) => self.show_error(network::unavailable(), None),
            }
            if self.current(&ticket) {
                self.finish();
            }
        });
    }

    pub fn check_operation(self) {
        let Some((scope, id)) = self.target() else {
            return;
        };
        let Some(draft) = self.draft.get_untracked() else {
            return;
        };
        let Some(pending) = draft.pending else { return };
        if pending.confirmation == Confirmation::KeyConflict {
            return;
        }
        let Some(ticket) = self.begin() else { return };
        leptos::task::spawn_local(async move {
            let response = network::operation(&scope, &id, &pending.key).await;
            if !self.current(&ticket) {
                return;
            }
            match response {
                Ok(Some(receipt)) => {
                    if self.record_receipt(&id, receipt) {
                        self.read_detail(&ticket, &scope, &id, None).await;
                    }
                }
                Ok(None) => {
                    self.set_confirmation(Confirmation::Absent);
                    self.message.set("Nenhuma gravação encontrada para esta tentativa. Com a edição reservada, você pode repetir o mesmo envio.".into());
                    self.focus("history-pending");
                }
                Err(failure) => self.failed(failure),
            }
            if self.current(&ticket) {
                self.finish();
            }
        });
    }

    fn set_confirmation(self, confirmation: Confirmation) {
        if let Some(mut draft) = self.draft.get_untracked()
            && let Some(pending) = draft.pending.as_mut()
        {
            pending.confirmation = confirmation;
            self.store_draft(draft);
        }
    }

    pub fn resolve_key_conflict(self) {
        if !self.busy.get_untracked() && self.confirmation() == Some(Confirmation::KeyConflict) {
            self.settle_unsaved();
            self.error.set(None);
            self.message.set(
                "Rascunho preservado. Confira os campos antes de iniciar outra gravação.".into(),
            );
            self.focus("history-title");
        }
    }

    pub fn confirmation(self) -> Option<Confirmation> {
        self.draft.with(|d| {
            d.as_ref()
                .and_then(|d| d.pending.as_ref().map(|p| p.confirmation.clone()))
        })
    }

    fn settle_unsaved(self) {
        if let Some(mut draft) = self.draft.get_untracked() {
            draft.pending = None;
            self.store_draft(draft);
        }
    }

    fn record_receipt(self, id: &str, receipt: SaveReceipt) -> bool {
        let matches = self.draft.with_untracked(|d| {
            d.as_ref().is_some_and(|d| {
                receipt.contract_id == id
                    && d.base_version.checked_add(1) == Some(receipt.version)
                    && d.title.trim() == receipt.title
            })
        });
        if !matches {
            self.set_confirmation(Confirmation::KeyConflict);
            self.show_error(AuthFailure {
                code: "receipt_mismatch".into(),
                message: "O resultado encontrado pertence a outra gravação. Confira o histórico e retome o rascunho.".into(),
            }, None);
            return false;
        }
        self.update_lease(None);
        if let Some(mut current) = self.detail.get_untracked() {
            if receipt.version >= current.contract.version {
                current.contract.version = receipt.version;
                current.contract.title = receipt.title.clone();
            }
            self.store_draft(Draft::new(&current.contract));
            self.update_contract_summary(&current.contract);
            self.detail.set(Some(current));
        }
        self.message.set(format!(
            "Gravação confirmada na versão {}. Título registrado nesta alteração: {}.",
            receipt.version, receipt.title
        ));
        self.focus("history-feedback");
        true
    }

    fn failed(self, failure: RequestFailure) {
        let denied = matches!(failure.status, Some(401 | 403 | 404))
            || failure.details.code == "context_changed";
        if denied {
            self.active_id.set(None);
            self.hide_private();
        } else if lost_lease(&failure) {
            self.mine.set(None);
        }
        self.show_error(failure.details, None);
    }

    fn show_error(self, failure: AuthFailure, field: Option<&'static str>) {
        self.error.set(Some(failure));
        self.invalid_field.set(field);
        self.focus(field.unwrap_or("history-feedback"));
    }

    fn focus(self, id: &'static str) {
        #[cfg(feature = "hydrate")]
        {
            let active = self.active.get_value();
            let generation = self.generation.get_untracked();
            request_animation_frame(move || {
                if active.load(Ordering::Acquire) && generation == self.generation.get_untracked() {
                    super::focus(id);
                }
            });
        }
        #[cfg(not(feature = "hydrate"))]
        let _ = id;
    }
}

fn lost_lease(failure: &RequestFailure) -> bool {
    matches!(
        failure.details.code.as_str(),
        "edit_reserved" | "edit_expired"
    )
}
