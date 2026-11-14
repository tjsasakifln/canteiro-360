//! Autenticação do servidor. A sessão identifica a pessoa; não concede acesso a
//! qualquer organização ou obra. Esses vínculos são revalidados pelos módulos consumidores.
mod crypto;
pub mod organizations;

use canteiro_contracts::auth::{AuthCommand, AuthContext, AuthState, SessionSummary};
use canteiro_persistence::{
    Database,
    auth::{AuthStore, AuthStoreError, AuthTransaction, StoredSession, StoredUser},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthErrorKind {
    InvalidInput,
    Unauthorized,
    Forbidden,
    Conflict,
    RateLimited,
    Unavailable,
}
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct AuthError {
    kind: AuthErrorKind,
    code: &'static str,
    message: &'static str,
}
impl AuthError {
    fn new(kind: AuthErrorKind, code: &'static str, message: &'static str) -> Self {
        Self {
            kind,
            code,
            message,
        }
    }
    fn unavailable() -> Self {
        Self::new(
            AuthErrorKind::Unavailable,
            "unavailable",
            "Não foi possível concluir agora. Seu formulário foi preservado. Tente novamente.",
        )
    }
    fn expired() -> Self {
        Self::new(
            AuthErrorKind::Unauthorized,
            "session_expired",
            "Sua sessão expirou ou foi encerrada. Entre novamente para continuar. Seu formulário foi preservado.",
        )
    }
    fn credentials() -> Self {
        Self::new(
            AuthErrorKind::Unauthorized,
            "invalid_credentials",
            "Não foi possível entrar. Confira suas credenciais e a verificação do e-mail.",
        )
    }
    fn code_invalid() -> Self {
        Self::new(
            AuthErrorKind::Unauthorized,
            "code_invalid",
            "Código inválido, expirado ou já utilizado. Confira o código ou solicite outro.",
        )
    }
    fn forbidden() -> Self {
        Self::new(
            AuthErrorKind::Forbidden,
            "forbidden",
            "Esta operação exige acesso autorizado e confirmação em duas etapas.",
        )
    }
    pub fn kind(&self) -> AuthErrorKind {
        self.kind
    }
    pub fn code(&self) -> &'static str {
        self.code
    }
    pub fn message(&self) -> &'static str {
        self.message
    }
}
impl From<AuthStoreError> for AuthError {
    fn from(_: AuthStoreError) -> Self {
        Self::unavailable()
    }
}

