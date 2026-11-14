use super::{close_dialog, focus, network, open_dialog};
use canteiro_contracts::{auth::AuthFailure, organizations::*};
use leptos::prelude::*;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub(super) struct Confirmation {
    pub command: OrganizationCommand,
    pub title: String,
    pub description: String,
    pub return_focus: String,
}

pub(super) type MemberDrafts = BTreeMap<(String, String, i64), (OrganizationRole, String)>;

/// Non-secret work is isolated by verified identity and organization. It stays
/// hidden while the account or organization is unavailable and is never put in
/// browser storage, navigation state or the initial HTML.
#[derive(Clone)]
pub(super) struct SuspendedDraft {
    user_id: String,
    organization_id: Option<String>,
    create_name: String,
    rename_name: String,
    invite_email: String,
    invite_role: OrganizationRole,
    invite_hours: String,
    invite_until: String,
    members: MemberDrafts,
}

#[derive(Clone, Copy)]
pub(super) struct Organizations {
    pub state: RwSignal<OrganizationState>,
    pub csrf: RwSignal<String>,
    pub ready: RwSignal<bool>,
    pub validating: RwSignal<bool>,
    pub busy: RwSignal<bool>,
    pub search_busy: RwSignal<bool>,
    pub error: RwSignal<Option<AuthFailure>>,
    pub message: RwSignal<String>,
    pub create_name: RwSignal<String>,
    pub rename_name: RwSignal<String>,
    pub invite_email: RwSignal<String>,
    pub invite_role: RwSignal<OrganizationRole>,
    pub invite_hours: RwSignal<String>,
    pub invite_until: RwSignal<String>,
    pub invitation_token: RwSignal<String>,
    pub invitation_preview: RwSignal<Option<InvitationPreview>>,
    pub invitation_code: RwSignal<Option<String>>,
    pub access_code: RwSignal<Option<String>>,
    pub request_code: RwSignal<String>,
    pub request_role: RwSignal<OrganizationRole>,
    pub search_query: RwSignal<String>,
    pub search_result: RwSignal<Option<MemberSearch>>,
    pub search_offset: RwSignal<i64>,
    pub member_edits: RwSignal<bool>,
    pub member_drafts: RwSignal<MemberDrafts>,
    pub suspended: RwSignal<Option<SuspendedDraft>>,
    pub pending: RwSignal<Option<Confirmation>>,
    mutation_in_flight: RwSignal<bool>,
    refresh_after_mutation: RwSignal<bool>,
    generation: RwSignal<u64>,
    search_generation: RwSignal<u64>,
    active: StoredValue<Arc<AtomicBool>>,
}

fn empty_state() -> OrganizationState {
    OrganizationState {
        current_user_id: String::new(),
        organizations: vec![],
        active: None,
        context_version: 1,
        mfa_verified: false,
    }
}

