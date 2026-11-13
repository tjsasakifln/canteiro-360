use super::model::{Access, Mode, entered, valid_new_password};
use canteiro_contracts::auth::AuthCommand;
use leptos::prelude::*;

const PASSWORD_HINT: &str = "Use de 15 a 128 caracteres. Uma frase longa funciona; não exigimos combinações de símbolos. Você pode colar ou usar seu gerenciador de senhas.";

#[component]
fn Input(
    id: &'static str,
    label: &'static str,
    value: RwSignal<String>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(default = "off")] autocomplete: &'static str,
    #[prop(default = "text")] inputmode: &'static str,
    #[prop(default = 254)] max_length: u32,
    #[prop(default = "")] hint: &'static str,
) -> impl IntoView {
    let access = expect_context::<Access>();
    let help = format!("{id}-help");
    let help_ref = help.clone();
    let reveal = RwSignal::new(false);
    view! {
        <div class="field access-field">
            <label for=id>{label}</label>
            <div class="access-input-row">
                <input id=id name=id type=move || if kind == "password" && reveal.get() { "text" } else { kind }
                    required autocomplete=autocomplete inputmode=inputmode maxlength=max_length
                    spellcheck="false" autocapitalize="none" value="" prop:value=move || value.get()
                    disabled=move || access.busy.get() || !access.ready.get()
                    aria-describedby=help_ref on:input=move |event| value.set(event_target_value(&event))/>
                <Show when=move || kind == "password">
                    <button type="button" class="access-reveal" aria-controls=id
                        aria-label=move || if reveal.get() { format!("Ocultar {}", label.to_lowercase()) } else { format!("Mostrar {}", label.to_lowercase()) }
                        aria-pressed=move || reveal.get().to_string()
                        disabled=move || access.busy.get() || !access.ready.get()
                        on:click=move |_| reveal.update(|v| *v = !*v)>{move || if reveal.get() { "Ocultar" } else { "Mostrar" }}</button>
                </Show>
            </div>
            <p id=help class="field-hint" hidden=hint.is_empty()>{hint}</p>
        </div>
    }
}

#[component]
fn Submit(id: &'static str, label: &'static str) -> impl IntoView {
    let access = expect_context::<Access>();
    view! { <button id=id type="submit" class="primary-button" disabled=move || access.blocked()>{label}</button> }
}

#[component]
fn ModeButton(id: &'static str, label: &'static str, mode: Mode) -> impl IntoView {
    let access = expect_context::<Access>();
    view! { <button id=id type="button" class="access-text-button" disabled=move || access.busy.get() || access.needs_refresh()
    on:click=move |_| access.switch(mode)>{label}</button> }
}

#[component]
pub(super) fn AnonymousForms() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <section class="section-card access-card" aria-labelledby="auth-form-title">
            <h2 id="auth-form-title" tabindex="-1">{move || match access.mode.get() {
                Mode::Login => "Entrar na sua conta",
                Mode::Register => "Criar sua conta",
                Mode::Verify => "Confirmar seu e-mail",
                Mode::Recovery => "Recuperar acesso",
                Mode::Reset => "Definir uma nova senha",
            }}</h2>
            <p class="access-form-intro">{move || match access.mode.get() {
                Mode::Login => "Informe seu e-mail e sua senha. Se houver verificação em duas etapas, ela será solicitada em seguida.",
                Mode::Register => "Depois de enviar, confirme seu e-mail com o código recebido para poder entrar.",
                Mode::Verify => "Cole o código recebido por e-mail e defina a senha que ficará na sua conta. O código vale por 15 minutos e pode ser usado uma vez.",
                Mode::Recovery => "Informe seu e-mail. Se o endereço estiver habilitado, enviaremos as orientações de recuperação.",
                Mode::Reset => "Cole o código de recuperação e escolha uma nova senha. O código vale por 15 minutos e só pode ser usado uma vez.",
            }}</p>
            <Show when=move || access.mode.get() == Mode::Login><LoginForm/></Show>
            <Show when=move || access.mode.get() == Mode::Register><RegisterForm/></Show>
            <Show when=move || access.mode.get() == Mode::Verify><VerifyForm/></Show>
            <Show when=move || access.mode.get() == Mode::Recovery><RecoveryForm/></Show>
            <Show when=move || access.mode.get() == Mode::Reset><ResetForm/></Show>
        </section>
    }
}

