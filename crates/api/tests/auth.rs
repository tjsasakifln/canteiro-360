//! Exercita a fronteira HTTP real com PostgreSQL, cookies e origens distintas.
#[path = "auth/access.rs"]
mod access;
#[path = "auth/organizations.rs"]
mod organizations;

use axum::{
    Extension, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::Engine;
use canteiro_api::{
    auth::AuthHttp,
    config::{Config, Environment},
    router,
};
use canteiro_application::auth::AuthService;
use canteiro_contracts::auth::{AuthCommand, AuthResponse, AuthState};
use canteiro_persistence::{Database, DatabaseEnvironment, DatabaseSettings, TransactionLimits};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tower::ServiceExt;
use uuid::Uuid;

const ORIGIN: &str = "https://canteiro.example.test";

struct Fixture {
    app: Router,
    service: AuthService,
    database: Database,
}
struct Reply {
    status: StatusCode,
    cookie: Option<String>,
    headers: axum::http::HeaderMap,
    body: Value,
}
impl Reply {
    fn success(&self) -> AuthResponse {
        assert_eq!(
            self.status,
            StatusCode::OK,
            "{}",
            self.body.get("code").unwrap_or(&Value::Null)
        );
        serde_json::from_value(self.body.clone()).expect("public response")
    }
    fn bearer(&self) -> String {
        self.cookie
            .as_ref()
            .expect("session cookie")
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_owned()
    }
}
impl Fixture {
    async fn new() -> Self {
        let connection =
            std::env::var("CANTEIRO_DATABASE_URL").expect("use a provisioned development fixture");
        let settings = DatabaseSettings::from_external_url(&connection).unwrap();
        let database = Database::connect(
            &settings,
            DatabaseEnvironment::Development,
            TransactionLimits::default(),
        )
        .await
        .unwrap();
        let key: [u8; 32] = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(
                std::env::var("CANTEIRO_AUTH_KEY")
                    .expect("inject one ephemeral key for the entire fixture database"),
            )
            .unwrap()
            .try_into()
            .unwrap();
        let service = AuthService::new(database.clone(), key).await.unwrap();
        let config = Config {
            environment: Environment::Development,
            bind_addr: "127.0.0.1:3100".parse().unwrap(),
            site_root: "target/site".into(),
        };
        let app = router(&config).layer(Extension(AuthHttp::new(service.clone(), ORIGIN.into())));
        Self {
            app,
            service,
            database,
        }
    }

    async fn request(
        &self,
        body: Option<Value>,
        cookie: Option<&str>,
        csrf: Option<&str>,
        origin: Option<&str>,
    ) -> Reply {
        let mut request = Request::builder()
            .method(if body.is_some() { "POST" } else { "GET" })
            .uri(if body.is_some() {
                "/api/auth/command"
            } else {
                "/api/auth/context"
            });
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie.split(';').next().unwrap());
        }
        if let Some(csrf) = csrf {
            request = request.header("x-csrf-token", csrf);
        }
        if let Some(origin) = origin {
            request = request.header(header::ORIGIN, origin);
        }
        let body = if let Some(body) = body {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&body).unwrap())
        } else {
            Body::empty()
        };
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let cookie = headers
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_owned());
        let bytes = to_bytes(response.into_body(), 65_536).await.unwrap();
        Reply {
            status,
            cookie,
            headers,
            body: serde_json::from_slice(&bytes).unwrap(),
        }
    }

    async fn run(&self, previous: &Reply, command: AuthCommand) -> Reply {
        self.request(
            Some(serde_json::to_value(command).unwrap()),
            previous.cookie.as_deref(),
            Some(&previous.success().csrf),
            Some(ORIGIN),
        )
        .await
    }

    async fn code(&self, email: &str, purpose: &str) -> String {
        let messages = self.service.pending_mail(25).await.unwrap();
        let mut selected = None;
        for message in messages {
            if message.email == email && message.kind == purpose {
                selected = Some(message.token.clone());
            }
            self.service.mark_mail_delivered(&message.id).await.unwrap();
        }
        selected.expect("encrypted notification delivered to controlled fixture")
    }

    async fn account(&self) -> (String, String, Reply) {
        let email = format!("http-{}@example.test", Uuid::new_v4().simple());
        let passphrase = format!("Frase fictícia {}", Uuid::new_v4().simple());
        let anonymous = self.request(None, None, None, None).await;
        let registered = self
            .run(
                &anonymous,
                AuthCommand::Register {
                    email: email.clone(),
                    password: passphrase.clone(),
                },
            )
            .await;
        registered.success();
        let code = self.code(&email, "verify_email").await;
        let verified = self
            .run(
                &registered,
                AuthCommand::VerifyEmail {
                    token: code,
                    password: passphrase.clone(),
                },
            )
            .await;
        verified.success();
        let session = self
            .run(
                &verified,
                AuthCommand::Login {
                    email: email.clone(),
                    password: passphrase.clone(),
                },
            )
            .await;
        assert_eq!(session.success().context.state, AuthState::Authenticated);
        (email, passphrase, session)
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn csrf_origin_cookie_rotation_and_revocation_are_enforced_over_http() {
    let fixture = Fixture::new().await;
    let first = fixture.request(None, None, None, None).await;
    let initial = first.success();
    let cookie = first.cookie.as_ref().unwrap();
    for attribute in [
        "Path=/",
        "Secure",
        "HttpOnly",
        "SameSite=Strict",
        "Max-Age=28800",
    ] {
        assert!(cookie.contains(attribute));
    }
    assert!(cookie.starts_with("__Host-canteiro-session="));
    assert!(!cookie.contains("Domain="));
    assert_eq!(first.headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(first.headers[header::REFERRER_POLICY], "no-referrer");
    let (email, password, logged) = fixture.account().await;
    let session = logged.success();
    let original = logged.bearer();
    for origin in [
        None,
        Some("null"),
        Some("https://evil.example.test"),
        Some("http://canteiro.example.test"),
        Some("https://canteiro.example.test:8443"),
    ] {
        let denied = fixture
            .request(
                Some(json!({"action":"logout"})),
                logged.cookie.as_deref(),
                Some(&session.csrf),
                origin,
            )
            .await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN);
        assert!(denied.cookie.is_none());
    }
    for csrf in [None, Some("invalid"), Some(initial.csrf.as_str())] {
        let denied = fixture
            .request(
                Some(json!({"action":"logout"})),
                logged.cookie.as_deref(),
                csrf,
                Some(ORIGIN),
            )
            .await;
        assert_eq!(denied.status, StatusCode::FORBIDDEN);
        assert!(denied.cookie.is_none());
    }
    assert_eq!(
        fixture
            .service
            .context(Some(&original))
            .await
            .unwrap()
            .context
            .state,
        AuthState::Authenticated
    );
    let another = fixture.request(None, None, None, None).await;
    let second = fixture
        .run(&another, AuthCommand::Login { email, password })
        .await;
    assert_ne!(second.bearer(), another.bearer());
    let obsolete = fixture
        .run(
            &another,
            AuthCommand::Login {
                email: "absent@example.test".into(),
                password: "uma frase longa qualquer".into(),
            },
        )
        .await;
    assert_eq!(obsolete.status, StatusCode::UNAUTHORIZED);
    assert_eq!(obsolete.body["code"], "session_expired");
    let target = session
        .context
        .sessions
        .iter()
        .find(|s| s.current)
        .unwrap()
        .id
        .clone();
    let revoked = fixture
        .run(&second, AuthCommand::RevokeSession { session_id: target })
        .await;
    revoked.success();
    let denied = fixture.run(&logged, AuthCommand::Logout).await;
    assert_eq!(denied.status, StatusCode::UNAUTHORIZED);
    assert_eq!(denied.body["code"], "session_expired");
    let logout = fixture.run(&revoked, AuthCommand::Logout).await;
    assert_eq!(logout.success().context.state, AuthState::Anonymous);
    assert_ne!(logout.bearer(), revoked.bearer());
    fixture.database.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn reset_is_atomic_private_and_never_logs_in_over_http() {
    let fixture = Fixture::new().await;
    let (email, password, logged) = fixture.account().await;
    let anonymous = fixture.request(None, None, None, None).await;
    let reset = fixture
        .run(
            &anonymous,
            AuthCommand::RequestReset {
                email: email.clone(),
            },
        )
        .await;
    reset.success();
    let token = fixture.code(&email, "reset_password").await;
    let new_passphrase = format!("Nova frase fictícia {}", Uuid::new_v4().simple());
    // A hostile origin must not consume the valid code or modify credentials.
    let denied = fixture
        .request(
            Some(json!({"action":"reset_password","token":token,"password":new_passphrase})),
            reset.cookie.as_deref(),
            Some(&reset.success().csrf),
            Some("https://evil.example.test"),
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    assert_eq!(
        fixture
            .service
            .context(Some(&logged.bearer()))
            .await
            .unwrap()
            .context
            .state,
        AuthState::Authenticated
    );
    let result = fixture
        .run(
            &reset,
            AuthCommand::ResetPassword {
                token: token.clone(),
                password: new_passphrase.clone(),
            },
        )
        .await;
    assert_eq!(result.success().context.state, AuthState::Anonymous);
    assert!(result.success().context.email.is_none());
    let repeat = fixture
        .run(
            &result,
            AuthCommand::ResetPassword {
                token: token.clone(),
                password: new_passphrase.clone(),
            },
        )
        .await;
    assert_eq!(repeat.status, StatusCode::UNAUTHORIZED);
    assert_eq!(repeat.body["code"], "code_invalid");
    let expired = fixture.run(&logged, AuthCommand::Logout).await;
    assert_eq!(expired.body["code"], "session_expired");
    let wrong = fixture
        .run(
            &result,
            AuthCommand::Login {
                email: email.clone(),
                password,
            },
        )
        .await;
    assert_eq!(wrong.body["code"], "invalid_credentials");
    let login = fixture
        .run(
            &result,
            AuthCommand::Login {
                email,
                password: new_passphrase.clone(),
            },
        )
        .await;
    assert_eq!(login.success().context.state, AuthState::Authenticated);
    let serialized = serde_json::to_string(&login.body).unwrap();
    for private in [&token, &new_passphrase, &logged.bearer()] {
        assert!(!serialized.contains(private));
    }
    fixture.database.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL de desenvolvimento provisionado"]
async fn public_account_responses_and_recovery_timing_do_not_disclose_existence() {
    let fixture = Fixture::new().await;
    let (email, _, _) = fixture.account().await;
    let anonymous = fixture.request(None, None, None, None).await;
    let started = Instant::now();
    let known = fixture
        .run(
            &anonymous,
            AuthCommand::RequestReset {
                email: email.clone(),
            },
        )
        .await;
    let known_duration = started.elapsed();
    let started = Instant::now();
    let absent = fixture
        .run(
            &anonymous,
            AuthCommand::RequestReset {
                email: format!("absent-{}@example.test", Uuid::new_v4().simple()),
            },
        )
        .await;
    let absent_duration = started.elapsed();
    assert_eq!(known.status, absent.status);
    assert_eq!(known.body, absent.body);
    assert!(known_duration >= Duration::from_millis(500));
    assert!(absent_duration >= Duration::from_millis(500));
    // Unknown fields cannot assign a privileged identity through registration.
    let denied=fixture.request(Some(json!({"action":"register","email":"not-authorized@example.test","password":"uma frase fictícia longa","privileged":true})),anonymous.cookie.as_deref(),Some(&anonymous.success().csrf),Some(ORIGIN)).await;
    assert_eq!(denied.status, StatusCode::BAD_REQUEST);
    let unauthorized = fixture.run(&anonymous, AuthCommand::PrivilegedCheck).await;
    assert_eq!(unauthorized.status, StatusCode::FORBIDDEN);
    fixture.database.close().await;
}