impl Organizations {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(true));
        on_cleanup({
            let active = Arc::clone(&active);
            move || active.store(false, Ordering::Release)
        });
        Self {
            state: RwSignal::new(empty_state()),
            csrf: RwSignal::new(String::new()),
            ready: RwSignal::new(false),
            validating: RwSignal::new(false),
            busy: RwSignal::new(false),
            search_busy: RwSignal::new(false),
            error: RwSignal::new(None),
            message: RwSignal::new(String::new()),
            create_name: RwSignal::new(String::new()),
            rename_name: RwSignal::new(String::new()),
            invite_email: RwSignal::new(String::new()),
            invite_role: RwSignal::new(OrganizationRole::Member),
            invite_hours: RwSignal::new("72".into()),
            invite_until: RwSignal::new(String::new()),
            invitation_token: RwSignal::new(String::new()),
            invitation_preview: RwSignal::new(None),
            invitation_code: RwSignal::new(None),
            access_code: RwSignal::new(None),
            request_code: RwSignal::new(String::new()),
            request_role: RwSignal::new(OrganizationRole::Member),
            search_query: RwSignal::new(String::new()),
            search_result: RwSignal::new(None),
            search_offset: RwSignal::new(0),
            member_edits: RwSignal::new(false),
            member_drafts: RwSignal::new(BTreeMap::new()),
            suspended: RwSignal::new(None),
            pending: RwSignal::new(None),
            mutation_in_flight: RwSignal::new(false),
            refresh_after_mutation: RwSignal::new(false),
            generation: RwSignal::new(0),
            search_generation: RwSignal::new(0),
            active: StoredValue::new(active),
        }
    }

    pub fn scope(self) -> Option<OrganizationScope> {
        let state = self.state.get_untracked();
        state.active.map(|a| OrganizationScope {
            organization_id: a.organization.id,
            context_version: state.context_version,
            organization_version: a.organization.version,
        })
    }

    pub fn administrator(self) -> bool {
        self.state.with(|s| {
            s.mfa_verified
                && s.active
                    .as_ref()
                    .is_some_and(|a| a.organization.role == OrganizationRole::Administrator)
        })
    }

    pub fn blocked(self) -> bool {
        !self.ready.get() || self.validating.get() || self.busy.get() || self.needs_refresh()
    }

    pub fn needs_refresh(self) -> bool {
        self.error.with(|e| {
            e.as_ref().is_some_and(|e| {
                matches!(
                    e.code.as_str(),
                    "session_expired"
                        | "forbidden"
                        | "network_unavailable"
                        | "csrf_invalid"
                        | "context_conflict"
                        | "stale_context"
                        | "scope_unavailable"
                        | "organization_unavailable"
                        | "context_changed"
                        | "version_conflict"
                )
            })
        })
    }

    pub fn dirty(self) -> bool {
        self.has_nonsecret_work()
            || !self.invitation_token.get_untracked().is_empty()
            || !self.request_code.get_untracked().is_empty()
            || self.invitation_code.get_untracked().is_some()
            || self.access_code.get_untracked().is_some()
            || self.suspended.get_untracked().is_some()
    }

    fn has_nonsecret_work(self) -> bool {
        !self.create_name.get_untracked().is_empty()
            || !self.rename_name.get_untracked().is_empty()
            || !self.invite_email.get_untracked().is_empty()
            || !self.invite_until.get_untracked().is_empty()
            || self.invite_role.get_untracked() != OrganizationRole::Member
            || self.invite_hours.get_untracked() != "72"
            || !self.member_drafts.get_untracked().is_empty()
    }

    fn clear_forms(self) {
        self.create_name.set(String::new());
        self.rename_name.set(String::new());
        self.invite_email.set(String::new());
        self.invite_role.set(OrganizationRole::Member);
        self.invite_hours.set("72".into());
        self.invite_until.set(String::new());
        self.invitation_token.set(String::new());
        self.invitation_preview.set(None);
        self.invitation_code.set(None);
        self.access_code.set(None);
        self.request_code.set(String::new());
        self.request_role.set(OrganizationRole::Member);
        self.member_edits.set(false);
        self.member_drafts.set(BTreeMap::new());
        self.search_query.set(String::new());
        self.clear_search();
    }

    fn clear_search(self) {
        self.search_generation.update(|v| *v = v.wrapping_add(1));
        self.search_result.set(None);
        self.search_offset.set(0);
        self.search_busy.set(false);
    }

    pub fn clear_private(self) {
        self.generation.update(|v| *v = v.wrapping_add(1));
        close_dialog();
        self.pending.set(None);
        let state = self.state.get_untracked();
        if !state.current_user_id.is_empty() && self.has_nonsecret_work() {
            self.suspended.set(Some(SuspendedDraft {
                user_id: state.current_user_id,
                organization_id: state.active.map(|a| a.organization.id),
                create_name: self.create_name.get_untracked(),
                rename_name: self.rename_name.get_untracked(),
                invite_email: self.invite_email.get_untracked(),
                invite_role: self.invite_role.get_untracked(),
                invite_hours: self.invite_hours.get_untracked(),
                invite_until: self.invite_until.get_untracked(),
                members: self.member_drafts.get_untracked(),
            }));
        }
        self.clear_forms();
        self.state.set(empty_state());
        self.csrf.set(String::new());
        self.ready.set(false);
        self.validating.set(true);
        self.busy.set(false);
        self.message.set(String::new());
    }

    fn begin_refresh(self) -> u64 {
        if self.pending.get_untracked().is_some() {
            close_dialog();
            self.pending.set(None);
        }
        // Authorization can change while the company ID stays the same. A
        // refresh invalidates both pending and already displayed search data.
        self.clear_search();
        // Hide previously authorized data until the current session and roles
        // have been revalidated, including a return from another tab or bfcache.
        self.validating.set(true);
        self.busy.set(true);
        self.error.set(None);
        self.generation.update(|v| *v = v.wrapping_add(1));
        self.generation.get_untracked()
    }

    pub fn load(self, announce: bool) {
        // Returning from another tab can mean the cookie now identifies a
        // different account. Supersede even an in-flight command response.
        // Wait for that request to finish before revalidating; do not permit a
        // repeated mutation based on a GET that overtook the original POST.
        let generation = self.begin_refresh();
        if self.mutation_in_flight.get_untracked() {
            self.refresh_after_mutation.set(true);
            return;
        }
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::context().await;
            if !active.load(Ordering::Acquire) || self.generation.get_untracked() != generation {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(response) => {
                    self.accept(response);
                    self.ready.set(true);
                    self.validating.set(false);
                    if announce {
                        self.message.set(
                            "Acesso e empresa atualizados. Nenhuma operação foi reenviada.".into(),
                        );
                    }
                }
                Err(error) => self.failed(error),
            }
        });
    }

    fn accept(self, response: OrganizationResponse) {
        let previous = self.state.get_untracked();
        let previously_admin = previous.mfa_verified
            && previous
                .active
                .as_ref()
                .is_some_and(|a| a.organization.role == OrganizationRole::Administrator);
        let now_admin = response.state.mfa_verified
            && response
                .state
                .active
                .as_ref()
                .is_some_and(|a| a.organization.role == OrganizationRole::Administrator);
        let previous_id = previous.active.as_ref().map(|a| &a.organization.id);
        let next_id = response.state.active.as_ref().map(|a| &a.organization.id);
        if previous_id != next_id
            || previous.context_version != response.state.context_version
            || (!previous.current_user_id.is_empty()
                && previous.current_user_id != response.state.current_user_id)
        {
            self.clear_forms();
        }
        if previously_admin && !now_admin {
            self.rename_name.set(String::new());
            self.invite_email.set(String::new());
            self.invite_until.set(String::new());
            self.invitation_code.set(None);
            self.access_code.set(None);
            self.member_edits.set(false);
            self.member_drafts.set(BTreeMap::new());
            self.clear_search();
        }
        // A freshly created company's code can be visible without that company
        // being selected. Revalidate each displayed code against its own scope.
        let may_display = |code: &str| {
            response.state.mfa_verified
                && code.split_once('.').is_some_and(|(id, _)| {
                    response
                        .state
                        .organizations
                        .iter()
                        .any(|o| o.id == id && o.role == OrganizationRole::Administrator)
                })
        };
        if self
            .access_code
            .get_untracked()
            .as_deref()
            .is_some_and(|code| !may_display(code))
        {
            self.access_code.set(None);
        }
        if self
            .invitation_code
            .get_untracked()
            .as_deref()
            .is_some_and(|code| !may_display(code))
        {
            self.invitation_code.set(None);
        }
        if let Some(draft) = self.suspended.get_untracked() {
            let same_user = draft.user_id == response.state.current_user_id;
            let still_authorized = draft
                .organization_id
                .as_ref()
                .is_none_or(|id| response.state.organizations.iter().any(|a| &a.id == id));
            let same_organization = draft.organization_id.as_ref() == next_id;
            if !same_user || !still_authorized || (next_id.is_some() && !same_organization) {
                self.suspended.set(None);
            } else if same_organization {
                self.create_name.set(draft.create_name);
                if now_admin {
                    self.rename_name.set(draft.rename_name);
                    self.invite_email.set(draft.invite_email);
                    self.invite_role.set(draft.invite_role);
                    self.invite_hours.set(draft.invite_hours);
                    self.invite_until.set(draft.invite_until);
                    self.member_edits.set(!draft.members.is_empty());
                    self.member_drafts.set(draft.members);
                }
                self.suspended.set(None);
            }
        }
        self.state.set(response.state);
        self.csrf.set(response.csrf);
        self.error.set(None);
        self.message.set(response.message);
        if response.invitation_code.is_some() {
            self.invitation_code.set(response.invitation_code);
        }
        if response.access_code.is_some() {
            self.access_code.set(response.access_code);
        }
        if response.invitation_preview.is_some() {
            self.invitation_preview.set(response.invitation_preview);
        }
    }

    fn failed(self, error: AuthFailure) {
        if matches!(
            error.code.as_str(),
            "session_expired" | "csrf_invalid" | "forbidden"
        ) {
            self.clear_private();
        } else if matches!(
            error.code.as_str(),
            "scope_unavailable"
                | "organization_unavailable"
                | "context_conflict"
                | "stale_context"
                | "context_changed"
        ) {
            self.suspended.set(None);
            self.clear_forms();
            self.state.update(|s| {
                s.active = None;
                s.organizations.clear();
            });
            self.invitation_code.set(None);
            self.access_code.set(None);
            self.invitation_preview.set(None);
            self.clear_search();
        }
        self.error.set(Some(error));
    }

    pub fn local_error(self, message: &str, field: &'static str) {
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

    pub fn select(self, target: Option<String>) {
        if self.blocked() {
            return;
        }
        let expected_context_version = self.state.get_untracked().context_version;
        let resuming = self
            .suspended
            .get_untracked()
            .is_some_and(|draft| draft.organization_id == target);
        let command = match target {
            Some(organization_id) => OrganizationCommand::Select {
                organization_id,
                expected_context_version,
            },
            None => OrganizationCommand::ClearSelection {
                expected_context_version,
            },
        };
        if self.dirty() && !resuming {
            self.confirm(command, "Trocar a empresa ativa?", "Os formulários, códigos e filtros desta página serão limpos. Copie os códigos necessários antes de continuar.", "org-selection");
        } else {
            self.submit(command);
        }
    }

    pub fn confirm(
        self,
        command: OrganizationCommand,
        title: &str,
        description: &str,
        return_focus: &str,
    ) {
        if self.blocked() {
            return;
        }
        self.pending.set(Some(Confirmation {
            command,
            title: title.into(),
            description: description.into(),
            return_focus: return_focus.into(),
        }));
        let active = self.active.get_value();
        let generation = self.generation.get_untracked();
        request_animation_frame(move || {
            if active.load(Ordering::Acquire)
                && self.generation.get_untracked() == generation
                && self.pending.get_untracked().is_some()
                && !self.blocked()
            {
                open_dialog();
            }
        });
    }

    pub fn dismiss(self) {
        let previous = self.pending.get_untracked();
        close_dialog();
        self.pending.set(None);
        let active = self.active.get_value();
        let generation = self.generation.get_untracked();
        request_animation_frame(move || {
            if active.load(Ordering::Acquire)
                && self.generation.get_untracked() == generation
                && !self.validating.get_untracked()
            {
                focus(
                    previous
                        .as_ref()
                        .map_or("org-selection", |p| &p.return_focus),
                );
            }
        });
    }

    pub fn confirm_pending(self) {
        let Some(pending) = self.pending.get_untracked() else {
            return;
        };
        close_dialog();
        self.pending.set(None);
        self.submit(pending.command);
    }

    pub fn submit(self, command: OrganizationCommand) {
        if self.blocked() {
            return;
        }
        let switching = matches!(
            command,
            OrganizationCommand::Select { .. } | OrganizationCommand::ClearSelection { .. }
        );
        self.busy.set(true);
        self.mutation_in_flight.set(true);
        self.error.set(None);
        self.message.set(String::new());
        self.generation.update(|v| *v = v.wrapping_add(1));
        self.clear_search();
        let generation = self.generation.get_untracked();
        if switching {
            let target = match &command {
                OrganizationCommand::Select {
                    organization_id, ..
                } => Some(organization_id),
                _ => None,
            };
            if self
                .suspended
                .get_untracked()
                .is_some_and(|draft| draft.organization_id.as_ref() != target)
            {
                self.suspended.set(None);
            }
            self.clear_forms();
            self.state.update(|s| s.active = None);
        }
        let csrf = self.csrf.get_untracked();
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::command(&command, &csrf).await;
            if !active.load(Ordering::Acquire) {
                return;
            }
            self.mutation_in_flight.set(false);
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
                Ok(response) => {
                    self.accept(response);
                    match command {
                        OrganizationCommand::Create { .. } => self.create_name.set(String::new()),
                        OrganizationCommand::Rename { .. } => self.rename_name.set(String::new()),
                        OrganizationCommand::Invite { .. } => {
                            self.invite_email.set(String::new());
                            self.invite_until.set(String::new());
                        }
                        OrganizationCommand::AcceptInvitation { .. } => {
                            self.invitation_token.set(String::new());
                            self.invitation_preview.set(None);
                        }
                        OrganizationCommand::RequestAccess { .. } => {
                            self.request_code.set(String::new())
                        }
                        OrganizationCommand::ChangeMember { user_id, .. }
                        | OrganizationCommand::RemoveMember { user_id, .. } => {
                            self.member_drafts
                                .update(|rows| rows.retain(|(_, id, _), _| id != &user_id));
                            self.member_edits
                                .set(!self.member_drafts.get_untracked().is_empty());
                        }
                        _ => {}
                    }
                }
                Err(error) => self.failed(error),
            }
            let focus_generation = self.generation.get_untracked();
            request_animation_frame(move || {
                if active.load(Ordering::Acquire)
                    && self.generation.get_untracked() == focus_generation
                {
                    focus("org-feedback");
                }
            });
        });
    }

    pub fn search(self, offset: i64) {
        if self.blocked() || !self.administrator() {
            return;
        }
        let Some(scope) = self.scope() else {
            return;
        };
        let query = self.search_query.get_untracked();
        self.search_generation.update(|v| *v = v.wrapping_add(1));
        let search_generation = self.search_generation.get_untracked();
        let generation = self.generation.get_untracked();
        self.search_busy.set(true);
        self.error.set(None);
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::members(
                &scope.organization_id,
                scope.context_version,
                &query,
                offset,
            )
            .await;
            if !active.load(Ordering::Acquire)
                || self.generation.get_untracked() != generation
                || self.search_generation.get_untracked() != search_generation
            {
                return;
            }
            self.search_busy.set(false);
            match result {
                Ok(result)
                    if result.organization_id == scope.organization_id
                        && result.context_version == scope.context_version =>
                {
                    self.search_result.set(Some(result));
                    self.search_offset.set(offset);
                }
                Ok(_) => self.failed(AuthFailure {
                    code: "context_conflict".into(),
                    message: "A empresa mudou. Atualize o acesso antes de continuar.".into(),
                }),
                Err(error) => self.failed(error),
            }
        });
    }

    pub fn member_rows(self) -> Vec<MemberSummary> {
        self.search_result
            .get()
            .map(|r| r.members)
            .unwrap_or_else(|| self.state.get().active.map_or_else(Vec::new, |a| a.members))
    }
}