#[component]
fn LoginForm() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <form id="auth-login-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
            on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::Login {
                email: entered("auth-login-email", access.email), password: entered("auth-login-password", access.password),
            }); }>
            <Input id="auth-login-email" label="E-mail" value=access.email kind="email" autocomplete="username" inputmode="email"/>
            <Input id="auth-login-password" label="Senha" value=access.password kind="password" autocomplete="current-password" max_length=1024/>
            <Submit id="auth-login-submit" label="Entrar"/>
        </form>
        <div class="access-links">
            <ModeButton id="auth-open-recovery" label="Esqueci minha senha" mode=Mode::Recovery/>
            <ModeButton id="auth-open-register" label="Criar conta" mode=Mode::Register/>
            <ModeButton id="auth-open-verification" label="Confirmar e-mail" mode=Mode::Verify/>
        </div>
    }
}

#[component]
fn RegisterForm() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <form id="auth-register-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
            on:submit=move |ev| {
                ev.prevent_default();
                let email = entered("auth-register-email", access.email);
                let password = entered("auth-register-password", access.new_password);
                if !valid_new_password(&password) { access.local_error("Use uma senha com 15 a 128 caracteres.", "auth-register-password"); return; }
                access.submit(AuthCommand::Register { email, password });
            }>
            <Input id="auth-register-email" label="E-mail" value=access.email kind="email" autocomplete="username" inputmode="email"/>
            <Input id="auth-register-password" label="Nova senha" value=access.new_password kind="password" autocomplete="new-password" max_length=1024 hint=PASSWORD_HINT/>
            <Submit id="auth-register-submit" label="Criar conta e receber código"/>
        </form>
        <div class="access-links"><ModeButton id="auth-register-back" label="Já tenho uma conta" mode=Mode::Login/></div>
    }
}

#[component]
fn VerifyForm() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <form id="auth-verify-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
            on:submit=move |ev| {
                ev.prevent_default();
                let token = entered("auth-verify-token", access.verify_token).trim().to_owned();
                let password = entered("auth-verify-password", access.verification_password);
                if !valid_new_password(&password) { access.local_error("Use uma senha com 15 a 128 caracteres.", "auth-verify-password"); return; }
                access.submit(AuthCommand::VerifyEmail { token, password });
            }>
            <Input id="auth-verify-token" label="Código de confirmação do e-mail" value=access.verify_token autocomplete="one-time-code" max_length=128
                hint="Cole o código completo. Você não precisa memorizá-lo."/>
            <Input id="auth-verify-password" label="Senha da sua conta" value=access.verification_password kind="password" autocomplete="new-password" max_length=1024 hint=PASSWORD_HINT/>
            <Submit id="auth-verify-submit" label="Confirmar e-mail e senha"/>
        </form>
        <details class="access-details"><summary>"Não recebi o código ou ele expirou"</summary>
            <p>"Confira também a pasta de spam. Ao solicitar um novo código, use a mensagem mais recente."</p>
            <form id="auth-resend-form" method="post" action="/acesso" on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::RequestVerification { email: entered("auth-resend-email", access.email) }); }>
                <Input id="auth-resend-email" label="E-mail para confirmação" value=access.email kind="email" autocomplete="username" inputmode="email"/>
                <Submit id="auth-resend-submit" label="Solicitar novo código"/>
            </form>
        </details>
        <div class="access-links"><ModeButton id="auth-verify-back" label="Voltar para entrar" mode=Mode::Login/></div>
    }
}

