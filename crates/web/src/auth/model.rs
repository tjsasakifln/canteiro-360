use super::{focus, network};
use canteiro_contracts::auth::{AuthCommand, AuthContext, AuthFailure, AuthResponse, AuthState};
use leptos::prelude::*;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Mode {
    #[default]
    Login,
    Register,
    Verify,
    Recovery,
    Reset,
}

#[derive(Clone, Copy)]
pub(super) struct Access {
    pub context: RwSignal<AuthContext>,
    pub csrf: RwSignal<String>,
    pub mode: RwSignal<Mode>,
    pub ready: RwSignal<bool>,
    pub busy: RwSignal<bool>,
    pub error: RwSignal<Option<AuthFailure>>,
    pub message: RwSignal<String>,
    pub setup_key: RwSignal<Option<String>>,
    pub recovery_codes: RwSignal<Vec<String>>,
    pub email: RwSignal<String>,
    pub password: RwSignal<String>,
    pub new_password: RwSignal<String>,
    pub verification_password: RwSignal<String>,
    pub verify_token: RwSignal<String>,
    pub reset_token: RwSignal<String>,
    pub code: RwSignal<String>,
    pub backup_code: RwSignal<String>,
    pub setup_password: RwSignal<String>,
    active: StoredValue<Arc<AtomicBool>>,
}

impl Access {
    pub fn new() -> Self {
        let active = Arc::new(AtomicBool::new(true));
        on_cleanup({
            let active = Arc::clone(&active);
            move || active.store(false, Ordering::Release)
        });
        Self {
            context: RwSignal::new(AuthContext {
                state: AuthState::Anonymous,
                email: None,
                privileged: false,
                mfa_enabled: false,
                sessions: vec![],
            }),
            csrf: RwSignal::new(String::new()),
            mode: RwSignal::new(Mode::Login),
            ready: RwSignal::new(false),
            busy: RwSignal::new(false),
            error: RwSignal::new(None),
            message: RwSignal::new(String::new()),
            setup_key: RwSignal::new(None),
            recovery_codes: RwSignal::new(vec![]),
            email: RwSignal::new(String::new()),
            password: RwSignal::new(String::new()),
            new_password: RwSignal::new(String::new()),
            verification_password: RwSignal::new(String::new()),
            verify_token: RwSignal::new(String::new()),
            reset_token: RwSignal::new(String::new()),
            code: RwSignal::new(String::new()),
            backup_code: RwSignal::new(String::new()),
            setup_password: RwSignal::new(String::new()),
            active: StoredValue::new(active),
        }
    }

    pub fn blocked(self) -> bool {
        !self.ready.get() || self.busy.get() || self.needs_refresh()
    }

    pub fn needs_refresh(self) -> bool {
        self.error.with(|error| {
            error.as_ref().is_some_and(|error| {
                matches!(error.code.as_str(), "session_expired" | "csrf_invalid")
            })
        })
    }

    pub fn switch(self, mode: Mode) {
        if self.busy.get_untracked() || self.needs_refresh() {
            return;
        }
        self.mode.set(mode);
        self.error.set(None);
        self.message.set(String::new());
        let active = self.active.get_value();
        request_animation_frame(move || {
            if active.load(Ordering::Acquire) {
                focus("auth-form-title");
            }
        });
    }