pub struct AuthResult {
    pub context: AuthContext,
    pub session_token: String,
    pub message: String,
    pub mfa_setup: Option<String>,
    pub recovery_codes: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct MailPayload {
    email: String,
    kind: String,
    token: String,
}
pub struct MailDelivery {
    pub id: String,
    pub email: String,
    pub kind: String,
    pub token: String,
    pub expires_at: String,
}
#[derive(Clone)]
pub struct AuthService {
    store: AuthStore,
    key: Arc<Zeroizing<[u8; 32]>>,
    password_slots: Arc<Semaphore>,
    dummy_hash: Arc<String>,
}

const EMAIL_MESSAGE: &str = "Se o endereço puder receber esta solicitação, enviaremos um código válido por 15 minutos. Confira também a pasta de spam.";
const SESSION_SECONDS: i32 = 8 * 60 * 60;
const PENDING_SECONDS: i32 = 10 * 60;
const ANONYMOUS_SECONDS: i32 = 30 * 60;

impl AuthService {
    pub async fn new(database: Database, key: [u8; 32]) -> Result<Self, AuthError> {
        let dummy = tokio::task::spawn_blocking(|| crypto::password_hash(&crypto::random_token()))
            .await
            .map_err(|_| AuthError::unavailable())??;
        Ok(Self {
            store: AuthStore::new(database),
            key: Arc::new(Zeroizing::new(key)),
            password_slots: Arc::new(Semaphore::new(2)),
            dummy_hash: Arc::new(dummy),
        })
    }
    pub fn csrf(&self, token: &str) -> String {
        crypto::mac(self.key.as_ref().as_ref(), "csrf-v1", token)
    }
    pub fn verify_csrf(&self, token: &str, csrf: &str) -> bool {
        crypto::token_valid(token) && crypto::equal(&self.csrf(token), csrf)
    }
    /// Usado pela fronteira HTTP antes de criar sessões anônimas. O endereço
    /// vem do peer/proxy confiável, nunca de cabeçalho livre enviado pelo visitante.
    pub async fn allow_context(&self, network: &str) -> Result<(), AuthError> {
        self.limit("context", network, 120, 60).await
    }
    async fn limit(
        &self,
        scope: &str,
        value: &str,
        max: i32,
        seconds: i32,
    ) -> Result<(), AuthError> {
        let bucket = crypto::hash(
            "rate",
            &crypto::mac(self.key.as_ref().as_ref(), scope, value),
        );
        if self.store.attempt(&bucket, max, seconds).await? {
            Ok(())
        } else {
            Err(AuthError::new(
                AuthErrorKind::RateLimited,
                "rate_limited",
                "Foram feitas várias tentativas. Aguarde alguns minutos e tente novamente.",
            ))
        }
    }
    async fn hash_password(&self, password: String) -> Result<String, AuthError> {
        crypto::policy_password(&password)?;
        let permit = self
            .password_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                AuthError::new(
                    AuthErrorKind::RateLimited,
                    "rate_limited",
                    "O acesso está ocupado. Aguarde um momento e tente novamente.",
                )
            })?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let password = Zeroizing::new(password);
            crypto::password_hash(&password)
        })
        .await
        .map_err(|_| AuthError::unavailable())?
    }
    async fn check_password(
        &self,
        password: String,
        hash: String,
    ) -> Result<(bool, Option<String>), AuthError> {
        // Mesmo teto de entrada antes de executar uma função de custo elevado.
        if password.len() > 512 {
            return Err(AuthError::credentials());
        }
        let permit = self
            .password_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                AuthError::new(
                    AuthErrorKind::RateLimited,
                    "rate_limited",
                    "O acesso está ocupado. Aguarde um momento e tente novamente.",
                )
            })?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let password = Zeroizing::new(password);
            let (valid, rehash) = crypto::password_verify(&password, &hash)?;
            let replacement = if valid && rehash {
                Some(crypto::password_rehash(&password, &hash)?)
            } else {
                None
            };
            Ok((valid, replacement))
        })
        .await
        .map_err(|_| AuthError::unavailable())?
    }
    async fn locked(
        &self,
        tx: &mut AuthTransaction,
        token: &str,
    ) -> Result<(StoredSession, Option<StoredUser>), AuthError> {
        if !crypto::token_valid(token) {
            return Err(AuthError::expired());
        }
        let hash = crypto::hash("session", token);
        let before = tx
            .session(&hash, false)
            .await?
            .ok_or_else(AuthError::expired)?;
        // A identidade é sempre bloqueada antes das suas sessões. Reset/revogação
        // e MFA usam a mesma ordem e voltam a validar a sessão após o bloqueio.
        let user = match before.user_id {
            Some(id) => Some(tx.user(id).await?.ok_or_else(AuthError::expired)?),
            None => None,
        };
        let session = tx
            .session(&hash, true)
            .await?
            .ok_or_else(AuthError::expired)?;
        if session.user_id != before.user_id {
            return Err(AuthError::expired());
        }
        if let Some(u) = &user
            && (!u.verified
                || session.password_version != Some(u.password_version)
                || (session.stage == "authenticated"
                    && (u.privileged || u.mfa_secret.is_some())
                    && !session.mfa_verified))
        {
            return Err(AuthError::expired());
        }
        Ok((session, user))
    }
    async fn new_session(
        &self,
        tx: &mut AuthTransaction,
        user: Option<&StoredUser>,
        mfa: bool,
    ) -> Result<String, AuthError> {
        let token = crypto::random_token();
        let (stage, seconds) = match user {
            None => ("anonymous", ANONYMOUS_SECONDS),
            Some(u) if (u.privileged || u.mfa_secret.is_some()) && !mfa => {
                ("mfa_pending", PENDING_SECONDS)
            }
            Some(_) => ("authenticated", SESSION_SECONDS),
        };
        tx.insert_session(
            Uuid::new_v4(),
            &crypto::hash("session", &token),
            user.map(|u| (u.id, u.password_version)),
            stage,
            mfa,
            seconds,
        )
        .await?;
        Ok(token)
    }
    async fn view(&self, tx: &mut AuthTransaction, token: &str) -> Result<AuthContext, AuthError> {
        let s = tx
            .session(&crypto::hash("session", token), false)
            .await?
            .ok_or_else(AuthError::expired)?;
        if let Some(id) = s.user_id {
            let u = tx.user(id).await?.ok_or_else(AuthError::expired)?;
            let sessions = if s.stage == "authenticated" {
                tx.sessions(id)
                    .await?
                    .into_iter()
                    .map(|v| SessionSummary {
                        id: v.id.to_string(),
                        current: v.id == s.id,
                        created_at: v.created_at.to_rfc3339(),
                        expires_at: v.expires_at.to_rfc3339(),
                    })
                    .collect()
            } else {
                vec![]
            };
            Ok(AuthContext {
                state: if s.stage == "authenticated" {
                    AuthState::Authenticated
                } else {
                    AuthState::MfaPending
                },
                email: Some(u.email),
                privileged: u.privileged,
                mfa_enabled: u.mfa_secret.is_some(),
                sessions,
            })
        } else {
            Ok(AuthContext {
                state: AuthState::Anonymous,
                email: None,
                privileged: false,
                mfa_enabled: false,
                sessions: vec![],
            })
        }
    }
    pub async fn context_limited(
        &self,
        token: Option<&str>,
        network: &str,
    ) -> Result<AuthResult, AuthError> {
        self.allow_context(network).await?;
        self.context(token).await
    }
    pub async fn context(&self, token: Option<&str>) -> Result<AuthResult, AuthError> {
        let mut tx = self.store.begin().await?;
        let token = if let Some(token) = token {
            match self.locked(&mut tx, token).await {
                Ok((s, _)) => {
                    tx.touch(s.id).await?;
                    token.to_owned()
                }
                Err(error) if error.code() == "session_expired" => {
                    self.new_session(&mut tx, None, false).await?
                }
                Err(error) => return Err(error),
            }
        } else {
            self.new_session(&mut tx, None, false).await?
        };
        let context = self.view(&mut tx, &token).await?;
        tx.commit().await?;
        Ok(AuthResult {
            context,
            session_token: token,
            message: String::new(),
            mfa_setup: None,
            recovery_codes: vec![],
        })
    }
    fn authenticated<'a>(
        s: &StoredSession,
        u: &'a Option<StoredUser>,
    ) -> Result<&'a StoredUser, AuthError> {
        if s.stage != "authenticated" {
            return Err(AuthError::forbidden());
        }
        u.as_ref().ok_or_else(AuthError::forbidden)
    }
    fn identified<'a>(
        s: &StoredSession,
        u: &'a Option<StoredUser>,
    ) -> Result<&'a StoredUser, AuthError> {
        if !matches!(s.stage.as_str(), "authenticated" | "mfa_pending") {
            return Err(AuthError::forbidden());
        }
        u.as_ref().ok_or_else(AuthError::forbidden)
    }
    async fn send_code(
        &self,
        tx: &mut AuthTransaction,
        u: &StoredUser,
        kind: &str,
    ) -> Result<(), AuthError> {
        let token = crypto::random_token();
        let token_id = tx
            .issue_token(u.id, kind, &crypto::hash(kind, &token))
            .await?;
        let id = Uuid::new_v4();
        let payload = Zeroizing::new(
            serde_json::to_vec(&MailPayload {
                email: u.email.clone(),
                kind: kind.into(),
                token,
            })
            .map_err(|_| AuthError::unavailable())?,
        );
        let ciphertext = crypto::seal(self.key.as_ref(), &format!("mail:{id}:v1"), &payload)?;
        tx.enqueue_mail(id, token_id, &ciphertext).await?;
        Ok(())
    }
    pub async fn pending_mail(&self, limit: i64) -> Result<Vec<MailDelivery>, AuthError> {
        let mut result = vec![];
        for mail in self.store.claim_mail(limit).await? {
            let plain = crypto::open(
                self.key.as_ref(),
                &format!("mail:{}:v1", mail.id),
                &mail.payload,
            )?;
            let payload: MailPayload =
                serde_json::from_slice(&plain).map_err(|_| AuthError::unavailable())?;
            result.push(MailDelivery {
                id: format!("{}:{}", mail.id, mail.lease_id),
                email: payload.email,
                kind: payload.kind,
                token: payload.token,
                expires_at: mail.expires_at.to_rfc3339(),
            });
        }
        Ok(result)
    }
    pub async fn mark_mail_delivered(&self, id: &str) -> Result<(), AuthError> {
        let (id, lease) = id.split_once(':').ok_or_else(AuthError::unavailable)?;
        self.store
            .delivered(
                Uuid::parse_str(id).map_err(|_| AuthError::unavailable())?,
                Uuid::parse_str(lease).map_err(|_| AuthError::unavailable())?,
            )
            .await?;
        Ok(())
    }
    pub async fn execute(
        &self,
        token: &str,
        command: AuthCommand,
        network: &str,
    ) -> Result<AuthResult, AuthError> {
        let deferred = matches!(
            &command,
            AuthCommand::RequestReset { .. } | AuthCommand::RequestVerification { .. }
        );
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
        let result = self.execute_inner(token, command, network).await;
        if deferred {
            tokio::time::sleep_until(deadline).await;
        }
        result
    }
    async fn execute_inner(
        &self,
        token: &str,
        command: AuthCommand,
        network: &str,
    ) -> Result<AuthResult, AuthError> {
        if !crypto::token_valid(token) {
            return Err(AuthError::expired());
        }
        self.limit("command-network", network, 100, 900).await?;
        let mut preflight = self.store.begin().await?;
        self.locked(&mut preflight, token).await?;
        preflight.commit().await?;
        // Costly work occurs outside the transaction; credential versions are
        // rechecked with locks before any session can be created.
        let requesting_reset = matches!(&command, AuthCommand::RequestReset { .. });
        let mut credential_snapshot: Option<StoredUser> = None;
        let mut checked = false;
        let mut password_replacement = None;
        let mut canonical_email = None;
        match &command {
            AuthCommand::Register { email, password } => {
                let e = crypto::email(email)?;
                self.limit("register-email", &e, 3, 900).await?;
                canonical_email = Some(e);
                password_replacement = Some(self.hash_password(password.clone()).await?);
            }
            AuthCommand::Login { email, password } => {
                let e = crypto::email(email)?;
                self.limit("login-email", &e, 8, 900).await?;
                credential_snapshot = self.store.user_by_email(&e).await?;
                let h = credential_snapshot
                    .as_ref()
                    .map(|u| u.password_hash.clone())
                    .unwrap_or_else(|| self.dummy_hash.as_ref().clone());
                (checked, password_replacement) = self.check_password(password.clone(), h).await?;
                canonical_email = Some(e);
            }
            AuthCommand::RequestReset { email } | AuthCommand::RequestVerification { email } => {
                let e = crypto::email(email)?;
                self.limit("email-code", &e, 3, 900).await?;
                canonical_email = Some(e);
            }
            AuthCommand::ResetPassword { password, .. }
            | AuthCommand::VerifyEmail { password, .. } => {
                password_replacement = Some(self.hash_password(password.clone()).await?);
            }
            AuthCommand::StartMfa { password } => {
                credential_snapshot = self
                    .store
                    .user_by_session(&crypto::hash("session", token))
                    .await?;
                let id = credential_snapshot
                    .as_ref()
                    .map(|u| u.id.to_string())
                    .unwrap_or_else(|| "anonymous".into());
                self.limit("mfa-password", &id, 5, 300).await?;
                let h = credential_snapshot
                    .as_ref()
                    .map(|u| u.password_hash.clone())
                    .unwrap_or_else(|| self.dummy_hash.as_ref().clone());
                (checked, _) = self.check_password(password.clone(), h).await?;
            }
            AuthCommand::VerifyMfa { .. }
            | AuthCommand::ConfirmMfa { .. }
            | AuthCommand::RecoverMfa { .. } => {
                // Bind attempts to the account, not a freshly requested session.
                let u = self
                    .store
                    .user_by_session(&crypto::hash("session", token))
                    .await?;
                let id = u
                    .as_ref()
                    .map(|u| u.id.to_string())
                    .unwrap_or_else(|| "anonymous".into());
                self.limit("mfa-code", &id, 6, 300).await?;
            }
            _ => {}
        }
        let mut tx = self.store.begin().await?;
        let (session, user) = self.locked(&mut tx, token).await?;
        let mut next_token = token.to_owned();
        let mut setup = None;
        let mut recovery = vec![];
        let message = match command {
            AuthCommand::Register { .. } => {
                if session.stage != "anonymous" {
                    return Err(AuthError::forbidden());
                }
                let id = Uuid::new_v4();
                let email = canonical_email
                    .as_deref()
                    .ok_or_else(AuthError::unavailable)?;
                if tx
                    .insert_user(
                        id,
                        email,
                        password_replacement
                            .as_deref()
                            .ok_or_else(AuthError::unavailable)?,
                    )
                    .await?
                {
                    let u = tx.user(id).await?.ok_or_else(AuthError::unavailable)?;
                    self.send_code(&mut tx, &u, "verify_email").await?;
                    tx.audit(Some(id), "registered").await?;
                }
                EMAIL_MESSAGE
            }
            AuthCommand::RequestVerification { .. } | AuthCommand::RequestReset { .. } => {
                if session.stage != "anonymous" {
                    return Err(AuthError::forbidden());
                }
                let email = canonical_email
                    .as_deref()
                    .ok_or_else(AuthError::unavailable)?;
                let kind = if requesting_reset {
                    "reset_password"
                } else {
                    "verify_email"
                };
                if let Some(u) = tx.user_by_email(email).await?
                    && ((kind == "verify_email" && !u.verified)
                        || (kind == "reset_password" && u.verified))
                {
                    self.send_code(&mut tx, &u, kind).await?;
                }
                tx.audit(
                    None,
                    if requesting_reset {
                        "reset_requested"
                    } else {
                        "verification_requested"
                    },
                )
                .await?;
                EMAIL_MESSAGE
            }
            AuthCommand::VerifyEmail { token: code, .. } => {
                if session.stage != "anonymous" {
                    return Err(AuthError::forbidden());
                }
                if !crypto::token_valid(&code) {
                    return Err(AuthError::code_invalid());
                }
                let t = tx
                    .token(&crypto::hash("verify_email", &code), "verify_email")
                    .await?
                    .ok_or_else(AuthError::code_invalid)?;
                tx.user(t.user_id)
                    .await?
                    .ok_or_else(AuthError::code_invalid)?;
                if !tx.consume_token(t.id).await? {
                    return Err(AuthError::code_invalid());
                }
                tx.set_password(
                    t.user_id,
                    password_replacement
                        .as_deref()
                        .ok_or_else(AuthError::unavailable)?,
                    true,
                )
                .await?;
                tx.verify_email(t.user_id).await?;
                tx.revoke_all(t.user_id).await?;
                tx.audit(Some(t.user_id), "email_verified").await?;
                "E-mail verificado. Entre com sua senha para continuar."
            }
            AuthCommand::Login { .. } => {
                if session.stage != "anonymous" {
                    return Err(AuthError::forbidden());
                }
                let snapshot = credential_snapshot.as_ref();
                let current = match snapshot {
                    Some(u) => tx.user(u.id).await?,
                    None => None,
                };
                if !checked
                    || current.as_ref().is_none_or(|u| {
                        !u.verified
                            || snapshot.is_none_or(|old| {
                                old.password_version != u.password_version
                                    || old.password_hash != u.password_hash
                            })
                    })
                {
                    tx.audit(None, "login_failed").await?;
                    tx.commit().await?;
                    return Err(AuthError::credentials());
                }
                let u = current.as_ref().ok_or_else(AuthError::credentials)?;
                if let Some(hash) = password_replacement {
                    tx.set_password(u.id, &hash, false).await?;
                }
                tx.revoke(session.id).await?;
                next_token = self.new_session(&mut tx, Some(u), false).await?;
                tx.audit(Some(u.id), "login").await?;
                if u.privileged || u.mfa_secret.is_some() {
                    "Confirme a segunda etapa para concluir o acesso."
                } else {
                    "Acesso confirmado."
                }
            }
            AuthCommand::ResetPassword { token: code, .. } => {
                if session.stage != "anonymous" {
                    return Err(AuthError::forbidden());
                }
                if !crypto::token_valid(&code) {
                    return Err(AuthError::code_invalid());
                }
                let t = tx
                    .token(&crypto::hash("reset_password", &code), "reset_password")
                    .await?
                    .ok_or_else(AuthError::code_invalid)?;
                let u = tx
                    .user(t.user_id)
                    .await?
                    .ok_or_else(AuthError::code_invalid)?;
                if !u.verified || !tx.consume_token(t.id).await? {
                    return Err(AuthError::code_invalid());
                }
                tx.set_password(
                    u.id,
                    password_replacement
                        .as_deref()
                        .ok_or_else(AuthError::unavailable)?,
                    true,
                )
                .await?;
                tx.revoke_all(u.id).await?;
                tx.revoke(session.id).await?;
                next_token = self.new_session(&mut tx, None, false).await?;
                tx.audit(Some(u.id), "password_reset").await?;
                "Senha alterada e sessões anteriores encerradas. Entre novamente. A verificação em duas etapas permanece ativa."
            }
            AuthCommand::StartMfa { .. } => {
                let u = Self::identified(&session, &user)?;
                if !checked
                    || credential_snapshot.as_ref().is_none_or(|old| {
                        old.id != u.id
                            || old.password_version != u.password_version
                            || old.password_hash != u.password_hash
                    })
                {
                    return Err(AuthError::credentials());
                }
                let now = tx.now().await?;
                if u.mfa_secret.is_some()
                    && (session.stage != "authenticated"
                        || !session.mfa_verified
                        || session.created_at <= now - chrono::Duration::minutes(5))
                {
                    return Err(AuthError::forbidden());
                }
                let raw = crypto::mfa_secret();
                let cipher = crypto::seal(self.key.as_ref(), &format!("mfa:{}:v1", u.id), &raw)?;
                tx.pending_mfa(u.id, session.id, &cipher).await?;
                setup = Some(data_encoding::BASE32_NOPAD.encode(&raw));
                tx.audit(Some(u.id), "mfa_started").await?;
                if u.mfa_secret.is_some() {
                    "Adicione a nova chave ao autenticador e confirme um código nesta sessão. A troca exige confirmação de acesso feita nos últimos cinco minutos."
                } else {
                    "Adicione esta chave ao autenticador e confirme um código. A chave de configuração vale por dez minutos."
                }
            }
            AuthCommand::ConfirmMfa { code } => {
                let u = Self::identified(&session, &user)?;
                let now = tx.now().await?;
                if u.mfa_pending_until.is_none_or(|end| end <= now)
                    || u.mfa_pending_session != Some(session.id)
                {
                    return Err(AuthError::code_invalid());
                }
                if u.mfa_secret.is_some()
                    && (session.stage != "authenticated"
                        || !session.mfa_verified
                        || session.created_at <= now - chrono::Duration::minutes(5))
                {
                    return Err(AuthError::forbidden());
                }
                let cipher = u.mfa_pending.as_ref().ok_or_else(AuthError::code_invalid)?;
                let raw = crypto::open(self.key.as_ref(), &format!("mfa:{}:v1", u.id), cipher)?;
                let step = crypto::verify_totp(&raw, &code, now.timestamp(), -1)
                    .ok_or_else(AuthError::code_invalid)?;
                tx.enable_mfa(u.id, cipher, step).await?;
                recovery = (0..10).map(|_| crypto::random_token()).collect();
                let hashes = recovery
                    .iter()
                    .map(|c| crypto::hash("recovery", c))
                    .collect::<Vec<_>>();
                tx.recovery_codes(u.id, &hashes).await?;
                tx.revoke_all(u.id).await?;
                let updated = tx.user(u.id).await?.ok_or_else(AuthError::unavailable)?;
                next_token = self.new_session(&mut tx, Some(&updated), true).await?;
                tx.audit(
                    Some(u.id),
                    if u.mfa_secret.is_some() {
                        "mfa_replaced"
                    } else {
                        "mfa_enabled"
                    },
                )
                .await?;
                "Verificação em duas etapas ativada. Guarde os códigos de reserva em local seguro; cada um pode ser usado uma vez."
            }
            AuthCommand::VerifyMfa { code } => {
                let u = Self::identified(&session, &user)?;
                if session.stage != "mfa_pending" {
                    return Err(AuthError::forbidden());
                }
                let cipher = u.mfa_secret.as_ref().ok_or_else(AuthError::code_invalid)?;
                let raw = crypto::open(self.key.as_ref(), &format!("mfa:{}:v1", u.id), cipher)?;
                let now = tx.now().await?;
                let step = match crypto::verify_totp(&raw, &code, now.timestamp(), u.mfa_last_step)
                {
                    Some(s) => s,
                    None => {
                        tx.audit(Some(u.id), "mfa_failed").await?;
                        tx.commit().await?;
                        return Err(AuthError::code_invalid());
                    }
                };
                tx.record_mfa_step(u.id, step).await?;
                tx.revoke(session.id).await?;
                next_token = self.new_session(&mut tx, Some(u), true).await?;
                tx.audit(Some(u.id), "mfa_verified").await?;
                "Acesso confirmado em duas etapas."
            }
            AuthCommand::RecoverMfa { code } => {
                let u = Self::identified(&session, &user)?;
                if session.stage != "mfa_pending" || u.mfa_secret.is_none() {
                    return Err(AuthError::forbidden());
                }
                if !crypto::token_valid(code.trim())
                    || !tx
                        .consume_recovery(u.id, &crypto::hash("recovery", code.trim()))
                        .await?
                {
                    tx.audit(Some(u.id), "mfa_failed").await?;
                    tx.commit().await?;
                    return Err(AuthError::code_invalid());
                }
                tx.revoke(session.id).await?;
                next_token = self.new_session(&mut tx, Some(u), true).await?;
                tx.audit(Some(u.id), "mfa_recovered").await?;
                "Acesso confirmado com código de reserva. Este código não poderá ser usado novamente. Seu autenticador permanece ativo."
            }
            AuthCommand::Logout => {
                tx.revoke(session.id).await?;
                tx.audit(session.user_id, "logout").await?;
                next_token = self.new_session(&mut tx, None, false).await?;
                "Você saiu desta sessão."
            }
            AuthCommand::RevokeSession { session_id } => {
                let u = Self::authenticated(&session, &user)?;
                let id = Uuid::parse_str(&session_id).map_err(|_| AuthError::forbidden())?;
                if !tx.revoke_owned(id, u.id).await? {
                    return Err(AuthError::forbidden());
                }
                if id == session.id {
                    next_token = self.new_session(&mut tx, None, false).await?;
                }
                tx.audit(Some(u.id), "session_revoked").await?;
                "Sessão encerrada. Seu próximo comando será recusado."
            }
            AuthCommand::RevokeAll => {
                let u = Self::authenticated(&session, &user)?;
                tx.revoke_all(u.id).await?;
                tx.audit(Some(u.id), "sessions_revoked").await?;
                next_token = self.new_session(&mut tx, None, false).await?;
                "Todas as suas sessões foram encerradas. Entre novamente para continuar."
            }
            AuthCommand::PrivilegedCheck => {
                let u = Self::authenticated(&session, &user)?;
                if !u.privileged || !session.mfa_verified || u.mfa_secret.is_none() {
                    return Err(AuthError::forbidden());
                }
                tx.audit(Some(u.id), "privileged_checked").await?;
                "Acesso privilegiado confirmado em duas etapas. Nenhuma operação financeira foi executada."
            }
        };
        if next_token == token {
            tx.touch(session.id).await?;
        }
        let context = self.view(&mut tx, &next_token).await?;
        tx.commit().await?;
        Ok(AuthResult {
            context,
            session_token: next_token,
            message: message.into(),
            mfa_setup: setup,
            recovery_codes: recovery,
        })
    }
}

#[cfg(test)]
mod tests;
