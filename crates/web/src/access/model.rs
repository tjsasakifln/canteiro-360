use super::{close_dialog, focus, format, network};
use canteiro_contracts::{access::*, auth::AuthFailure, organizations::OrganizationScope};
use canteiro_domain::decimal::Money;
use leptos::prelude::*;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) const ROLES: [AccessRole; 10] = AccessRole::ALL;
pub(super) const ACTIONS: [AccessAction; AccessAction::ALL.len()] = AccessAction::ALL;
pub(super) const DECISIONS: [AccessAction; 6] = [
    AccessAction::ApproveBudget,
    AccessAction::ApproveChange,
    AccessAction::AuthorizePayment,
    AccessAction::TechnicalAccept,
    AccessAction::ApproveProjectScope,
    AccessAction::ApproveTechnicalException,
];

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Draft {
    pub selected_scope: String,
    pub scope_name: String,
    pub scope_kind: ScopeKind,
    pub scope_parent: String,
    pub grant_email: String,
    pub grant_role: AccessRole,
    pub grant_actions: Vec<AccessAction>,
    pub grant_limit: String,
    pub grant_from: String,
    pub grant_until: String,
    pub grant_required: bool,
    pub source_id: String,
    pub source_version: i64,
    pub delegate_email: String,
    pub delegate_reason: String,
    pub delegate_actions: Vec<AccessAction>,
    pub delegate_limit: String,
    pub delegate_from: String,
    pub delegate_until: String,
    pub substitution: bool,
    pub policy_action: AccessAction,
    pub policy_threshold: String,
    pub policy_double: bool,
    pub policy_segregate: bool,
    pub policy_version: Option<i64>,
    pub request_id: String,
    pub request_version: i64,
    pub request_title: String,
    pub request_action: AccessAction,
    pub request_increase: String,
    pub request_decrease: String,
    pub request_conflicts: Vec<String>,
    pub changed: bool,
    pub dirty_forms: u8,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            selected_scope: String::new(),
            scope_name: String::new(),
            scope_kind: ScopeKind::Work,
            scope_parent: String::new(),
            grant_email: String::new(),
            grant_role: AccessRole::Viewer,
            grant_actions: vec![AccessAction::View],
            grant_limit: "0,00".into(),
            grant_from: String::new(),
            grant_until: String::new(),
            grant_required: false,
            source_id: String::new(),
            source_version: 0,
            delegate_email: String::new(),
            delegate_reason: String::new(),
            delegate_actions: vec![],
            delegate_limit: "0,00".into(),
            delegate_from: String::new(),
            delegate_until: String::new(),
            substitution: false,
            policy_action: AccessAction::ApproveChange,
            policy_threshold: "0,00".into(),
            policy_double: true,
            policy_segregate: true,
            policy_version: None,
            request_id: String::new(),
            request_version: 0,
            request_title: String::new(),
            request_action: AccessAction::ApproveChange,
            request_increase: "0,00".into(),
            request_decrease: "0,00".into(),
            request_conflicts: vec![],
            changed: false,
            dirty_forms: 0,
        }
    }
}
#[derive(Clone)]
pub(super) struct Confirmation {
    pub command: Option<AccessCommand>,
    transition: Option<DraftTransition>,
    pub title: String,
    pub description: String,
    pub return_focus: String,
}
#[derive(Clone)]
enum DraftTransition {
    Scope(String),
    Policy(AccessAction),
    Delegation(Box<GrantView>),
    Revision(Box<RequestView>),
}
#[derive(Clone)]
struct Suspended {
    user_id: String,
    organization_id: Option<String>,
    can_manage: bool,
    draft: Draft,
}

#[derive(Clone)]
struct PendingAuthority {
    user_id: String,
    command: AccessCommand,
    key: String,
    absence_checked: bool,
}

impl PendingAuthority {
    fn matches(&self, state: &AccessState) -> bool {
        self.user_id == state.current_user_id
            && state
                .organization
                .as_ref()
                .is_some_and(|o| o.id == self.command.context().organization_id)
    }

    fn retry(&self, context_version: i64) -> AccessCommand {
        let mut command = self.command.clone();
        if let AccessCommand::Propose { context, .. }
        | AccessCommand::ReviseRequest { context, .. }
        | AccessCommand::Decide { context, .. } = &mut command
        {
            context.context_version = context_version;
        }
        command
    }
}

/// Only an identity-bound draft survives route changes. Server responses and
/// CSRF material remain owned by the permission page and are fetched again.
#[derive(Clone, Copy)]
struct DraftStore {
    suspended: RwSignal<Option<Suspended>>,
    authority: RwSignal<BTreeMap<String, PendingAuthority>>,
}

pub(super) fn provide_draft_store() {
    let store = DraftStore {
        suspended: RwSignal::new(None),
        authority: RwSignal::new(BTreeMap::new()),
    };
    provide_context(store);
    #[cfg(feature = "hydrate")]
    {
        let leaving = window_event_listener(leptos::ev::beforeunload, move |event| {
            if store
                .authority
                .with_untracked(|attempts| !attempts.is_empty())
                || store
                    .suspended
                    .with_untracked(|saved| saved.as_ref().is_some_and(|s| s.draft.changed))
            {
                event.prevent_default();
                event.set_return_value("");
            }
        });
        on_cleanup(move || leaving.remove());
    }
}

