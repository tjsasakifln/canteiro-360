//! Fronteira HTTP de identidade. Cookies são a única fonte da sessão.
use axum::{
    Extension, Json, Router,
    extract::{
        ConnectInfo, DefaultBodyLimit,
        rejection::{ExtensionRejection, JsonRejection},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use canteiro_application::auth::{AuthError, AuthErrorKind, AuthResult, AuthService};
use canteiro_contracts::auth::{AuthCommand, AuthFailure, AuthResponse};
use canteiro_persistence::Database;
use std::{net::SocketAddr, time::Duration};

use crate::config::{Config, Environment};

const COOKIE_NAME: &str = "__Host-canteiro-session";

/// Não implementa Debug: carrega material exclusivamente do servidor.
#[derive(Clone)]
pub struct AuthHttp {
    service: AuthService,
    origin: String,
}

impl AuthHttp {
    pub fn new(service: AuthService, origin: String) -> Self {
        Self { service, origin }
    }

    pub async fn from_env(database: Database, config: &Config) -> Result<Self, String> {
        let encoded = std::env::var("CANTEIRO_AUTH_KEY")
            .map_err(|_| "Injete a chave externa de autenticação antes de iniciar.".to_owned())?;
        let key: [u8; 32] = URL_SAFE_NO_PAD
            .decode(encoded.trim())
            .map_err(|_| {
                "A chave externa de autenticação deve conter 32 bytes em base64url.".to_owned()
            })?
            .try_into()
            .map_err(|_| {
                "A chave externa de autenticação deve conter 32 bytes em base64url.".to_owned()
            })?;
        if key == [0; 32] {
            return Err("Use uma chave aleatória gerada fora do repositório.".into());
        }
        let raw_origin = std::env::var("CANTEIRO_PUBLIC_ORIGIN")
            .unwrap_or_else(|_| format!("http://{}", config.bind_addr));
        let origin = validate_origin(&raw_origin, config.environment)?;
        let service = AuthService::new(database, key)
            .await
            .map_err(|_| "Não foi possível preparar a autenticação.".to_owned())?;
        Ok(Self { service, origin })
    }

    pub fn service(&self) -> &AuthService {
        &self.service
    }

    /// Entrega somente quando um provedor externo explícito está configurado.
    /// A fila permanece cifrada e retomável se o provedor estiver indisponível.
    pub fn start_mail_worker(
        &self,
        environment: Environment,
    ) -> Result<Option<tokio::task::JoinHandle<()>>, String> {
        let Some(delivery) = MailTransport::from_env(environment, &self.origin)? else {
            if environment == Environment::Homologation {
                return Err(
                    "Configure o serviço de e-mail para habilitar a autenticação em homologação."
                        .into(),
                );
            }
            tracing::warn!(
                "Entrega de e-mail não configurada no ambiente de desenvolvimento; mensagens permanecem protegidas na fila."
            );
            return Ok(None);
        };
        let service = self.service.clone();
        Ok(Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                let Ok(messages) = service.pending_mail(1).await else {
                    tracing::warn!("Não foi possível consultar a fila de acesso.");
                    continue;
                };
                for message in messages {
                    if delivery.send(&message).await {
                        if service.mark_mail_delivered(&message.id).await.is_err() {
                            tracing::warn!("Entrega de acesso aguarda confirmação na fila.");
                        }
                    } else {
                        tracing::warn!(
                            "Provedor de e-mail indisponível; a entrega será tentada novamente."
                        );
                    }
                }
            }
        })))
    }
}

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/auth/context", get(context))
        .route("/api/auth/command", post(command))
        .layer(DefaultBodyLimit::max(8_192))
}

async fn context(
    auth: Option<Extension<AuthHttp>>,
    peer: Result<ConnectInfo<SocketAddr>, ExtensionRejection>,
    headers: HeaderMap,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    // Refuse cross-site context establishment even though GET never authenticates.
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v != "same-origin" && v != "none")
        || headers
            .get(header::ORIGIN)
            .is_some_and(|v| v.as_bytes() != auth.origin.as_bytes())
    {
        return failure(
            StatusCode::FORBIDDEN,
            "origin_invalid",
            "Abra a página de acesso neste site e tente novamente.",
        );
    }
    let token = match session_cookie(&headers) {
        Ok(token) => token,
        Err(()) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "session_invalid",
                "Abra uma nova página de acesso para continuar.",
            );
        }
    };
    match auth
        .service
        .context_limited(token.as_deref(), &network(peer))
        .await
    {
        Ok(result) => success(&auth, result),
        Err(error) => application_failure(error),
    }
}