#[component]
fn RecoveryForm() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <form id="auth-recovery-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
            on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::RequestReset { email: entered("auth-recovery-email", access.email) }); }>
            <Input id="auth-recovery-email" label="E-mail da conta" value=access.email kind="email" autocomplete="username" inputmode="email"/>
            <Submit id="auth-recovery-submit" label="Enviar orientações de recuperação"/>
        </form>
        <p class="access-small-note">"A recuperação mantém a verificação em duas etapas. Depois de alterar a senha, entre novamente."</p>
        <div class="access-links">
            <ModeButton id="auth-open-reset" label="Já recebi meu código" mode=Mode::Reset/>
            <ModeButton id="auth-recovery-back" label="Voltar para entrar" mode=Mode::Login/>
        </div>
    }
}

#[component]
fn ResetForm() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <form id="auth-reset-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
            on:submit=move |ev| {
                ev.prevent_default();
                let token = entered("auth-reset-token", access.reset_token).trim().to_owned();
                let password = entered("auth-reset-password", access.new_password);
                if !valid_new_password(&password) { access.local_error("Use uma senha com 15 a 128 caracteres.", "auth-reset-password"); return; }
                access.submit(AuthCommand::ResetPassword { token, password });
            }>
            <Input id="auth-reset-token" label="Código de recuperação" value=access.reset_token autocomplete="one-time-code" max_length=128/>
            <Input id="auth-reset-password" label="Nova senha" value=access.new_password kind="password" autocomplete="new-password" max_length=1024 hint=PASSWORD_HINT/>
            <p class="notice warning">"Alterar a senha encerra as sessões anteriores. Quando ativada, a verificação em duas etapas permanece."</p>
            <Submit id="auth-reset-submit" label="Alterar senha"/>
        </form>
        <div class="access-links">
            <ModeButton id="auth-reset-request" label="Pedir outro código" mode=Mode::Recovery/>
            <ModeButton id="auth-reset-back" label="Voltar para entrar" mode=Mode::Login/>
        </div>
    }
}

#[component]
pub(super) fn MfaChallenge() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <section class="section-card access-card" aria-labelledby="auth-mfa-title">
            <h2 id="auth-mfa-title" tabindex="-1">"Verificação em duas etapas"</h2>
            <p>{move || access.context.get().email.unwrap_or_default()}</p>
            <Show when=move || access.context.get().mfa_enabled fallback=move || view! {
                <p>"Este perfil precisa da verificação em duas etapas antes de continuar. Configure seu aplicativo autenticador abaixo."</p>
                <MfaEnrollment/>
            }>
                <p>"Informe o código atual do seu aplicativo autenticador. Você pode copiar e colar."</p>
                <form id="auth-mfa-form" method="post" action="/acesso" aria-busy=move || access.busy.get().to_string()
                    on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::VerifyMfa { code: entered("auth-mfa-code", access.code) }); }>
                    <Input id="auth-mfa-code" label="Código do aplicativo autenticador" value=access.code autocomplete="one-time-code" inputmode="numeric" max_length=16/>
                    <Submit id="auth-mfa-submit" label="Confirmar e entrar"/>
                </form>
                <details id="auth-backup-details" class="access-details"><summary>"Usar código de reserva"</summary>
                    <p>"Se você perdeu acesso ao aplicativo, use um dos códigos guardados na configuração. Cada código pode ser usado uma vez."</p>
                    <form id="auth-backup-form" method="post" action="/acesso" on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::RecoverMfa { code: entered("auth-backup-code", access.backup_code) }); }>
                        <Input id="auth-backup-code" label="Código de reserva" value=access.backup_code autocomplete="one-time-code" max_length=128/>
                        <Submit id="auth-backup-submit" label="Entrar com código de reserva"/>
                    </form>
                    <p class="access-small-note">"Sem o aplicativo e sem código de reserva, este acesso permanece bloqueado. A redefinição de senha não remove essa proteção."</p>
                </details>
            </Show>
            <button id="auth-mfa-cancel" type="button" class="access-text-button" disabled=move || access.blocked()
                on:click=move |_| access.submit(AuthCommand::Logout)>"Cancelar acesso e voltar"</button>
        </section>
    }
}