pub(super) fn role(value: &str) -> OrganizationRole {
    if value == "administrator" {
        OrganizationRole::Administrator
    } else {
        OrganizationRole::Member
    }
}

/// The date is interpreted in Brasília; invalid input remains an explicit
/// server validation failure rather than becoming a missing expiry.
pub(super) fn end_of_day(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    use chrono::TimeZone;
    Some(
        chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
            .ok()
            .and_then(|date| date.and_hms_opt(23, 59, 59))
            .and_then(|date| {
                chrono_tz::America::Sao_Paulo
                    .from_local_datetime(&date)
                    .single()
            })
            .map(|date| date.to_rfc3339())
            .unwrap_or_else(|| value.to_owned()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(user: &str, active: Option<&str>) -> OrganizationResponse {
        let organizations: Vec<_> = ["empresa-a", "empresa-b"]
            .into_iter()
            .map(|id| OrganizationSummary {
                id: id.into(),
                name: id.into(),
                role: OrganizationRole::Administrator,
                version: 1,
                valid_until: None,
            })
            .collect();
        OrganizationResponse {
            state: OrganizationState {
                current_user_id: user.into(),
                active: active.map(|id| OrganizationDetail {
                    organization: organizations.iter().find(|o| o.id == id).unwrap().clone(),
                    members: vec![],
                    invitations: vec![],
                    access_requests: vec![],
                }),
                organizations,
                context_version: 1,
                mfa_verified: true,
            },
            csrf: String::new(),
            message: String::new(),
            invitation_code: None,
            access_code: None,
            invitation_preview: None,
        }
    }

    #[test]
    fn expired_session_hides_work_until_same_identity_and_company_are_confirmed() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.invite_email.set("rascunho@example.invalid".into());
            org.rename_name.set("Nome em revisão".into());
            org.member_drafts.update(|rows| {
                rows.insert(
                    ("empresa-a".into(), "pessoa-b".into(), 1),
                    (OrganizationRole::Member, "2027-08-30".into()),
                );
            });
            org.member_edits.set(true);
            org.invitation_code.set(Some("transient-fixture".into()));
            org.failed(AuthFailure {
                code: "session_expired".into(),
                message: "Entre novamente".into(),
            });
            assert!(org.state.get_untracked().active.is_none());
            assert!(org.state.get_untracked().organizations.is_empty());
            assert!(org.invite_email.get_untracked().is_empty());
            assert!(org.invitation_code.get_untracked().is_none());
            org.accept(response("pessoa-a", None));
            assert!(org.suspended.get_untracked().is_some());
            assert!(org.invite_email.get_untracked().is_empty());
            org.accept(response("pessoa-a", Some("empresa-a")));
            assert_eq!(org.invite_email.get_untracked(), "rascunho@example.invalid");
            assert_eq!(org.rename_name.get_untracked(), "Nome em revisão");
            assert_eq!(org.member_drafts.get_untracked().len(), 1);
            assert!(org.invitation_code.get_untracked().is_none());
        });
    }

    #[test]
    fn work_never_returns_to_another_identity_or_company() {
        for (user, company) in [("pessoa-b", "empresa-a"), ("pessoa-a", "empresa-b")] {
            Owner::new().with(|| {
                let org = Organizations::new();
                org.accept(response("pessoa-a", Some("empresa-a")));
                org.invite_email.set("privado@example.invalid".into());
                org.failed(AuthFailure {
                    code: "session_expired".into(),
                    message: String::new(),
                });
                org.accept(response(user, Some(company)));
                assert!(org.invite_email.get_untracked().is_empty());
                assert!(org.suspended.get_untracked().is_none());
            });
        }
    }

    #[test]
    fn revoked_membership_clears_context_and_private_codes() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.invite_email.set("privado@example.invalid".into());
            org.access_code.set(Some("transient-fixture".into()));
            org.failed(AuthFailure {
                code: "organization_unavailable".into(),
                message: String::new(),
            });
            assert!(org.state.get_untracked().active.is_none());
            assert!(org.state.get_untracked().organizations.is_empty());
            assert!(org.access_code.get_untracked().is_none());
            assert!(org.invite_email.get_untracked().is_empty());
            assert!(org.needs_refresh());
        });
    }

    #[test]
    fn loss_of_administration_removes_cached_people_and_codes_without_company_change() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.access_code.set(Some("transient-fixture".into()));
            org.invitation_code.set(Some("transient-fixture".into()));
            org.search_result.set(Some(MemberSearch {
                organization_id: "empresa-a".into(),
                context_version: 1,
                query: String::new(),
                total: 7,
                members: vec![MemberSummary {
                    id: "outra-pessoa".into(),
                    email: "privado@example.invalid".into(),
                    role: OrganizationRole::Member,
                    active: true,
                    valid_until: None,
                    version: 1,
                }],
            }));
            org.search_busy.set(true);
            let mut downgraded = response("pessoa-a", Some("empresa-a"));
            downgraded.state.active.as_mut().unwrap().organization.role = OrganizationRole::Member;
            org.accept(downgraded);
            assert!(org.member_rows().is_empty());
            assert!(org.search_result.get_untracked().is_none());
            assert!(!org.search_busy.get_untracked());
            assert!(org.access_code.get_untracked().is_none());
            assert!(org.invitation_code.get_untracked().is_none());
        });
    }

    #[test]
    fn forbidden_identity_hides_private_context_and_preserves_only_bound_work() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.ready.set(true);
            org.invite_email.set("rascunho@example.invalid".into());
            org.failed(AuthFailure {
                code: "forbidden".into(),
                message: String::new(),
            });
            assert!(!org.ready.get_untracked());
            assert!(org.validating.get_untracked());
            assert!(org.state.get_untracked().organizations.is_empty());
            assert!(org.invite_email.get_untracked().is_empty());
            assert!(org.suspended.get_untracked().is_some());
            assert!(org.needs_refresh());
        });
    }

    #[test]
    fn repeated_hiding_before_reselection_keeps_original_company_draft() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.invite_email.set("rascunho@example.invalid".into());
            org.clear_private();
            org.accept(response("pessoa-a", None));
            org.clear_private();
            org.accept(response("pessoa-a", None));
            org.failed(AuthFailure {
                code: "forbidden".into(),
                message: String::new(),
            });
            org.accept(response("pessoa-a", Some("empresa-a")));
            assert_eq!(org.invite_email.get_untracked(), "rascunho@example.invalid");
            assert!(org.suspended.get_untracked().is_none());
        });
    }

    #[test]
    fn mandatory_refresh_supersedes_an_in_flight_response() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.ready.set(true);
            org.busy.set(true);
            org.search_busy.set(true);
            let previous_generation = org.generation.get_untracked();
            let next_generation = org.begin_refresh();
            assert_ne!(previous_generation, next_generation);
            assert!(org.validating.get_untracked());
            assert!(org.blocked());
            assert!(!org.search_busy.get_untracked());
        });
    }

    #[test]
    fn hiding_only_transient_codes_does_not_create_a_draft() {
        Owner::new().with(|| {
            let org = Organizations::new();
            org.accept(response("pessoa-a", Some("empresa-a")));
            org.access_code
                .set(Some("empresa-a.transient-fixture".into()));
            org.clear_private();
            assert!(org.access_code.get_untracked().is_none());
            assert!(org.suspended.get_untracked().is_none());
        });
    }

    #[test]
    fn civil_expiry_uses_brasilia_and_does_not_hide_invalid_input() {
        assert_eq!(
            end_of_day("2027-05-20").as_deref(),
            Some("2027-05-20T23:59:59-03:00")
        );
        assert_eq!(end_of_day(""), None);
        assert_eq!(end_of_day("2027-02-30").as_deref(), Some("2027-02-30"));
    }
}