async fn command(
    auth: Option<Extension<AuthHttp>>,
    peer: Result<ConnectInfo<SocketAddr>, ExtensionRejection>,
    headers: HeaderMap,
    input: Result<Json<AuthCommand>, JsonRejection>,
) -> Response {
    let Some(Extension(auth)) = auth else {
        return unavailable();
    };
    // Exact scheme, host and port, independent from Host or forwarded headers.
    if headers.get_all(header::ORIGIN).iter().count() != 1
        || headers
            .get(header::ORIGIN)
            .is_none_or(|v| v.as_bytes() != auth.origin.as_bytes())
        || headers
            .get("sec-fetch-site")
            .is_some_and(|v| v != "same-origin")
    {
        return failure(
            StatusCode::FORBIDDEN,
            "origin_invalid",
            "Esta solicitação não veio da página de acesso. Abra a página e tente novamente.",
        );
    }
    let token = match session_cookie(&headers) {
        Ok(Some(value)) => value,
        _ => {
            return failure(
                StatusCode::UNAUTHORIZED,
                "session_expired",
                "Seu acesso expirou. Restabeleça o acesso e tente novamente; mantenha o formulário aberto.",
            );
        }
    };
    if headers.get_all("x-csrf-token").iter().count() != 1
        || !headers
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|value| auth.service.verify_csrf(&token, value))
    {
        return failure(
            StatusCode::FORBIDDEN,
            "csrf_invalid",
            "Restabeleça o acesso antes de tentar novamente. Os campos podem ser mantidos abertos.",
        );
    }
    let Json(input) = match input {
        Ok(value) => value,
        Err(_) => {
            return failure(
                StatusCode::BAD_REQUEST,
                "invalid_input",
                "Confira os campos informados e tente novamente.",
            );
        }
    };
    match auth.service.execute(&token, input, &network(peer)).await {
        Ok(result) => success(&auth, result),
        Err(error) => application_failure(error),
    }
}

fn network(peer: Result<ConnectInfo<SocketAddr>, ExtensionRejection>) -> String {
    peer.map(|ConnectInfo(peer)| peer.ip().to_string())
        .unwrap_or_else(|_| "local-service".into())
}

fn session_cookie(headers: &HeaderMap) -> Result<Option<String>, ()> {
    let mut found = None;
    for header in headers.get_all(header::COOKIE) {
        for pair in header.to_str().map_err(|_| ())?.split(';') {
            let Some((name, value)) = pair.trim().split_once('=') else {
                continue;
            };
            if name == COOKIE_NAME {
                if found.is_some()
                    || value.len() != 43
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err(());
                }
                found = Some(value.to_owned());
            }
        }
    }
    Ok(found)
}

fn success(auth: &AuthHttp, result: AuthResult) -> Response {
    let csrf = auth.service.csrf(&result.session_token);
    let cookie = format!(
        "{COOKIE_NAME}={}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=28800",
        result.session_token
    );
    let Ok(cookie) = HeaderValue::from_str(&cookie) else {
        return unavailable();
    };
    let mut response = Json(AuthResponse {
        context: result.context,
        csrf,
        message: result.message,
        mfa_setup: result.mfa_setup,
        recovery_codes: result.recovery_codes,
    })
    .into_response();
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    private_headers(&mut response);
    response
}

fn application_failure(error: AuthError) -> Response {
    let status = match error.kind() {
        AuthErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        AuthErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        AuthErrorKind::Forbidden => StatusCode::FORBIDDEN,
        AuthErrorKind::RateLimited => StatusCode::TOO_MANY_REQUESTS,
        AuthErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    failure(status, error.code(), error.message())
}

fn unavailable() -> Response {
    failure(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "O acesso está temporariamente indisponível. Mantenha a página aberta e tente novamente.",
    )
}

fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    let mut response = (
        status,
        Json(AuthFailure {
            code: code.into(),
            message: message.into(),
        }),
    )
        .into_response();
    private_headers(&mut response);
    if status == StatusCode::TOO_MANY_REQUESTS {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("900"));
    }
    response
}