#[component]
fn MfaEnrollment() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <Show when=move || access.setup_key.get().is_some() fallback=move || view! {
            <form id="auth-enroll-form" method="post" action="/acesso" on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::StartMfa { password: entered("auth-setup-password", access.setup_password) }); }>
                <Input id="auth-setup-password" label="Confirme sua senha" value=access.setup_password kind="password" autocomplete="current-password" max_length=1024/>
                <Submit id="auth-enroll-submit" label=if access.context.get_untracked().mfa_enabled { "Preparar novo autenticador" } else { "Configurar verificação em duas etapas" }/>
            </form>
        }>
            <div id="auth-setup-instructions" class="access-secret">
                <h3>"Adicione uma conta no seu aplicativo autenticador"</h3>
                <p>"Escolha a opção de informar uma chave manualmente. Use o nome Canteiro 360 e o tipo baseado em tempo."</p>
                <Show when=move || access.context.get().mfa_enabled><p>"A proteção atual continua ativa até você confirmar o código do novo aplicativo. Ao concluir, os códigos de reserva antigos serão substituídos."</p></Show>
                <label for="auth-setup-key">"Chave de configuração"</label>
                <textarea id="auth-setup-key" rows="2" readonly spellcheck="false" autocapitalize="none" autocomplete="off"
                    prop:value=move || access.setup_key.get().unwrap_or_default() aria-describedby="auth-setup-help"></textarea>
                <p id="auth-setup-help">"Selecione e copie a chave para o aplicativo. Não compartilhe nem envie capturas desta tela. A configuração só termina depois da confirmação abaixo."</p>
            </div>
            <form id="auth-confirm-mfa-form" method="post" action="/acesso" on:submit=move |ev| { ev.prevent_default(); access.submit(AuthCommand::ConfirmMfa { code: entered("auth-confirm-code", access.code) }); }>
                <Input id="auth-confirm-code" label=if access.context.get_untracked().mfa_enabled { "Código do novo aplicativo" } else { "Código gerado no aplicativo" } value=access.code autocomplete="one-time-code" inputmode="numeric" max_length=16/>
                <Submit id="auth-confirm-mfa-submit" label=if access.context.get_untracked().mfa_enabled { "Confirmar novo autenticador" } else { "Ativar verificação em duas etapas" }/>
            </form>
        </Show>
    }
}