    /// Reconnects without replaying any command or clearing the current form.
    pub fn load(self) {
        if self.busy.get_untracked() {
            return;
        }
        self.busy.set(true);
        self.error.set(None);
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::context().await;
            if !active.load(Ordering::Acquire) {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(response) => {
                    self.accept(response);
                    self.ready.set(true);
                }
                Err(error) => self.failed(error),
            }
        });
    }

    fn accept(self, response: AuthResponse) {
        let AuthResponse {
            context,
            csrf,
            message,
            mfa_setup,
            recovery_codes,
        } = response;
        let previous_email = self.context.get_untracked().email;
        if previous_email.is_some() && context.email.is_some() && previous_email != context.email {
            self.clear_secrets();
        }
        if context.state == AuthState::Anonymous || previous_email != context.email {
            self.setup_key.set(None);
            self.recovery_codes.set(vec![]);
        }
        self.context.set(context);
        self.csrf.set(csrf);
        self.message.set(message);
        self.error.set(None);
        if mfa_setup.is_some() {
            self.setup_key.set(mfa_setup);
        }
        if !recovery_codes.is_empty() {
            self.recovery_codes.set(recovery_codes);
        }
    }

    fn failed(self, error: AuthFailure) {
        if matches!(error.code.as_str(), "session_expired" | "csrf_invalid") {
            self.setup_key.set(None);
            self.recovery_codes.set(vec![]);
        }
        if error.code == "session_expired"
            && self.context.get_untracked().state != AuthState::Anonymous
        {
            self.context.set(AuthContext {
                state: AuthState::Anonymous,
                email: None,
                privileged: false,
                mfa_enabled: false,
                sessions: vec![],
            });
            self.mode.set(Mode::Login);
        }
        self.error.set(Some(error));
    }

    pub fn local_error(self, message: &str, input: &'static str) {
        self.error.set(Some(AuthFailure {
            code: "invalid_input".into(),
            message: message.into(),
        }));
        let active = self.active.get_value();
        request_animation_frame(move || {
            if active.load(Ordering::Acquire) {
                focus(input);
            }
        });
    }

    pub fn submit(self, command: AuthCommand) {
        if self.blocked() {
            return;
        }
        self.busy.set(true);
        self.error.set(None);
        self.message.set(String::new());
        let csrf = self.csrf.get_untracked();
        let active = self.active.get_value();
        leptos::task::spawn_local(async move {
            let result = network::command(&command, &csrf).await;
            if !active.load(Ordering::Acquire) {
                return;
            }
            self.busy.set(false);
            match result {
                Ok(response) => {
                    self.accept(response);
                    match command {
                        AuthCommand::Register { password, .. } => {
                            self.verification_password.set(password);
                            self.new_password.set(String::new());
                            self.mode.set(Mode::Verify);
                        }
                        AuthCommand::VerifyEmail { .. } => {
                            self.verify_token.set(String::new());
                            self.verification_password.set(String::new());
                            self.new_password.set(String::new());
                            self.password.set(String::new());
                            self.mode.set(Mode::Login);
                        }
                        AuthCommand::RequestVerification { .. } => self.mode.set(Mode::Verify),
                        AuthCommand::RequestReset { .. } => self.mode.set(Mode::Reset),
                        AuthCommand::ResetPassword { .. } => {
                            self.new_password.set(String::new());
                            self.reset_token.set(String::new());
                            self.password.set(String::new());
                            self.mode.set(Mode::Login);
                        }
                        AuthCommand::Login { .. } => self.password.set(String::new()),
                        AuthCommand::StartMfa { .. } => self.setup_password.set(String::new()),
                        AuthCommand::ConfirmMfa { .. } => {
                            self.code.set(String::new());
                            self.setup_key.set(None);
                        }
                        AuthCommand::VerifyMfa { .. } | AuthCommand::RecoverMfa { .. } => {
                            self.code.set(String::new());
                            self.backup_code.set(String::new());
                        }
                        AuthCommand::Logout | AuthCommand::RevokeAll => {
                            self.clear_secrets();
                            self.mode.set(Mode::Login);
                        }
                        AuthCommand::RevokeSession { .. } | AuthCommand::PrivilegedCheck => {}
                    }
                    if self.context.get_untracked().state == AuthState::Anonymous {
                        self.setup_key.set(None);
                        self.recovery_codes.set(vec![]);
                    }
                }
                Err(error) => self.failed(error),
            }
            let active = Arc::clone(&active);
            request_animation_frame(move || {
                if active.load(Ordering::Acquire) {
                    if !self.recovery_codes.get_untracked().is_empty()
                        && self.error.get_untracked().is_none()
                    {
                        focus("auth-recovery-codes");
                    } else if self.error.get_untracked().is_some()
                        || !self.message.get_untracked().is_empty()
                    {
                        focus("auth-feedback");
                    } else {
                        focus(match self.context.get_untracked().state {
                            AuthState::Anonymous => "auth-form-title",
                            AuthState::MfaPending => "auth-mfa-title",
                            AuthState::Authenticated => "auth-account-title",
                        });
                    }
                }
            });
        });
    }

    fn clear_secrets(self) {
        self.password.set(String::new());
        self.new_password.set(String::new());
        self.verification_password.set(String::new());
        self.verify_token.set(String::new());
        self.reset_token.set(String::new());
        self.code.set(String::new());
        self.backup_code.set(String::new());
        self.setup_password.set(String::new());
        self.setup_key.set(None);
        self.recovery_codes.set(vec![]);
    }
}

/// Read the actual field at submission, including password-manager autofill.
pub(super) fn entered(id: &str, signal: RwSignal<String>) -> String {
    #[cfg(feature = "hydrate")]
    {
        use wasm_bindgen::JsCast;
        if let Some(input) = web_sys::window()
            .and_then(|w| w.document())
            .and_then(|d| d.get_element_by_id(id))
            .and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok())
        {
            let value = input.value();
            signal.set(value.clone());
            return value;
        }
    }
    #[cfg(not(feature = "hydrate"))]
    let _ = id;
    signal.get_untracked()
}

pub(super) fn valid_new_password(password: &str) -> bool {
    (15..=128).contains(&password.chars().count()) && password.len() <= 1024
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn password_length_is_characters_without_composition_challenges() {
        assert!(!valid_new_password("curta"));
        assert!(valid_new_password("uma frase longa sem cifras"));
        assert!(valid_new_password(&"é".repeat(15)));
        assert!(valid_new_password(&"🔑".repeat(128)));
        assert!(!valid_new_password(&"a".repeat(129)));
    }
}