fn private_headers(response: &mut Response) {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::VARY, HeaderValue::from_static("Cookie, Origin"));
}

fn validate_origin(raw: &str, environment: Environment) -> Result<String, String> {
    let invalid = || {
        "Configure a origem pública completa, com HTTPS e sem caminho, credenciais ou parâmetros."
            .to_owned()
    };
    let url = reqwest::Url::parse(raw).map_err(|_| invalid())?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.host_str().is_none()
        || !(url.scheme() == "https"
            || environment == Environment::Development && local && url.scheme() == "http")
    {
        return Err(invalid());
    }
    Ok(url.origin().ascii_serialization())
}

struct MailTransport {
    client: reqwest::Client,
    endpoint: String,
    credential: String,
    sender: String,
    access_url: String,
}

impl MailTransport {
    fn from_env(environment: Environment, origin: &str) -> Result<Option<Self>, String> {
        let Ok(credential) = std::env::var("CANTEIRO_AUTH_MAIL_CREDENTIAL") else {
            return Ok(None);
        };
        let endpoint = std::env::var("CANTEIRO_AUTH_MAIL_ENDPOINT")
            .unwrap_or_else(|_| "https://api.resend.com/emails".into());
        let url = reqwest::Url::parse(&endpoint)
            .map_err(|_| "Configure o endereço do provedor de e-mail.".to_owned())?;
        let local = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !(url.scheme() == "https"
                || environment == Environment::Development && local && url.scheme() == "http")
        {
            return Err(
                "O provedor de e-mail precisa de HTTPS e endereço sem credenciais ou parâmetros."
                    .into(),
            );
        }
        let sender = std::env::var("CANTEIRO_AUTH_MAIL_FROM")
            .map_err(|_| "Configure o remetente verificado no provedor de e-mail.".to_owned())?;
        if sender.is_empty()
            || sender.len() > 320
            || sender.chars().any(char::is_control)
            || !sender.contains('@')
        {
            return Err("Configure um remetente de e-mail válido e verificado no provedor.".into());
        }
        if credential.is_empty() || HeaderValue::from_str(&format!("Bearer {credential}")).is_err()
        {
            return Err("A credencial externa do provedor de e-mail é inválida.".into());
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(3))
            .build()
            .map_err(|_| "Não foi possível configurar o provedor de e-mail.".to_owned())?;
        Ok(Some(Self {
            client,
            endpoint,
            credential,
            sender,
            access_url: format!("{origin}/acesso"),
        }))
    }