#[derive(Clone, Copy)]
pub(super) struct Access {
    pub state: RwSignal<Option<AccessState>>,
    pub draft: RwSignal<Draft>,
    pub ready: RwSignal<bool>,
    pub busy: RwSignal<bool>,
    pub validating: RwSignal<bool>,
    pub error: RwSignal<Option<AuthFailure>>,
    pub message: RwSignal<String>,
    pub pending: RwSignal<Option<Confirmation>>,
    csrf: RwSignal<String>,
    generation: RwSignal<u64>,
    mutation: RwSignal<bool>,
    refresh_after_mutation: RwSignal<bool>,
    suspended: RwSignal<Option<Suspended>>,
    authority: RwSignal<BTreeMap<String, PendingAuthority>>,
    active: StoredValue<Arc<AtomicBool>>,
}
impl Access {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(true));
        let cleanup_active = Arc::clone(&active);
        let store = use_context::<DraftStore>().unwrap_or_else(|| DraftStore {
            suspended: RwSignal::new(None),
            authority: RwSignal::new(BTreeMap::new()),
        });
        let access = Self {
            state: RwSignal::new(None),
            draft: RwSignal::new(Draft::default()),
            ready: RwSignal::new(false),
            busy: RwSignal::new(false),
            validating: RwSignal::new(false),
            error: RwSignal::new(None),
            message: RwSignal::new(String::new()),
            pending: RwSignal::new(None),
            csrf: RwSignal::new(String::new()),
            generation: RwSignal::new(0),
            mutation: RwSignal::new(false),
            refresh_after_mutation: RwSignal::new(false),
            suspended: store.suspended,
            authority: store.authority,
            active: StoredValue::new(active),
        };
        on_cleanup(move || {
            cleanup_active.store(false, Ordering::Release);
            access.generation.update(|g| *g = g.wrapping_add(1));
            access.save_draft();
        });
        access
    }
    pub fn context(self) -> Option<OrganizationScope> {
        let s = self.state.get_untracked()?;
        let o = s.organization?;
        Some(OrganizationScope {
            organization_id: o.id,
            context_version: s.context_version,
            organization_version: o.version,
        })
    }
    pub fn can_manage(self) -> bool {
        self.state
            .with(|s| s.as_ref().is_some_and(|s| s.can_manage))
    }
    pub fn blocked(self) -> bool {
        !self.ready.get() || self.busy.get() || self.validating.get()
    }
    pub fn dirty(self) -> bool {
        self.authority
            .with_untracked(|attempts| !attempts.is_empty())
            || self.draft.get_untracked().changed
            || self
                .suspended
                .get_untracked()
                .is_some_and(|s| s.draft.changed)
    }
    pub fn edit(self, f: impl FnOnce(&mut Draft)) {
        self.draft.update(|d| {
            let before = d.clone();
            f(d);
            if (d.scope_name.as_str(), d.scope_kind, d.scope_parent.as_str())
                != (
                    before.scope_name.as_str(),
                    before.scope_kind,
                    before.scope_parent.as_str(),
                )
            {
                d.dirty_forms |= 1;
            }
            if (
                d.grant_email.as_str(),
                d.grant_role,
                &d.grant_actions,
                d.grant_limit.as_str(),
                d.grant_from.as_str(),
                d.grant_until.as_str(),
                d.grant_required,
            ) != (
                before.grant_email.as_str(),
                before.grant_role,
                &before.grant_actions,
                before.grant_limit.as_str(),
                before.grant_from.as_str(),
                before.grant_until.as_str(),
                before.grant_required,
            ) {
                d.dirty_forms |= 2;
            }
            if (
                d.delegate_email.as_str(),
                d.delegate_reason.as_str(),
                &d.delegate_actions,
                d.delegate_limit.as_str(),
                d.delegate_from.as_str(),
                d.delegate_until.as_str(),
                d.substitution,
            ) != (
                before.delegate_email.as_str(),
                before.delegate_reason.as_str(),
                &before.delegate_actions,
                before.delegate_limit.as_str(),
                before.delegate_from.as_str(),
                before.delegate_until.as_str(),
                before.substitution,
            ) {
                d.dirty_forms |= 4;
            }
            if (
                d.policy_threshold.as_str(),
                d.policy_double,
                d.policy_segregate,
            ) != (
                before.policy_threshold.as_str(),
                before.policy_double,
                before.policy_segregate,
            ) {
                d.dirty_forms |= 8;
            }
            if (
                d.request_title.as_str(),
                d.request_action,
                d.request_increase.as_str(),
                d.request_decrease.as_str(),
                &d.request_conflicts,
            ) != (
                before.request_title.as_str(),
                before.request_action,
                before.request_increase.as_str(),
                before.request_decrease.as_str(),
                &before.request_conflicts,
            ) {
                d.dirty_forms |= 16;
            }
            d.changed = d.dirty_forms != 0;
        });
    }
    pub fn scopes(self) -> Vec<ScopeView> {
        self.state.get().map(|s| s.scopes).unwrap_or_default()
    }
    pub fn grants(self) -> Vec<GrantView> {
        let id = self.draft.get().selected_scope;
        self.state
            .get()
            .map(|s| s.grants.into_iter().filter(|g| g.scope_id == id).collect())
            .unwrap_or_default()
    }
    pub fn requests(self) -> Vec<RequestView> {
        let id = self.draft.get().selected_scope;
        self.state
            .get()
            .map(|s| {
                s.requests
                    .into_iter()
                    .filter(|r| r.scope_id == id)
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn scope_name(self) -> String {
        let id = self.draft.get().selected_scope;
        self.scopes()
            .into_iter()
            .find(|s| s.id == id)
            .map(|s| s.name)
            .unwrap_or_else(|| "Escolha uma obra, contrato ou orçamento".into())
    }
    pub fn local_error(self, message: impl Into<String>, field: &'static str) {
        self.error.set(Some(AuthFailure {
            code: "invalid_input".into(),
            message: message.into(),
        }));
        let active = self.active.get_value();
        let generation = self.generation.get_untracked();
        request_animation_frame(move || {
            if active.load(Ordering::Acquire) && self.generation.get_untracked() == generation {
                focus(field);
            }
        });
    }
    fn save_draft(self) {
        if let Some(state) = self.state.get_untracked() {
            self.suspended.set(Some(Suspended {
                user_id: state.current_user_id,
                organization_id: state.organization.map(|o| o.id),
                can_manage: state.can_manage,
                draft: self.draft.get_untracked(),
            }));
        }
    }
    pub fn hide(self) {
        self.generation.update(|g| *g = g.wrapping_add(1));
        close_dialog();
        self.pending.set(None);
        self.save_draft();
        self.state.set(None);
        self.csrf.set(String::new());
        self.draft.set(Draft::default());
        self.ready.set(false);
        self.validating.set(true);
        self.message.set(String::new());
    }
    pub fn load(self, announce: bool) {
        self.hide();
        self.busy.set(true);
        self.error.set(None);
        if self.mutation.get_untracked() {
            self.refresh_after_mutation.set(true);
            return;
        }
        let generation = self.generation.get_untracked();
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::context().await;
            if !active.load(Ordering::Acquire) || generation != self.generation.get_untracked() {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(r) => {
                    self.accept(r);
                    if announce {
                        self.message
                            .set("Acesso atualizado. Nenhuma operação foi reenviada.".into());
                    }
                }
                Err(e) => self.failed(e),
            }
        });
    }
    fn accept(self, response: AccessResponse) {
        let next = &response.state;
        self.authority.update(|attempts| {
            attempts.retain(|_, attempt| attempt.user_id == next.current_user_id);
            for attempt in attempts.values_mut() {
                attempt.absence_checked = false;
            }
        });
        let mut draft = self.draft.get_untracked();
        if let Some(saved) = self.suspended.get_untracked() {
            if saved.user_id == next.current_user_id
                && saved.organization_id == next.organization.as_ref().map(|o| o.id.clone())
                && (!saved.can_manage || next.can_manage)
            {
                draft = saved.draft;
            }
            self.suspended.set(None);
        } else if let Some(previous) = self.state.get_untracked()
            && (previous.current_user_id != next.current_user_id
                || previous.organization.as_ref().map(|o| &o.id)
                    != next.organization.as_ref().map(|o| &o.id)
                || previous.context_version != next.context_version
                || (previous.can_manage && !next.can_manage))
        {
            draft = Draft::default();
        }
        if !draft.selected_scope.is_empty()
            && !next.scopes.iter().any(|s| s.id == draft.selected_scope)
        {
            draft = Draft::default();
        }
        if !draft.source_id.is_empty()
            && !next
                .grants
                .iter()
                .any(|g| g.id == draft.source_id && g.can_delegate)
        {
            draft.source_id.clear();
            draft.delegate_email.clear();
            draft.delegate_actions.clear();
        }
        let awaiting_revision = next.organization.as_ref().is_some_and(|org| {
            self.authority.with_untracked(|attempts| {
                attempts.get(&org.id).is_some_and(|attempt| {
                    matches!(&attempt.command,
                        AccessCommand::ReviseRequest { request_id, .. } if request_id == &draft.request_id)
                })
            })
        });
        if !awaiting_revision
            && !draft.request_id.is_empty()
            && !next
                .requests
                .iter()
                .any(|r| r.id == draft.request_id && r.can_revise)
        {
            draft.request_id.clear();
            draft.request_title.clear();
            draft.request_conflicts.clear();
        }
        if draft.grant_from.is_empty() {
            draft.grant_from = format::current_local_time();
        }
        self.draft.set(draft);
        self.state.set(Some(response.state));
        self.csrf.set(response.csrf);
        self.message.set(response.message);
        self.error.set(None);
        self.ready.set(true);
        self.validating.set(false);
    }
    fn failed(self, error: AuthFailure) {
        if !matches!(
            error.code.as_str(),
            "invalid_input"
                | "last_responsible"
                | "last_responsible_required"
                | "conflict_of_interest"
                | "insufficient_authority"
                | "approval_conflict"
                | "idempotency_conflict"
        ) {
            self.hide();
        }
        self.busy.set(false);
        self.error.set(Some(error));
    }
    pub fn change_scope(self, id: String) {
        if self.blocked() {
            return;
        }
        self.draft.set(Draft {
            selected_scope: id,
            grant_from: format::current_local_time(),
            ..Draft::default()
        });
        self.load_policy();
    }
    pub fn request_scope(self, id: String) {
        if self.blocked() {
            return;
        }
        if self.dirty() {
            self.confirm_transition(
                DraftTransition::Scope(id),
                "Trocar a obra, o contrato ou o orçamento?",
                "Os campos ainda não enviados desta página serão limpos. Nenhuma alteração será gravada.",
                "perm-scope-selection",
            );
        } else {
            self.change_scope(id);
        }
    }
    pub fn change_policy(self, action: AccessAction) {
        if self.blocked() || self.draft.get_untracked().policy_action == action {
            return;
        }
        if self.draft.get_untracked().dirty_forms & 8 != 0 {
            // The browser changed the select before on:change. A binding with
            // the same Rust value need not write it again, so restore the DOM.
            super::select_value(
                "perm-policy-action",
                self.draft.get_untracked().policy_action.as_str(),
            );
            self.confirm_transition(
                DraftTransition::Policy(action),
                "Descartar a edição desta regra?",
                "O limiar e as opções ainda não enviados desta regra serão descartados. A regra da decisão escolhida será carregada. Os demais formulários serão preservados.",
                "perm-policy-action",
            );
        } else {
            self.select_policy(action);
        }
    }
    fn select_policy(self, action: AccessAction) {
        self.draft.update(|d| {
            d.policy_action = action;
            d.dirty_forms &= !8;
            d.changed = d.dirty_forms != 0;
        });
        self.load_policy();
    }
    pub fn load_policy(self) {
        let draft = self.draft.get_untracked();
        let policy = self.state.get_untracked().and_then(|s| {
            s.policies
                .into_iter()
                .find(|p| p.scope_id == draft.selected_scope && p.action == draft.policy_action)
        });
        self.draft.update(|d| {
            if let Some(p) = policy {
                d.policy_threshold =
                    canteiro_contracts::forms::format_decimal_pt_br(&p.threshold.to_string());
                d.policy_double = p.approvals_above_threshold == 2;
                d.policy_segregate = p.segregate_proposer;
                d.policy_version = Some(p.version);
            } else {
                d.policy_threshold = "0,00".into();
                d.policy_double = true;
                d.policy_segregate = true;
                d.policy_version = None;
            }
        });
    }
    fn amount(self, value: &str, field: &'static str) -> Option<Money> {
        match format::money_input(value).and_then(|v| v.parse::<Money>().map_err(|e| e.to_string()))
        {
            Ok(v) => Some(v),
            Err(e) => {
                self.local_error(e, field);
                None
            }
        }
    }
    fn date(self, value: &str, field: &'static str) -> Option<String> {
        match format::local_instant(value) {
            Ok(v) => Some(v),
            Err(e) => {
                self.local_error(e, field);
                None
            }
        }
    }
    pub fn create_scope(self) {
        let Some(context) = self.context() else {
            return;
        };
        let d = self.draft.get_untracked();
        if d.scope_name.trim().is_empty() {
            self.local_error(
                "Informe o nome da obra, do contrato ou do orçamento.",
                "perm-scope-name",
            );
            return;
        }
        if matches!(d.scope_kind, ScopeKind::Contract | ScopeKind::Budget)
            && d.scope_parent.is_empty()
        {
            self.local_error(
                if d.scope_kind == ScopeKind::Contract {
                    "Selecione a obra deste contrato."
                } else {
                    "Selecione o contrato deste orçamento."
                },
                "perm-scope-parent",
            );
            return;
        }
        self.submit(AccessCommand::CreateScope {
            context,
            kind: d.scope_kind,
            name: d.scope_name,
            parent_id: if matches!(d.scope_kind, ScopeKind::Contract | ScopeKind::Budget) {
                Some(d.scope_parent)
            } else {
                None
            },
        });
    }
    pub fn grant(self) {
        let Some(context) = self.context() else {
            return;
        };
        let d = self.draft.get_untracked();
        let Some(max_amount) = self.amount(&d.grant_limit, "perm-grant-limit") else {
            return;
        };
        let Some(valid_from) = self.date(&d.grant_from, "perm-grant-from") else {
            return;
        };
        let valid_until = if d.grant_until.trim().is_empty() {
            None
        } else {
            let Some(v) = self.date(&d.grant_until, "perm-grant-until") else {
                return;
            };
            Some(v)
        };
        if d.grant_actions.is_empty() {
            self.local_error(
                "Selecione pelo menos uma tarefa permitida.",
                "perm-grant-actions",
            );
            return;
        }
        let description = format!(
            "{} receberá a função {} em {}. Limite por ato: {}. Tarefas: {}. Início: {}. Término: {}.",
            d.grant_email,
            d.grant_role.label(),
            self.scope_name(),
            format::money(&max_amount.to_string()),
            d.grant_actions
                .iter()
                .map(|a| a.label())
                .collect::<Vec<_>>()
                .join(", "),
            format::instant(&valid_from),
            valid_until
                .as_deref()
                .map(format::instant)
                .unwrap_or_else(|| "sem vencimento".into())
        );
        self.confirm(
            AccessCommand::Grant {
                context,
                scope_id: d.selected_scope,
                user_email: d.grant_email,
                role: d.grant_role,
                actions: d.grant_actions,
                max_amount,
                valid_from,
                valid_until,
                required: d.grant_required,
            },
            "Conceder este acesso?",
            &description,
            "perm-grant-submit",
        );
    }
    pub fn begin_delegation(self, grant: GrantView) {
        if self.blocked() {
            return;
        }
        let draft = self.draft.get_untracked();
        if draft.source_id == grant.id && draft.source_version == grant.version {
            Self::focus_delegation();
            return;
        }
        if draft.dirty_forms & 4 != 0 {
            let return_focus = format!("perm-grant-delegate-{}", grant.id);
            self.confirm_transition(
                DraftTransition::Delegation(Box::new(grant)),
                "Descartar esta delegação em edição?",
                "Os campos ainda não enviados da delegação serão descartados para usar o outro vínculo. Os demais formulários serão preservados. Nenhuma delegação será gravada nesta troca.",
                &return_focus,
            );
        } else {
            self.start_delegation(grant);
        }
    }
    fn start_delegation(self, grant: GrantView) {
        self.draft.update(|d| {
            d.source_id = grant.id;
            d.source_version = grant.version;
            d.delegate_email.clear();
            d.delegate_reason.clear();
            d.delegate_actions = grant.actions;
            d.delegate_limit =
                canteiro_contracts::forms::format_decimal_pt_br(&grant.max_amount.to_string());
            d.delegate_from = format::delegation_start(&grant.valid_from);
            d.delegate_until = grant
                .valid_until
                .as_deref()
                .map(format::local_date_time)
                .unwrap_or_default();
            d.substitution = false;
            d.dirty_forms |= 4;
            d.changed = true;
        });
        Self::focus_delegation();
    }
    fn focus_delegation() {
        #[cfg(feature = "hydrate")]
        request_animation_frame(|| {
            super::open_details("perm-delegate-panel");
            focus("perm-delegate-email");
        });
    }
    pub fn delegate(self) {
        let Some(context) = self.context() else {
            return;
        };
        let d = self.draft.get_untracked();
        let Some(max_amount) = self.amount(&d.delegate_limit, "perm-delegate-limit") else {
            return;
        };
        let Some(valid_from) = self.date(&d.delegate_from, "perm-delegate-from") else {
            return;
        };
        let Some(valid_until) = self.date(&d.delegate_until, "perm-delegate-until") else {
            return;
        };
        if !(2..=200).contains(&d.delegate_reason.trim().chars().count()) {
            self.local_error(
                "Explique o motivo com 2 a 200 caracteres.",
                "perm-delegate-reason",
            );
            return;
        }
        let description = format!(
            "{} receberá acesso temporário em {}, até {} por ato, de {} até {}. Tarefas: {}. Motivo: {}. O acesso depende do vínculo de origem e não forma uma aprovação independente do titular.",
            d.delegate_email,
            self.scope_name(),
            format::money(&max_amount.to_string()),
            format::instant(&valid_from),
            format::instant(&valid_until),
            d.delegate_actions
                .iter()
                .map(|a| a.label())
                .collect::<Vec<_>>()
                .join(", "),
            d.delegate_reason
        );
        self.confirm(
            AccessCommand::Delegate {
                context,
                source_grant_id: d.source_id,
                source_version: d.source_version,
                user_email: d.delegate_email,
                actions: d.delegate_actions,
                max_amount,
                valid_from,
                valid_until,
                substitution: d.substitution,
                reason: d.delegate_reason,
            },
            if d.substitution {
                "Confirmar substituição temporária?"
            } else {
                "Confirmar delegação temporária?"
            },
            &description,
            "perm-delegate-submit",
        );
    }
    pub fn revoke(self, grant: GrantView) {
        let Some(context) = self.context() else {
            return;
        };
        let description = format!(
            "Encerrar o vínculo de {} ({}) em {}. As delegações dependentes também perderão validade. Se houver responsabilidade obrigatória, mantenha outro titular para os mesmos atos e valor.",
            grant.user_email,
            grant.role.label(),
            self.scope_name()
        );
        self.confirm(
            AccessCommand::RevokeGrant {
                context,
                grant_id: grant.id,
                grant_version: grant.version,
            },
            "Encerrar este vínculo?",
            &description,
            "perm-scope-selection",
        );
    }
    pub fn policy(self) {
        let Some(context) = self.context() else {
            return;
        };
        let d = self.draft.get_untracked();
        let Some(threshold) = self.amount(&d.policy_threshold, "perm-policy-threshold") else {
            return;
        };
        let description = format!(
            "{} em {}: {} aprovação(ões) a partir de {}. {} Pessoas com conflito declarado continuam impedidas. Pedidos pendentes precisarão de revisão se a regra mudar.",
            d.policy_action.label(),
            self.scope_name(),
            if d.policy_double { 2 } else { 1 },
            format::money(&threshold.to_string()),
            if d.policy_segregate {
                "O proponente não poderá aprovar."
            } else {
                "O proponente poderá participar se tiver alçada."
            }
        );
        self.confirm(
            AccessCommand::SetPolicy {
                context,
                scope_id: d.selected_scope,
                action: d.policy_action,
                threshold,
                approvals_above_threshold: if d.policy_double { 2 } else { 1 },
                segregate_proposer: d.policy_segregate,
                expected_version: d.policy_version,
            },
            "Salvar a regra de aprovação?",
            &description,
            "perm-policy-submit",
        );
    }
    pub fn propose(self) {
        let Some(context) = self.context() else {
            return;
        };
        let d = self.draft.get_untracked();
        let Some(increase) = self.amount(&d.request_increase, "perm-request-increase") else {
            return;
        };
        let Some(decrease) = self.amount(&d.request_decrease, "perm-request-decrease") else {
            return;
        };
        if d.request_title.trim().is_empty() {
            self.local_error("Descreva o pedido que será decidido.", "perm-request-title");
            return;
        }
        let description = format!(
            "{}: {} em {}. Acréscimo ou autorização: {}. Redução: {}. Valor da alçada: {}. Este pedido não executa serviços nem pagamentos.",
            d.request_action.label(),
            d.request_title,
            self.scope_name(),
            format::money(&increase.to_string()),
            format::money(&decrease.to_string()),
            format::money(&increase.max(decrease).to_string())
        );
        let command = if d.request_id.is_empty() {
            AccessCommand::Propose {
                context,
                scope_id: d.selected_scope,
                action: d.request_action,
                title: d.request_title,
                increase,
                decrease,
                conflicted_user_ids: d.request_conflicts,
            }
        } else {
            AccessCommand::ReviseRequest {
                context,
                request_id: d.request_id,
                request_version: d.request_version,
                title: d.request_title,
                increase,
                decrease,
                conflicted_user_ids: d.request_conflicts,
            }
        };
        self.confirm(
            command,
            "Enviar para decisão?",
            &description,
            "perm-request-submit",
        );
    }
    pub fn begin_revision(self, request: RequestView) {
        if self.blocked() {
            return;
        }
        let draft = self.draft.get_untracked();
        if draft.request_id == request.id && draft.request_version == request.version {
            Self::focus_revision();
            return;
        }
        if draft.dirty_forms & 16 != 0 {
            let return_focus = format!("perm-revise-{}", request.id);
            self.confirm_transition(
                DraftTransition::Revision(Box::new(request)),
                "Descartar este pedido em edição?",
                "Os campos ainda não enviados deste pedido serão descartados para revisar o outro pedido. Os demais formulários serão preservados. Nenhuma revisão será gravada nesta troca.",
                &return_focus,
            );
        } else {
            self.start_revision(request);
        }
    }
    fn start_revision(self, request: RequestView) {
        self.draft.update(|d| {
            d.request_id = request.id;
            d.request_version = request.version;
            d.request_action = request.action;
            d.request_title = request.title;
            d.request_increase =
                canteiro_contracts::forms::format_decimal_pt_br(&request.increase.to_string());
            d.request_decrease =
                canteiro_contracts::forms::format_decimal_pt_br(&request.decrease.to_string());
            d.request_conflicts = request.conflicted_user_ids;
            d.dirty_forms |= 16;
            d.changed = true;
        });
        Self::focus_revision();
    }
    fn focus_revision() {
        #[cfg(feature = "hydrate")]
        request_animation_frame(|| {
            super::open_details("perm-request-panel");
            focus("perm-request-title");
        });
    }
    pub fn decide(self, request: RequestView, grant_id: String, approve: bool) {
        let Some(context) = self.context() else {
            return;
        };
        let Some(grant) = self
            .state
            .get_untracked()
            .and_then(|s| s.grants.into_iter().find(|g| g.id == grant_id))
        else {
            return;
        };
        let description = format!(
            "{} · versão {}. Acréscimo ou autorização: {}. Redução: {}. Valor da alçada: {}. Você decidirá como {}, com limite de {}. A decisão não executa serviços nem pagamentos.",
            request.title,
            request.version,
            format::money(&request.increase.to_string()),
            format::money(&request.decrease.to_string()),
            format::money(&request.authority_amount.to_string()),
            grant.role.label(),
            format::money(&grant.max_amount.to_string())
        );
        self.confirm(
            AccessCommand::Decide {
                context,
                request_id: request.id,
                request_version: request.version,
                grant_id,
                grant_version: grant.version,
                approve,
            },
            if approve {
                "Aprovar esta versão?"
            } else {
                "Recusar esta versão?"
            },
            &description,
            "perm-requests-heading",
        );
    }
    pub fn confirm(
        self,
        command: AccessCommand,
        title: &str,
        description: &str,
        return_focus: &str,
    ) {
        if self.blocked() {
            return;
        }
        self.pending.set(Some(Confirmation {
            command: Some(command),
            transition: None,
            title: title.into(),
            description: description.into(),
            return_focus: return_focus.into(),
        }));
        self.show_confirmation();
    }
    fn confirm_transition(
        self,
        transition: DraftTransition,
        title: &str,
        description: &str,
        return_focus: &str,
    ) {
        self.pending.set(Some(Confirmation {
            command: None,
            transition: Some(transition),
            title: title.into(),
            description: description.into(),
            return_focus: return_focus.into(),
        }));
        self.show_confirmation();
    }
    fn show_confirmation(self) {
        #[cfg(feature = "hydrate")]
        {
            let generation = self.generation.get_untracked();
            let active = self.active.get_value();
            request_animation_frame(move || {
                if active.load(Ordering::Acquire)
                    && self.generation.get_untracked() == generation
                    && self.pending.get_untracked().is_some()
                {
                    super::open_dialog();
                }
            });
        }
    }
    pub fn dismiss(self) {
        let previous = self.pending.get_untracked();
        close_dialog();
        self.pending.set(None);
        if !self.validating.get_untracked() {
            focus(
                previous
                    .as_ref()
                    .map_or("perm-refresh", |p| &p.return_focus),
            );
        }
    }
    pub fn confirm_pending(self) {
        if self.blocked() {
            return;
        }
        let Some(p) = self.pending.get_untracked() else {
            return;
        };
        close_dialog();
        self.pending.set(None);
        if let Some(command) = p.command {
            self.submit(command);
        } else if let Some(transition) = p.transition {
            match transition {
                DraftTransition::Scope(scope) => {
                    self.change_scope(scope);
                    focus("perm-scope-selection");
                }
                DraftTransition::Policy(action) => {
                    self.select_policy(action);
                    focus("perm-policy-action");
                }
                DraftTransition::Delegation(grant) => self.start_delegation(*grant),
                DraftTransition::Revision(request) => self.start_revision(*request),
            }
        }
    }
    pub fn authority_pending(self) -> bool {
        self.authority_attempt().is_some()
    }

    pub fn authority_checked(self) -> bool {
        self.authority_attempt()
            .is_some_and(|attempt| attempt.absence_checked)
    }

    fn authority_attempt(self) -> Option<PendingAuthority> {
        self.state.with(|state| {
            let state = state.as_ref()?;
            let company = &state.organization.as_ref()?.id;
            self.authority.with(|attempts| {
                attempts
                    .get(company)
                    .filter(|attempt| attempt.matches(state))
                    .cloned()
            })
        })
    }

    pub fn retry_authority(self) {
        if self.blocked() {
            return;
        }
        let Some((attempt, scope)) = self.authority_attempt().zip(self.context()) else {
            return;
        };
        if attempt.absence_checked {
            self.send(attempt.retry(scope.context_version), Some(attempt.key));
            return;
        }
        self.busy.set(true);
        self.error.set(None);
        self.message.set(String::new());
        let generation = self.generation.get_untracked();
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let act = attempt.command.authority_act().expect("recibo de alçada");
            let result = network::authority_result(&scope, act, &attempt.key).await;
            if !active.load(Ordering::Acquire) || self.generation.get_untracked() != generation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(Some(response)) => {
                    self.complete(&attempt.command, response);
                    self.clear_authority(&scope.organization_id);
                }
                Ok(None) => {
                    self.authority.update(|attempts| {
                        if let Some(attempt) = attempts.get_mut(&scope.organization_id) {
                            attempt.absence_checked = true;
                        }
                    });
                    self.message.set("A consulta não encontrou confirmação. Você pode reenviar os mesmos dados com segurança.".into());
                }
                Err(error) => {
                    if definitive_authority_error(&error) {
                        self.clear_authority(&scope.organization_id);
                    }
                    self.failed(error);
                }
            }
            self.focus_feedback(active, generation);
        });
    }

    pub fn submit(self, command: AccessCommand) {
        if self.blocked() {
            return;
        }
        let key = if command.authority_act().is_some() {
            if self.authority_pending() {
                self.local_error(
                    "Confira o resultado da operação anterior para iniciar outra decisão.",
                    "perm-authority-retry",
                );
                return;
            }
            let Some(state) = self.state.get_untracked() else {
                return;
            };
            let key = match crate::http::operation_id() {
                Ok(key) => key,
                Err(error) => {
                    self.failed(error);
                    return;
                }
            };
            self.authority.update(|attempts| {
                attempts.insert(
                    command.context().organization_id.clone(),
                    PendingAuthority {
                        user_id: state.current_user_id,
                        command: command.clone(),
                        key: key.clone(),
                        absence_checked: false,
                    },
                );
            });
            Some(key)
        } else {
            None
        };
        self.send(command, key);
    }

    fn send(self, command: AccessCommand, key: Option<String>) {
        if key.is_some() {
            self.authority.update(|attempts| {
                if let Some(attempt) = attempts.get_mut(&command.context().organization_id) {
                    attempt.absence_checked = false;
                }
            });
        }
        self.busy.set(true);
        self.mutation.set(true);
        self.error.set(None);
        self.message.set(String::new());
        self.generation.update(|g| *g = g.wrapping_add(1));
        let generation = self.generation.get_untracked();
        let csrf = self.csrf.get_untracked();
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::command(&command, &csrf, key.as_deref()).await;
            if !active.load(Ordering::Acquire) {
                return;
            }
            self.mutation.set(false);
            if self.refresh_after_mutation.get_untracked() {
                self.refresh_after_mutation.set(false);
                self.busy.set(false);
                self.load(true);
                return;
            }
            if self.generation.get_untracked() != generation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(r) => {
                    self.complete(&command, r);
                    if key.is_some() {
                        self.clear_authority(&command.context().organization_id);
                    }
                }
                Err(e) => {
                    if key.is_some() && definitive_authority_error(&e) {
                        self.clear_authority(&command.context().organization_id);
                    }
                    self.failed(e);
                }
            }
            self.focus_feedback(active, generation);
        });
    }

    fn clear_authority(self, organization: &str) {
        self.authority.update(|attempts| {
            attempts.remove(organization);
        });
    }

    fn focus_feedback(self, active: Arc<AtomicBool>, generation: u64) {
        request_animation_frame(move || {
            if active.load(Ordering::Acquire) && self.generation.get_untracked() == generation {
                focus("perm-feedback");
            }
        });
    }

    fn complete(self, command: &AccessCommand, response: AccessResponse) {
        let clear_proposal = authority_draft_matches(command, &self.draft.get_untracked());
        self.accept(response);
        self.draft.update(|draft| {
            match command {
                AccessCommand::CreateScope { .. } => {
                    draft.scope_name.clear();
                    draft.dirty_forms &= !1;
                }
                AccessCommand::Grant { .. } => {
                    draft.grant_email.clear();
                    draft.dirty_forms &= !2;
                }
                AccessCommand::Delegate { .. } => {
                    draft.delegate_email.clear();
                    draft.delegate_reason.clear();
                    draft.source_id.clear();
                    draft.dirty_forms &= !4;
                }
                AccessCommand::Propose { .. } | AccessCommand::ReviseRequest { .. }
                    if clear_proposal =>
                {
                    draft.request_title.clear();
                    draft.request_id.clear();
                    draft.request_conflicts.clear();
                    draft.dirty_forms &= !16;
                }
                AccessCommand::SetPolicy { .. } => draft.dirty_forms &= !8,
                _ => {}
            }
            draft.changed = draft.dirty_forms != 0;
        });
        if matches!(command, AccessCommand::SetPolicy { .. }) {
            self.load_policy();
        }
    }
}