#[component]
pub(super) fn Account() -> impl IntoView {
    let access = expect_context::<Access>();
    view! {
        <section class="section-card access-card" aria-labelledby="auth-account-title">
            <div class="access-account-heading"><h2 id="auth-account-title" tabindex="-1">"Você entrou na sua conta"</h2><span class="status-badge success">"Acesso ativo"</span></div>
            <p id="auth-account-email" class="access-account-email">{move || access.context.get().email.unwrap_or_default()}</p>
            <p>"Confira a proteção da conta e as sessões abertas. O acesso às organizações e obras será disponibilizado nas próximas etapas."</p>
            <div class="access-links"><button id="auth-refresh" type="button" class="secondary-button" disabled=move || access.busy.get() on:click=move |_| access.load()>"Atualizar sessões"</button>
                <button id="auth-logout" type="button" class="secondary-button" disabled=move || access.blocked() on:click=move |_| access.submit(AuthCommand::Logout)>"Sair desta sessão"</button></div>
        </section>
        <section class="section-card access-card" aria-labelledby="auth-protection-title">
            <h2 id="auth-protection-title">"Proteção da conta"</h2>
            <Show when=move || access.context.get().mfa_enabled fallback=move || view! {
                <p>"Adicione a verificação em duas etapas para exigir também o código do seu aplicativo autenticador ao entrar."</p>
                <MfaEnrollment/>
            }>
                <p class="access-protected"><strong>"Verificação em duas etapas ativa"</strong></p>
                <p>"Sua senha e um código do aplicativo protegem o acesso. A recuperação da senha mantém esta proteção."</p>
                <details id="auth-replace-details" class="access-details"><summary>"Substituir aplicativo autenticador"</summary>
                    <p>"Entre novamente e confirme com seu aplicativo ou um código de reserva. Depois informe sua senha e cadastre o novo autenticador. Sua proteção atual continua ativa até confirmar o novo código."</p>
                    <p>"A confirmação de acesso deve ter ocorrido nos últimos cinco minutos. Concluir a troca encerra as outras sessões e gera novos códigos de reserva."</p>
                    <MfaEnrollment/>
                </details>
            </Show>
            <Show when=move || !access.recovery_codes.get().is_empty()>
                <div id="auth-recovery-codes" class="access-secret" tabindex="-1">
                    <h3>"Guarde seus códigos de reserva agora"</h3>
                    <p>"Eles não serão mostrados novamente. Guarde-os no seu gerenciador de senhas ou em outro local seguro. Cada código só pode ser usado uma vez."</p>
                    <label for="auth-recovery-copy">"Códigos de reserva para copiar"</label>
                    <textarea id="auth-recovery-copy" rows="10" readonly spellcheck="false" autocapitalize="none" autocomplete="off"
                        prop:value=move || access.recovery_codes.get().join("\n")></textarea>
                    <button id="auth-hide-codes" type="button" class="secondary-button" on:click=move |_| { access.recovery_codes.set(vec![]); access.message.set("Códigos ocultados. Guarde a sua cópia em local seguro.".into()); }>"Já guardei, ocultar códigos"</button>
                </div>
            </Show>
            <Show when=move || access.context.get().privileged>
                <button id="auth-check-privileged" type="button" class="secondary-button" disabled=move || access.blocked()
                    on:click=move |_| access.submit(AuthCommand::PrivilegedCheck)>"Conferir acesso protegido"</button>
            </Show>
        </section>
        <section class="section-card access-card" aria-labelledby="auth-sessions-title">
            <h2 id="auth-sessions-title">"Sessões abertas"</h2>
            <p>"Encerre qualquer sessão que você não reconheça. Ela precisará entrar novamente antes da próxima operação."</p>
            <ul id="auth-sessions" class="access-sessions">{move || access.context.get().sessions.into_iter().map(|session| {
                let id = session.id.clone();
                view! { <li data-session-id=session.id>
                    <div><strong>{if session.current { "Esta sessão" } else { "Outra sessão" }}</strong>
                        <p>{format!("Início: {}", display_instant(&session.created_at))}</p>
                        <p>{format!("Validade máxima: {}", display_instant(&session.expires_at))}</p></div>
                    <button type="button" class="secondary-button" aria-label=if session.current { "Encerrar esta sessão" } else { "Encerrar outra sessão" }
                        disabled=move || access.blocked() on:click=move |_| access.submit(AuthCommand::RevokeSession { session_id: id.clone() })>"Encerrar"</button>
                </li> }
            }).collect_view()}</ul>
            <details id="auth-revoke-all-details" class="access-details"><summary>"Encerrar todas as sessões"</summary>
                <p>"Você também sairá desta sessão. Todos os acessos precisarão informar novamente a senha e, quando ativa, a verificação em duas etapas."</p>
                <button id="auth-revoke-all" type="button" class="secondary-button" disabled=move || access.blocked()
                    on:click=move |_| access.submit(AuthCommand::RevokeAll)>"Confirmar encerramento de todas"</button>
            </details>
        </section>
    }
}

fn display_instant(value: &str) -> String {
    // Explicit UTC keeps the label accurate on SSR and on every device.
    if let Some(date) = value.get(..10)
        && let Some(time) = value.get(11..16)
        && date.is_ascii()
        && time.is_ascii()
        && date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-')
        && (value.ends_with('Z') || value.ends_with("+00:00"))
    {
        return format!(
            "{}/{}/{} às {} (UTC)",
            &date[8..10],
            &date[5..7],
            &date[..4],
            time
        );
    }
    value.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_labels_do_not_invent_a_local_timezone() {
        assert_eq!(
            display_instant("2026-11-13T12:30:00Z"),
            "13/11/2026 às 12:30 (UTC)"
        );
        assert_eq!(
            display_instant("2026-11-13T12:30:00-03:00"),
            "2026-11-13T12:30:00-03:00"
        );
    }
}
