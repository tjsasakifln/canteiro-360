use super::*;
use canteiro_persistence::{DatabaseEnvironment, DatabaseSettings, TransactionLimits, migrate};
use sqlx::{
    ConnectOptions, PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::{str::FromStr, time::Duration};

const ACCESS_PHRASE: &str = "frase de acesso fictícia 2026";
const NEXT_PHRASE: &str = "outra frase de acesso fictícia 2026";
struct Fixture {
    bootstrap: PgPool,
    admin: PgPool,
    database: Database,
    service: AuthService,
    name: String,
    owner: String,
    runtime: String,
    runtime_settings: DatabaseSettings,
}
impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("CANTEIRO_TEST_DATABASE_URL")
            .expect("CANTEIRO_TEST_DATABASE_URL deve apontar ao PostgreSQL local de teste");
        let options = PgConnectOptions::from_str(&url)
            .expect("configuração de teste inválida")
            .disable_statement_logging();
        assert!(
            matches!(options.get_host(), "127.0.0.1" | "localhost" | "::1")
                && options.get_socket().is_none()
        );
        assert!(options.get_database().is_some_and(|v| v.contains("test")));
        let bootstrap = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options.clone())
            .await
            .unwrap();
        let suffix = Uuid::new_v4().simple().to_string();
        let name = format!("auth_test_{suffix}");
        let owner = format!("auth_m_{suffix}");
        let runtime = format!("auth_r_{suffix}");
        for role in [&owner, &runtime] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE ROLE {role} LOGIN NOSUPERUSER NOBYPASSRLS NOCREATEDB NOCREATEROLE NOREPLICATION"))).execute(&bootstrap).await.unwrap();
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {name} OWNER {owner}"
        )))
        .execute(&bootstrap)
        .await
        .unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(3)
            .connect_with(options.clone().database(&name))
            .await
            .unwrap();
        let owner_settings = DatabaseSettings::from_external_url(
            options
                .clone()
                .database(&name)
                .username(&owner)
                .to_url_lossy()
                .as_str(),
        )
        .unwrap();
        let runtime_settings = DatabaseSettings::from_external_url(
            options
                .database(&name)
                .username(&runtime)
                .to_url_lossy()
                .as_str(),
        )
        .unwrap();
        migrate(
            &owner_settings,
            &runtime_settings,
            DatabaseEnvironment::Test,
        )
        .await
        .unwrap();
        let database = Database::connect(
            &runtime_settings,
            DatabaseEnvironment::Test,
            TransactionLimits::default(),
        )
        .await
        .unwrap();
        let service = AuthService::new(database.clone(), [47; 32]).await.unwrap();
        Self {
            bootstrap,
            admin,
            database,
            service,
            name,
            owner,
            runtime,
            runtime_settings,
        }
    }
    async fn close(self) {
        self.database.close().await;
        self.admin.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE {} WITH(FORCE)",
            self.name
        )))
        .execute(&self.bootstrap)
        .await
        .unwrap();
        for role in [&self.runtime, &self.owner] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP ROLE {role}")))
                .execute(&self.bootstrap)
                .await
                .unwrap();
        }
        self.bootstrap.close().await;
    }
    async fn anon(&self) -> String {
        self.service.context(None).await.unwrap().session_token
    }
    async fn execute(&self, s: &str, c: AuthCommand) -> Result<AuthResult, AuthError> {
        self.service.execute(s, c, "fixture-network").await
    }
    async fn mail(&self, email: &str, kind: &str) -> String {
        let pending = self.service.pending_mail(25).await.unwrap();
        let mut token = None;
        for m in pending {
            if m.email == email && m.kind == kind {
                token = Some(m.token.clone());
            }
            self.service
                .mark_mail_delivered(&m.id, "fixture")
                .await
                .unwrap();
        }
        token.expect("mensagem esperada na fila criptografada")
    }
    async fn register(&self, email: &str) -> String {
        let s = self.anon().await;
        self.execute(
            &s,
            AuthCommand::Register {
                email: email.into(),
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
        let code = self.mail(email, "verify_email").await;
        self.execute(
            &s,
            AuthCommand::VerifyEmail {
                token: code,
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
        s
    }
    async fn login(&self, email: &str, password: &str) -> AuthResult {
        let s = self.anon().await;
        self.execute(
            &s,
            AuthCommand::Login {
                email: email.into(),
                password: password.into(),
            },
        )
        .await
        .unwrap()
    }
    async fn enroll(&self, email: &str) -> AuthResult {
        self.register(email).await;
        sqlx::query("UPDATE canteiro_auth.users SET privileged=true WHERE email=$1")
            .bind(email)
            .execute(&self.admin)
            .await
            .unwrap();
        let login = self.login(email, ACCESS_PHRASE).await;
        assert_eq!(login.context.state, AuthState::MfaPending);
        let started = self
            .execute(
                &login.session_token,
                AuthCommand::StartMfa {
                    password: ACCESS_PHRASE.into(),
                },
            )
            .await
            .unwrap();
        let raw = data_encoding::BASE32_NOPAD
            .decode(started.mfa_setup.unwrap().as_bytes())
            .unwrap();
        let now: i64 = sqlx::query_scalar("SELECT extract(epoch FROM clock_timestamp())::bigint")
            .fetch_one(&self.admin)
            .await
            .unwrap();
        self.execute(
            &login.session_token,
            AuthCommand::ConfirmMfa {
                code: crypto::totp(&raw, now / 30),
            },
        )
        .await
        .unwrap()
    }
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn identity_verification_rotation_and_opaque_storage() {
    let f = Fixture::new().await;
    let s = f.anon().await;
    let registered = f
        .execute(
            &s,
            AuthCommand::Register {
                email: "person@example.test".into(),
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    let duplicate = f
        .execute(
            &s,
            AuthCommand::Register {
                email: "PERSON@EXAMPLE.TEST".into(),
                password: NEXT_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(registered.message, duplicate.message);
    assert_eq!(
        f.execute(
            &s,
            AuthCommand::Login {
                email: "person@example.test".into(),
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "invalid_credentials"
    );
    let code = f.mail("person@example.test", "verify_email").await;
    assert_eq!(
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &code)
            .unwrap()
            .len(),
        32
    );
    f.execute(
        &s,
        AuthCommand::VerifyEmail {
            token: code.clone(),
            password: ACCESS_PHRASE.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.execute(
            &s,
            AuthCommand::VerifyEmail {
                token: code.clone(),
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    let login = f
        .execute(
            &s,
            AuthCommand::Login {
                email: "person@example.test".into(),
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(login.context.state, AuthState::Authenticated);
    assert_ne!(login.session_token, s);
    assert_eq!(
        f.execute(&s, AuthCommand::Logout)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    assert!(
        f.service
            .verify_csrf(&login.session_token, &f.service.csrf(&login.session_token))
    );
    assert!(
        !f.service
            .verify_csrf(&s, &f.service.csrf(&login.session_token))
    );
    let audit: String =
        sqlx::query_scalar("SELECT coalesce(json_agg(a)::text,'[]') FROM canteiro_auth.audit a")
            .fetch_one(&f.admin)
            .await
            .unwrap();
    for secret in [ACCESS_PHRASE, &code, &login.session_token] {
        assert!(!audit.contains(secret));
    }
    let hashes: Vec<Vec<u8>> = sqlx::query_scalar("SELECT token_hash FROM canteiro_auth.sessions")
        .fetch_all(&f.admin)
        .await
        .unwrap();
    assert!(hashes.iter().all(|h| h.len() == 32));
    let raw_user: String =
        sqlx::query_scalar("SELECT row_to_json(u)::text FROM canteiro_auth.users u LIMIT 1")
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert!(!raw_user.contains(ACCESS_PHRASE));
    assert!(raw_user.contains("argon2id"));
    assert!(!raw_user.contains(&code));
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn password_reset_is_single_use_expiring_revokes_and_does_not_authenticate() {
    let f = Fixture::new().await;
    f.register("reset@example.test").await;
    let login = f.login("reset@example.test", ACCESS_PHRASE).await;
    let a = f.anon().await;
    let b = f.anon().await;
    let known = f
        .execute(
            &a,
            AuthCommand::RequestReset {
                email: "reset@example.test".into(),
            },
        )
        .await
        .unwrap();
    let missing = f
        .execute(
            &b,
            AuthCommand::RequestReset {
                email: "absent@example.test".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(known.message, missing.message);
    let code = f.mail("reset@example.test", "reset_password").await;
    assert_eq!(
        f.execute(
            &a,
            AuthCommand::VerifyEmail {
                token: code.clone(),
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    let (one, two) = tokio::join!(
        f.execute(
            &a,
            AuthCommand::ResetPassword {
                token: code.clone(),
                password: NEXT_PHRASE.into()
            }
        ),
        f.execute(
            &b,
            AuthCommand::ResetPassword {
                token: code.clone(),
                password: NEXT_PHRASE.into()
            }
        )
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let result = one.ok().or(two.ok()).unwrap();
    assert_eq!(result.context.state, AuthState::Anonymous);
    assert_eq!(
        f.execute(&login.session_token, AuthCommand::Logout)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    let s = f.anon().await;
    assert_eq!(
        f.execute(
            &s,
            AuthCommand::Login {
                email: "reset@example.test".into(),
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "invalid_credentials"
    );
    assert_eq!(
        f.login("reset@example.test", NEXT_PHRASE)
            .await
            .context
            .state,
        AuthState::Authenticated
    );
    f.execute(
        &s,
        AuthCommand::RequestReset {
            email: "reset@example.test".into(),
        },
    )
    .await
    .unwrap();
    let expired = f.mail("reset@example.test", "reset_password").await;
    sqlx::query(
        "UPDATE canteiro_auth.tokens SET expires_at=clock_timestamp() WHERE used_at IS NULL",
    )
    .execute(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        f.execute(
            &s,
            AuthCommand::ResetPassword {
                token: expired,
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn mfa_privilege_recovery_replay_and_reset_preservation() {
    let f = Fixture::new().await;
    let enrolled = f.enroll("mfa@example.test").await;
    assert_eq!(enrolled.context.state, AuthState::Authenticated);
    assert_eq!(enrolled.recovery_codes.len(), 10);
    assert!(enrolled.context.mfa_enabled);
    f.execute(&enrolled.session_token, AuthCommand::PrivilegedCheck)
        .await
        .unwrap();
    let a = f.login("mfa@example.test", ACCESS_PHRASE).await;
    let b = f.login("mfa@example.test", ACCESS_PHRASE).await;
    assert_eq!(
        f.execute(&a.session_token, AuthCommand::PrivilegedCheck)
            .await
            .err()
            .unwrap()
            .code(),
        "forbidden"
    );
    let code = enrolled.recovery_codes[0].clone();
    let (one, two) = tokio::join!(
        f.execute(
            &a.session_token,
            AuthCommand::RecoverMfa { code: code.clone() }
        ),
        f.execute(
            &b.session_token,
            AuthCommand::RecoverMfa { code: code.clone() }
        )
    );
    assert_ne!(one.is_ok(), two.is_ok());
    let recovered = one.ok().or(two.ok()).unwrap();
    assert!(recovered.context.mfa_enabled);
    let anon = f.anon().await;
    f.execute(
        &anon,
        AuthCommand::RequestReset {
            email: "mfa@example.test".into(),
        },
    )
    .await
    .unwrap();
    let reset = f.mail("mfa@example.test", "reset_password").await;
    let result = f
        .execute(
            &anon,
            AuthCommand::ResetPassword {
                token: reset,
                password: NEXT_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(result.context.state, AuthState::Anonymous);
    let after = f.login("mfa@example.test", NEXT_PHRASE).await;
    assert_eq!(after.context.state, AuthState::MfaPending);
    assert!(after.context.mfa_enabled);
    assert_eq!(
        f.execute(&enrolled.session_token, AuthCommand::PrivilegedCheck)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    assert_eq!(
        f.execute(&after.session_token, AuthCommand::RecoverMfa { code })
            .await
            .err()
            .unwrap()
            .code(),
        "code_invalid"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn revocation_is_scoped_and_next_command_checks_live_state() {
    let f = Fixture::new().await;
    f.register("a@example.test").await;
    f.register("b@example.test").await;
    let a = f.login("a@example.test", ACCESS_PHRASE).await;
    let another = f.login("a@example.test", ACCESS_PHRASE).await;
    let b = f.login("b@example.test", ACCESS_PHRASE).await;
    let other_id = b
        .context
        .sessions
        .iter()
        .find(|s| s.current)
        .unwrap()
        .id
        .clone();
    assert_eq!(
        f.execute(
            &a.session_token,
            AuthCommand::RevokeSession {
                session_id: other_id
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "forbidden"
    );
    assert_eq!(
        f.service
            .context(Some(&b.session_token))
            .await
            .unwrap()
            .context
            .state,
        AuthState::Authenticated
    );
    let own = another
        .context
        .sessions
        .iter()
        .find(|s| s.current)
        .unwrap()
        .id
        .clone();
    f.execute(
        &a.session_token,
        AuthCommand::RevokeSession { session_id: own },
    )
    .await
    .unwrap();
    assert_eq!(
        f.execute(&another.session_token, AuthCommand::RevokeAll)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    f.execute(&a.session_token, AuthCommand::RevokeAll)
        .await
        .unwrap();
    assert_eq!(
        f.execute(&a.session_token, AuthCommand::Logout)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    assert_eq!(
        f.service
            .context(Some(&b.session_token))
            .await
            .unwrap()
            .context
            .state,
        AuthState::Authenticated
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn rate_limits_survive_new_service_and_mail_is_encrypted_and_leased() {
    let f = Fixture::new().await;
    let s = f.anon().await;
    f.execute(
        &s,
        AuthCommand::Register {
            email: "mail@example.test".into(),
            password: ACCESS_PHRASE.into(),
        },
    )
    .await
    .unwrap();
    let ciphertext: Vec<u8> =
        sqlx::query_scalar("SELECT payload FROM canteiro_auth.mail_outbox LIMIT 1")
            .fetch_one(&f.admin)
            .await
            .unwrap();
    assert!(!String::from_utf8_lossy(&ciphertext).contains("mail@example.test"));
    let (a, b) = tokio::join!(f.service.pending_mail(25), f.service.pending_mail(25));
    assert_eq!(a.unwrap().len() + b.unwrap().len(), 1);
    let second = AuthService::new(f.database.clone(), [47; 32])
        .await
        .unwrap();
    for n in 0..8 {
        let service = if n % 2 == 0 { &f.service } else { &second };
        assert_eq!(
            service
                .execute(
                    &s,
                    AuthCommand::Login {
                        email: "none@example.test".into(),
                        password: ACCESS_PHRASE.into()
                    },
                    "fixture-network"
                )
                .await
                .err()
                .unwrap()
                .code(),
            "invalid_credentials"
        );
    }
    assert_eq!(
        second
            .execute(
                &s,
                AuthCommand::Login {
                    email: "none@example.test".into(),
                    password: ACCESS_PHRASE.into()
                },
                "fixture-network"
            )
            .await
            .err()
            .unwrap()
            .code(),
        "rate_limited"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn idle_expiry_credential_version_and_runtime_privilege_are_enforced() {
    let f = Fixture::new().await;
    f.register("version@example.test").await;
    let login = f.login("version@example.test", ACCESS_PHRASE).await;
    sqlx::query(
        "UPDATE canteiro_auth.users SET password_version=password_version+1 WHERE email=$1",
    )
    .bind("version@example.test")
    .execute(&f.admin)
    .await
    .unwrap();
    assert_eq!(
        f.execute(&login.session_token, AuthCommand::Logout)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    let again = f.login("version@example.test", ACCESS_PHRASE).await;
    sqlx::query("UPDATE canteiro_auth.sessions SET last_seen_at=clock_timestamp()-interval '30 minutes' WHERE user_id IS NOT NULL").execute(&f.admin).await.unwrap();
    assert_eq!(
        f.execute(&again.session_token, AuthCommand::Logout)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    let can_promote:bool=sqlx::query_scalar("SELECT has_column_privilege($1,'canteiro_auth.users','privileged','UPDATE') OR has_column_privilege($1,'canteiro_auth.users','privileged','INSERT')").bind(&f.runtime).fetch_one(&f.admin).await.unwrap();
    assert!(!can_promote);
    let audit_mutable: bool = sqlx::query_scalar(
        "SELECT has_table_privilege($1,'canteiro_auth.audit','UPDATE,DELETE,TRUNCATE')",
    )
    .bind(&f.runtime)
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert!(!audit_mutable);
    f.close().await;
}

#[tokio::test]
#[ignore = "mede Argon2 real e exige PostgreSQL local"]
async fn password_cost_is_measured_with_bounded_parallelism() {
    let f = Fixture::new().await;
    let start = std::time::Instant::now();
    let hash = f.service.hash_password(ACCESS_PHRASE.into()).await.unwrap();
    let hash_ms = start.elapsed().as_millis();
    let start = std::time::Instant::now();
    assert!(
        f.service
            .check_password(ACCESS_PHRASE.into(), hash)
            .await
            .unwrap()
            .0
    );
    let verify_ms = start.elapsed().as_millis();
    let p1 = f
        .service
        .password_slots
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let p2 = f
        .service
        .password_slots
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    assert_eq!(
        f.service
            .hash_password(ACCESS_PHRASE.into())
            .await
            .err()
            .unwrap()
            .code(),
        "rate_limited"
    );
    drop((p1, p2));
    println!(
        "Argon2id m=65536 KiB, t=3, p=1: hash={hash_ms}ms, verificação={verify_ms}ms; máximo=2 operações simultâneas por processo (128 MiB de memória Argon2). Este ensaio mede o executor atual, sem representar capacidade de produção."
    );
    assert!(start.elapsed() < Duration::from_secs(60));
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn email_owner_chooses_password_after_pre_registration_attack() {
    let f = Fixture::new().await;
    let attacker = f.anon().await;
    let owner = f.anon().await;
    f.execute(
        &attacker,
        AuthCommand::Register {
            email: "owner@example.test".into(),
            password: ACCESS_PHRASE.into(),
        },
    )
    .await
    .unwrap();
    // A tentativa do titular de cadastrar a conta não altera uma credencial sem prova.
    f.execute(
        &owner,
        AuthCommand::Register {
            email: "owner@example.test".into(),
            password: NEXT_PHRASE.into(),
        },
    )
    .await
    .unwrap();
    let token = f.mail("owner@example.test", "verify_email").await;
    f.execute(
        &owner,
        AuthCommand::VerifyEmail {
            token,
            password: NEXT_PHRASE.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        f.execute(
            &attacker,
            AuthCommand::Login {
                email: "owner@example.test".into(),
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "invalid_credentials"
    );
    assert_eq!(
        f.login("owner@example.test", NEXT_PHRASE)
            .await
            .context
            .state,
        AuthState::Authenticated
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn replacement_factor_requires_recent_mfa_and_invalidates_old_recovery_codes() {
    let f = Fixture::new().await;
    let enrolled = f.enroll("replace@example.test").await;
    let original: Vec<u8> = sqlx::query_scalar(
        "SELECT mfa_secret FROM canteiro_auth.users WHERE email='replace@example.test'",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    let pending = f.login("replace@example.test", ACCESS_PHRASE).await;
    assert_eq!(
        f.execute(
            &pending.session_token,
            AuthCommand::StartMfa {
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "forbidden"
    );
    let recovered = f
        .execute(
            &pending.session_token,
            AuthCommand::RecoverMfa {
                code: enrolled.recovery_codes[0].clone(),
            },
        )
        .await
        .unwrap();
    let started = f
        .execute(
            &recovered.session_token,
            AuthCommand::StartMfa {
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    let new_secret = data_encoding::BASE32_NOPAD
        .decode(started.mfa_setup.unwrap().as_bytes())
        .unwrap();
    let unchanged: Vec<u8> = sqlx::query_scalar(
        "SELECT mfa_secret FROM canteiro_auth.users WHERE email='replace@example.test'",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_eq!(unchanged, original);
    let now: i64 = sqlx::query_scalar("SELECT extract(epoch FROM clock_timestamp())::bigint")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let replacement = f
        .execute(
            &recovered.session_token,
            AuthCommand::ConfirmMfa {
                code: crypto::totp(&new_secret, now / 30),
            },
        )
        .await
        .unwrap();
    assert_eq!(replacement.recovery_codes.len(), 10);
    assert_ne!(replacement.recovery_codes, enrolled.recovery_codes);
    let changed: Vec<u8> = sqlx::query_scalar(
        "SELECT mfa_secret FROM canteiro_auth.users WHERE email='replace@example.test'",
    )
    .fetch_one(&f.admin)
    .await
    .unwrap();
    assert_ne!(changed, original);
    assert_eq!(
        f.execute(&enrolled.session_token, AuthCommand::PrivilegedCheck)
            .await
            .err()
            .unwrap()
            .code(),
        "session_expired"
    );
    let login = f.login("replace@example.test", ACCESS_PHRASE).await;
    assert_eq!(
        f.execute(
            &login.session_token,
            AuthCommand::RecoverMfa {
                code: enrolled.recovery_codes[1].clone()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    let user: Uuid =
        sqlx::query_scalar("SELECT id FROM canteiro_auth.users WHERE email='replace@example.test'")
            .fetch_one(&f.admin)
            .await
            .unwrap();
    let old_secret = crypto::open(&[47; 32], &format!("mfa:{user}:v1"), &original).unwrap();
    let old_code = [now / 30 - 1, now / 30, now / 30 + 1]
        .into_iter()
        .map(|step| crypto::totp(&old_secret, step))
        .find(|code| crypto::verify_totp(&new_secret, code, now, -1).is_none())
        .expect("distintos fatores de teste");
    assert_eq!(
        f.execute(
            &login.session_token,
            AuthCommand::VerifyMfa { code: old_code }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn obsolete_mail_is_not_claimed_and_public_requests_have_a_uniform_floor() {
    let f = Fixture::new().await;
    let s = f.register("resend@example.test").await;
    let start = std::time::Instant::now();
    let known = f
        .execute(
            &s,
            AuthCommand::RequestReset {
                email: "resend@example.test".into(),
            },
        )
        .await
        .unwrap();
    assert!(start.elapsed() >= Duration::from_millis(500));
    let start = std::time::Instant::now();
    let missing = f
        .execute(
            &s,
            AuthCommand::RequestReset {
                email: "missing@example.test".into(),
            },
        )
        .await
        .unwrap();
    assert!(start.elapsed() >= Duration::from_millis(500));
    assert_eq!(known.message, missing.message);
    f.execute(
        &s,
        AuthCommand::RequestReset {
            email: "resend@example.test".into(),
        },
    )
    .await
    .unwrap();
    let pending = f.service.pending_mail(25).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].kind, "reset_password");
    assert!(!pending[0].expires_at.is_empty());
    let discarded:i64=sqlx::query_scalar("SELECT count(*) FROM canteiro_auth.mail_outbox WHERE discarded_at IS NOT NULL AND octet_length(payload)=0").fetch_one(&f.admin).await.unwrap();
    assert_eq!(discarded, 1);
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn execution_identity_rejects_privileged_insert_and_column_audit_updates() {
    let f = Fixture::new().await;
    for permission in [
        "INSERT(privileged) ON canteiro_auth.users",
        "UPDATE(event) ON canteiro_auth.audit",
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "GRANT {permission} TO {}",
            f.runtime
        )))
        .execute(&f.admin)
        .await
        .unwrap();
        let limits = TransactionLimits::new(1, 300, 250, 100, 1000).unwrap();
        assert!(
            Database::connect(&f.runtime_settings, DatabaseEnvironment::Test, limits)
                .await
                .is_err()
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "REVOKE {permission} FROM {}",
            f.runtime
        )))
        .execute(&f.admin)
        .await
        .unwrap();
    }
    f.close().await;
}

#[tokio::test]
#[ignore = "requer PostgreSQL local e roles descartáveis"]
async fn mfa_configuration_is_bound_to_session_and_fresh_authentication() {
    let f = Fixture::new().await;
    let enrolled = f.enroll("fresh@example.test").await;
    let pending = f.login("fresh@example.test", ACCESS_PHRASE).await;
    let fresh = f
        .execute(
            &pending.session_token,
            AuthCommand::RecoverMfa {
                code: enrolled.recovery_codes[0].clone(),
            },
        )
        .await
        .unwrap();
    let started = f
        .execute(
            &fresh.session_token,
            AuthCommand::StartMfa {
                password: ACCESS_PHRASE.into(),
            },
        )
        .await
        .unwrap();
    let raw = data_encoding::BASE32_NOPAD
        .decode(started.mfa_setup.unwrap().as_bytes())
        .unwrap();
    let now: i64 = sqlx::query_scalar("SELECT extract(epoch FROM clock_timestamp())::bigint")
        .fetch_one(&f.admin)
        .await
        .unwrap();
    let code = crypto::totp(&raw, now / 30);
    assert_eq!(
        f.execute(
            &enrolled.session_token,
            AuthCommand::ConfirmMfa { code: code.clone() }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "code_invalid"
    );
    sqlx::query("UPDATE canteiro_auth.sessions SET created_at=clock_timestamp()-interval '6 minutes' WHERE token_hash=$1").bind(crypto::hash("session",&fresh.session_token)).execute(&f.admin).await.unwrap();
    assert_eq!(
        f.execute(&fresh.session_token, AuthCommand::ConfirmMfa { code })
            .await
            .err()
            .unwrap()
            .code(),
        "forbidden"
    );
    assert_eq!(
        f.execute(
            &fresh.session_token,
            AuthCommand::StartMfa {
                password: ACCESS_PHRASE.into()
            }
        )
        .await
        .err()
        .unwrap()
        .code(),
        "forbidden"
    );
    assert_eq!(
        f.service
            .context(Some(&fresh.session_token))
            .await
            .unwrap()
            .context
            .state,
        AuthState::Authenticated
    );
    f.close().await;
}