fn authority_draft_matches(command: &AccessCommand, draft: &Draft) -> bool {
    let (title, increase, decrease, people) = match command {
        AccessCommand::Propose {
            scope_id,
            action,
            title,
            increase,
            decrease,
            conflicted_user_ids,
            ..
        } if *scope_id == draft.selected_scope
            && *action == draft.request_action
            && draft.request_id.is_empty() =>
        {
            (title, increase, decrease, conflicted_user_ids)
        }
        AccessCommand::ReviseRequest {
            request_id,
            request_version,
            title,
            increase,
            decrease,
            conflicted_user_ids,
            ..
        } if *request_id == draft.request_id && *request_version == draft.request_version => {
            (title, increase, decrease, conflicted_user_ids)
        }
        _ => return false,
    };
    let amount = |value| format::money_input(value).ok()?.parse::<Money>().ok();
    title.trim() == draft.request_title.trim()
        && amount(&draft.request_increase) == Some(*increase)
        && amount(&draft.request_decrease) == Some(*decrease)
        && *people == draft.request_conflicts
}

fn definitive_authority_error(error: &AuthFailure) -> bool {
    !matches!(
        error.code.as_str(),
        "network_unavailable"
            | "unavailable"
            | "session_expired"
            | "context_changed"
            | "csrf_invalid"
            | "rate_limited"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use canteiro_contracts::organizations::{OrganizationRole, OrganizationSummary};
    fn response(user: &str, company: &str, manage: bool) -> AccessResponse {
        AccessResponse {
            state: AccessState {
                current_user_id: user.into(),
                organization: Some(OrganizationSummary {
                    id: company.into(),
                    name: "Empresa fictícia".into(),
                    role: if manage {
                        OrganizationRole::Administrator
                    } else {
                        OrganizationRole::Member
                    },
                    version: 1,
                    valid_until: None,
                }),
                context_version: 1,
                mfa_verified: manage,
                can_manage: manage,
                scopes: vec![ScopeView {
                    id: "obra-a".into(),
                    kind: ScopeKind::Work,
                    name: "Obra fictícia".into(),
                    parent_id: None,
                    version: 1,
                }],
                members: vec![],
                grants: vec![],
                policies: vec![],
                requests: vec![],
            },
            csrf: "fixture-csrf".into(),
            message: String::new(),
            receipt: None,
        }
    }
    fn grant(id: &str) -> GrantView {
        GrantView {
            id: id.into(),
            scope_id: "obra-a".into(),
            user_id: "pessoa-a".into(),
            user_email: "pessoa-a@example.invalid".into(),
            role: AccessRole::Approver,
            actions: vec![AccessAction::View, AccessAction::ApproveChange],
            max_amount: "100.00".parse().unwrap(),
            valid_from: "2020-01-01T00:00:00Z".into(),
            valid_until: Some("2090-01-01T00:00:00Z".into()),
            effective: true,
            revoked: false,
            source_grant_id: None,
            substitution: false,
            reason: String::new(),
            required: false,
            version: 3,
            can_delegate: true,
            can_revoke: true,
        }
    }
    fn request(id: &str) -> RequestView {
        RequestView {
            id: id.into(),
            scope_id: "obra-a".into(),
            title: format!("Pedido {id}"),
            action: AccessAction::ApproveChange,
            proposer_id: "pessoa-a".into(),
            proposer_email: "pessoa-a@example.invalid".into(),
            conflicted_user_ids: vec![],
            increase: "10.00".parse().unwrap(),
            decrease: "0.00".parse().unwrap(),
            authority_amount: "10.00".parse().unwrap(),
            contains_increase: true,
            status: "pending".into(),
            version: 3,
            policy_version: 2,
            policy_changed: false,
            required_approvals: 2,
            valid_approvals: 0,
            approvals: vec![],
            allowed_grant_ids: vec![],
            can_revise: true,
            denial: None,
        }
    }
    #[test]
    fn dirty_policy_change_requires_a_decision_and_keeps_other_drafts() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.edit(|d| {
                d.policy_threshold = "500,00".into();
                d.policy_version = Some(3);
                d.grant_email = "outro-rascunho@example.invalid".into();
            });
            a.change_policy(AccessAction::AuthorizePayment);
            assert!(a.pending.get_untracked().is_some());
            assert_eq!(
                a.draft.get_untracked().policy_action,
                AccessAction::ApproveChange
            );
            a.dismiss();
            assert_eq!(a.draft.get_untracked().policy_threshold, "500,00");
            assert_eq!(a.draft.get_untracked().policy_version, Some(3));
            a.change_policy(AccessAction::AuthorizePayment);
            a.confirm_pending();
            let draft = a.draft.get_untracked();
            assert_eq!(draft.policy_action, AccessAction::AuthorizePayment);
            assert_eq!(draft.policy_threshold, "0,00");
            assert_eq!(draft.policy_version, None);
            assert_eq!(draft.grant_email, "outro-rascunho@example.invalid");
            assert_eq!(draft.dirty_forms, 2);
            assert!(draft.changed);
        });
    }
    #[test]
    fn switching_delegation_keeps_the_draft_until_discard_is_confirmed() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.begin_delegation(grant("fonte-a"));
            a.edit(|d| {
                d.delegate_email = "destino@example.invalid".into();
                d.delegate_reason = "Cobertura de férias".into();
                d.grant_email = "outro-rascunho@example.invalid".into();
            });
            a.begin_delegation(grant("fonte-a"));
            assert!(a.pending.get_untracked().is_none());
            assert_eq!(
                a.draft.get_untracked().delegate_reason,
                "Cobertura de férias"
            );
            a.begin_delegation(grant("fonte-b"));
            assert!(a.pending.get_untracked().is_some());
            a.dismiss();
            assert_eq!(a.draft.get_untracked().source_id, "fonte-a");
            assert_eq!(
                a.draft.get_untracked().delegate_email,
                "destino@example.invalid"
            );
            a.begin_delegation(grant("fonte-b"));
            a.confirm_pending();
            let draft = a.draft.get_untracked();
            assert_eq!(draft.source_id, "fonte-b");
            assert_eq!(draft.source_version, 3);
            assert!(draft.delegate_email.is_empty() && draft.delegate_reason.is_empty());
            assert_eq!(draft.grant_email, "outro-rascunho@example.invalid");
            assert_eq!(draft.dirty_forms, 2 | 4);
        });
    }
    #[test]
    fn switching_revision_keeps_values_and_version_until_discard_is_confirmed() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.begin_revision(request("pedido-a"));
            a.edit(|d| {
                d.request_title = "Revisão ainda não enviada".into();
                d.request_increase = "125,00".into();
                d.request_conflicts = vec!["pessoa-b".into()];
                d.grant_email = "outro-rascunho@example.invalid".into();
            });
            a.begin_revision(request("pedido-a"));
            assert!(a.pending.get_untracked().is_none());
            a.begin_revision(request("pedido-b"));
            assert!(a.pending.get_untracked().is_some());
            a.dismiss();
            assert_eq!(
                a.draft.get_untracked().request_title,
                "Revisão ainda não enviada"
            );
            assert_eq!(a.draft.get_untracked().request_conflicts, vec!["pessoa-b"]);
            assert_eq!(a.draft.get_untracked().request_version, 3);
            a.begin_revision(request("pedido-b"));
            a.confirm_pending();
            let draft = a.draft.get_untracked();
            assert_eq!(draft.request_id, "pedido-b");
            assert_eq!(draft.request_title, "Pedido pedido-b");
            assert_eq!(draft.request_increase, "10,00");
            assert!(draft.request_conflicts.is_empty());
            assert_eq!(draft.grant_email, "outro-rascunho@example.invalid");
            assert_eq!(draft.dirty_forms, 2 | 16);
        });
    }
    #[test]
    fn route_unmount_preserves_only_a_bound_draft_until_authenticated_return() {
        for (user, company, manage, restore) in [
            ("pessoa-a", "empresa-a", true, true),
            ("pessoa-b", "empresa-a", true, false),
            ("pessoa-a", "empresa-b", true, false),
            ("pessoa-a", "empresa-a", false, false),
        ] {
            Owner::new().with(|| {
                provide_draft_store();
                let page = Owner::new();
                let old_active = page.with(|| {
                    let a = Access::new();
                    a.accept(response("pessoa-a", "empresa-a", true));
                    a.draft.update(|d| {
                        d.selected_scope = "obra-a".into();
                        d.policy_version = Some(3);
                    });
                    a.edit(|d| {
                        d.grant_email = "privado@example.invalid".into();
                        d.policy_threshold = "50,00".into();
                    });
                    // Leaving while a command is in flight must not keep an
                    // active callback or replay it in the next page instance.
                    a.mutation.set(true);
                    a.active.get_value()
                });
                page.cleanup();
                assert!(!old_active.load(Ordering::Acquire));
                let next_page = Owner::new();
                next_page.with(|| {
                    let a = Access::new();
                    assert!(a.state.get_untracked().is_none());
                    assert!(a.csrf.get_untracked().is_empty());
                    assert!(a.draft.get_untracked().grant_email.is_empty());
                    assert!(a.blocked());
                    assert!(!a.mutation.get_untracked());
                    assert!(a.suspended.get_untracked().is_some());
                    let mut fresh = response(user, company, manage);
                    fresh.state.policies.push(PolicyView {
                        scope_id: "obra-a".into(),
                        action: AccessAction::ApproveChange,
                        threshold: "80.00".parse().unwrap(),
                        approvals_above_threshold: 2,
                        segregate_proposer: true,
                        version: 4,
                    });
                    a.accept(fresh);
                    let draft = a.draft.get_untracked();
                    assert_eq!(draft.grant_email == "privado@example.invalid", restore);
                    assert_eq!(draft.policy_version, if restore { Some(3) } else { None });
                    assert!(a.suspended.get_untracked().is_none());
                });
                next_page.cleanup();
            });
        }
        // A different App/SSR owner receives a separate, empty store.
        Owner::new().with(|| {
            provide_draft_store();
            assert!(Access::new().suspended.get_untracked().is_none());
        });
    }
    #[test]
    fn network_failure_hides_private_data_but_resumes_same_identity_draft() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.draft.update(|d| d.selected_scope = "obra-a".into());
            a.edit(|d| d.grant_email = "rascunho@example.invalid".into());
            a.failed(AuthFailure {
                code: "network_unavailable".into(),
                message: "Falha fictícia".into(),
            });
            assert!(!a.ready.get_untracked());
            assert!(a.state.get_untracked().is_none());
            assert!(a.draft.get_untracked().grant_email.is_empty());
            a.accept(response("pessoa-a", "empresa-a", true));
            assert_eq!(
                a.draft.get_untracked().grant_email,
                "rascunho@example.invalid"
            );
            assert!(a.dirty());
        });
    }
    #[test]
    fn another_identity_company_or_lost_admin_cannot_restore_draft() {
        for (user, company, manage) in [
            ("pessoa-b", "empresa-a", true),
            ("pessoa-a", "empresa-b", true),
            ("pessoa-a", "empresa-a", false),
        ] {
            Owner::new().with(|| {
                let a = Access::new();
                a.accept(response("pessoa-a", "empresa-a", true));
                a.edit(|d| d.grant_email = "privado@example.invalid".into());
                a.hide();
                a.accept(response(user, company, manage));
                assert!(a.draft.get_untracked().grant_email.is_empty());
                assert!(!a.dirty());
            });
        }
    }
    #[test]
    fn focus_refresh_supersedes_pending_mutation_before_fetching_identity() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.edit(|d| d.request_title = "Pedido privado".into());
            let old = a.generation.get_untracked();
            a.mutation.set(true);
            a.load(false);
            assert!(a.refresh_after_mutation.get_untracked());
            assert_ne!(old, a.generation.get_untracked());
            assert!(a.state.get_untracked().is_none());
            assert!(a.blocked());
            assert!(a.mutation.get_untracked());
        });
    }

    #[test]
    fn authority_attempt_recovers_only_in_its_company_and_does_not_clear_new_edits() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            let command = AccessCommand::Propose {
                context: a.context().unwrap(),
                scope_id: "obra-a".into(),
                action: AccessAction::ApproveChange,
                title: "Pedido enviado".into(),
                increase: "10.00".parse().unwrap(),
                decrease: "0.00".parse().unwrap(),
                conflicted_user_ids: vec![],
            };
            a.authority.update(|pending| {
                pending.insert(
                    "empresa-a".into(),
                    PendingAuthority {
                        user_id: "pessoa-a".into(),
                        command: command.clone(),
                        key: "chave-original".into(),
                        absence_checked: true,
                    },
                );
            });
            a.accept(response("pessoa-a", "empresa-b", true));
            assert!(!a.authority_pending());
            a.accept(response("pessoa-a", "empresa-a", true));
            assert!(a.authority_pending());
            assert!(!a.authority_checked());
            let attempt = a.authority_attempt().unwrap();
            assert_eq!(attempt.key, "chave-original");
            assert_eq!(attempt.retry(3).context().context_version, 3);
            assert_eq!(attempt.retry(3).context().organization_version, 1);
            a.draft
                .update(|draft| draft.selected_scope = "obra-a".into());
            a.edit(|draft| {
                draft.request_increase = "10,00".into();
                draft.request_title = "Novo rascunho em edição".into();
            });
            a.complete(&command, response("pessoa-a", "empresa-a", true));
            assert_eq!(a.draft.get_untracked().selected_scope, "obra-a");
            assert_eq!(
                a.draft.get_untracked().request_title,
                "Novo rascunho em edição"
            );
            a.accept(response("pessoa-b", "empresa-a", true));
            assert!(!a.authority_pending());
            assert!(!a.dirty());
        });
    }

    #[test]
    fn refreshing_does_not_upgrade_saved_policy_version() {
        Owner::new().with(|| {
            let a = Access::new();
            a.accept(response("pessoa-a", "empresa-a", true));
            a.draft.update(|d| {
                d.selected_scope = "obra-a".into();
                d.policy_version = Some(3);
            });
            a.edit(|d| d.policy_threshold = "50,00".into());
            a.hide();
            let mut newer = response("pessoa-a", "empresa-a", true);
            newer.state.policies.push(PolicyView {
                scope_id: "obra-a".into(),
                action: AccessAction::ApproveChange,
                threshold: "80.00".parse().unwrap(),
                approvals_above_threshold: 2,
                segregate_proposer: true,
                version: 4,
            });
            a.accept(newer);
            assert_eq!(a.draft.get_untracked().policy_version, Some(3));
            assert_eq!(a.draft.get_untracked().policy_threshold, "50,00");
        });
    }
}