    async fn send(&self, message: &canteiro_application::auth::MailDelivery) -> bool {
        let delivery_id = message
            .id
            .split_once(':')
            .map_or(message.id.as_str(), |(id, _)| id);
        let result = self
            .client
            .post(&self.endpoint)
            .bearer_auth(&self.credential)
            .header("Idempotency-Key", delivery_id)
            .json(&self.payload(message))
            .send()
            .await;
        let Ok(response) = result else {
            return false;
        };
        if !response.status().is_success() {
            return false;
        }
        match response.json::<serde_json::Value>().await {
            Ok(body) => body
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| !id.is_empty() && id.len() <= 256),
            Err(_) => false,
        }
    }

    // Contrato oficial POST /emails da Resend. O corpo é determinístico em
    // todas as tentativas da mesma mensagem para preservar a idempotência.
    fn payload(&self, message: &canteiro_application::auth::MailDelivery) -> serde_json::Value {
        let (subject, purpose) = if message.kind == "verify_email" {
            (
                "Confirme seu e-mail no Canteiro 360",
                "confirmar seu e-mail e definir a senha da sua conta",
            )
        } else {
            (
                "Recupere seu acesso ao Canteiro 360",
                "definir uma nova senha",
            )
        };
        let body = format!(
            "Use o código abaixo para {purpose}:\n\n{}\n\nAbra {} e escolha a opção correspondente. Cole o código completo, sem colocá-lo no endereço da página.\n\nExpiração: {}. O código só pode ser usado uma vez. Se você solicitar outro, use a mensagem mais recente.\n\nA recuperação de senha não desativa a verificação em duas etapas quando ela estiver ativada. Nunca envie sua senha ou este código a outra pessoa.\n\nSe não fez esta solicitação, ignore a mensagem.\n\nCanteiro 360",
            message.token, self.access_url, message.expires_at,
        );
        serde_json::json!({ "from": self.sender, "to": [message.email], "subject": subject, "text": body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resend_protocol_retries_keep_same_payload_and_id_without_following_redirects() {
        use std::sync::Arc;
        use tokio::sync::Mutex;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = observed.clone();
        let provider = Router::new().route(
            "/emails",
            post(
                move |headers: HeaderMap, Json(body): Json<serde_json::Value>| {
                    let seen = seen.clone();
                    async move {
                        let mut seen = seen.lock().await;
                        seen.push((headers, body));
                        if seen.len() == 1 {
                            (
                                StatusCode::SERVICE_UNAVAILABLE,
                                Json(serde_json::json!({"error":"temporary"})),
                            )
                        } else {
                            (
                                StatusCode::OK,
                                Json(serde_json::json!({"id":"delivery-fixture"})),
                            )
                        }
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
        let transport = MailTransport {
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            endpoint: format!("http://{address}/emails"),
            credential: "fixture-only-provider".to_owned(),
            sender: "Canteiro 360 <access@example.test>".into(),
            access_url: "https://canteiro.example.test/acesso".into(),
        };
        let mut message = canteiro_application::auth::MailDelivery {
            id: "mail-uuid:first-lease".into(),
            email: "owner@example.test".into(),
            kind: "verify_email".into(),
            token: "fixture-only-code".into(),
            expires_at: "2026-11-13T16:15:00Z".into(),
        };
        assert!(!transport.send(&message).await);
        message.id = "mail-uuid:second-lease".into();
        assert!(transport.send(&message).await);
        let seen = observed.lock().await;
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].1, seen[1].1);
        for (headers, body) in seen.iter() {
            assert_eq!(headers["idempotency-key"], "mail-uuid");
            assert_eq!(
                headers[header::AUTHORIZATION],
                "Bearer fixture-only-provider"
            );
            assert_eq!(body["to"], serde_json::json!(["owner@example.test"]));
            assert_eq!(body["from"], transport.sender);
            let text = body["text"].as_str().unwrap();
            assert!(text.contains(&message.expires_at));
            assert!(text.contains(&message.token));
            assert!(text.contains("https://canteiro.example.test/acesso"));
            assert!(!text.contains("?token="));
            assert!(body.get("password").is_none());
        }
        server.abort();
    }

    #[test]
    fn cookie_parser_rejects_shadowing_and_malformed_bearers() {
        let value = "a".repeat(43);
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            format!("other=1; {COOKIE_NAME}={value}").parse().unwrap(),
        );
        assert_eq!(session_cookie(&headers), Ok(Some(value.clone())));
        headers.append(
            header::COOKIE,
            format!("{COOKIE_NAME}={value}").parse().unwrap(),
        );
        assert!(session_cookie(&headers).is_err());
        headers.clear();
        headers.insert(
            header::COOKIE,
            format!("{COOKIE_NAME}=not-valid").parse().unwrap(),
        );
        assert!(session_cookie(&headers).is_err());
    }

    #[test]
    fn origin_configuration_requires_complete_trusted_origin() {
        assert_eq!(
            validate_origin("https://example.test:8443", Environment::Homologation).unwrap(),
            "https://example.test:8443"
        );
        assert!(validate_origin("http://example.test", Environment::Development).is_err());
        assert!(validate_origin("http://127.0.0.1:3100", Environment::Homologation).is_err());
        assert!(validate_origin("http://127.0.0.1:3100", Environment::Development).is_ok());
        for value in [
            "https://user@example.test",
            "https://example.test/path",
            "https://example.test?x=1",
            "https://example.test#x",
            "null",
        ] {
            assert!(validate_origin(value, Environment::Homologation).is_err());
        }
    }
}
