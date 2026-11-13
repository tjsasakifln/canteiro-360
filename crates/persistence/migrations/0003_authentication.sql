-- Identidades globais podem integrar várias organizações. Permissões de obra
-- continuam dependentes dos vínculos de cada organização, nunca desta tabela.
CREATE SCHEMA canteiro_auth;
REVOKE ALL ON SCHEMA canteiro_auth FROM PUBLIC;
CREATE TABLE canteiro_auth.users (
    id UUID PRIMARY KEY,
    email TEXT NOT NULL UNIQUE CHECK (length(email) BETWEEN 3 AND 254),
    password_hash TEXT NOT NULL,
    password_version BIGINT NOT NULL DEFAULT 1 CHECK (password_version > 0),
    verified BOOLEAN NOT NULL DEFAULT false,
    privileged BOOLEAN NOT NULL DEFAULT false,
    mfa_secret BYTEA,
    mfa_pending BYTEA,
    mfa_pending_until TIMESTAMPTZ,
    mfa_pending_session UUID,
    mfa_last_step BIGINT NOT NULL DEFAULT -1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE canteiro_auth.sessions (
    id UUID PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    user_id UUID REFERENCES canteiro_auth.users(id),
    password_version BIGINT,
    stage TEXT NOT NULL CHECK (stage IN ('anonymous', 'mfa_pending', 'authenticated')),
    mfa_verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CHECK ((stage = 'anonymous' AND user_id IS NULL AND password_version IS NULL)
        OR (stage <> 'anonymous' AND user_id IS NOT NULL AND password_version IS NOT NULL))
);
CREATE INDEX auth_sessions_user ON canteiro_auth.sessions(user_id);
CREATE INDEX auth_sessions_expiry ON canteiro_auth.sessions(expires_at);
CREATE TABLE canteiro_auth.tokens (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    purpose TEXT NOT NULL CHECK (purpose IN ('verify_email', 'reset_password')),
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ
);
CREATE INDEX auth_tokens_user_purpose ON canteiro_auth.tokens(user_id, purpose);
CREATE TABLE canteiro_auth.recovery_codes (
    user_id UUID NOT NULL REFERENCES canteiro_auth.users(id),
    code_hash BYTEA NOT NULL CHECK (octet_length(code_hash) = 32),
    used_at TIMESTAMPTZ,
    PRIMARY KEY(user_id, code_hash)
);
CREATE TABLE canteiro_auth.rate_limits (
    bucket BYTEA PRIMARY KEY CHECK (octet_length(bucket) = 32),
    window_start TIMESTAMPTZ NOT NULL,
    attempts INTEGER NOT NULL CHECK (attempts > 0)
);
CREATE TABLE canteiro_auth.audit (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES canteiro_auth.users(id),
    event TEXT NOT NULL CHECK (event IN ('registered', 'email_verified', 'login', 'login_failed',
        'verification_requested', 'reset_requested', 'password_reset', 'mfa_started', 'mfa_enabled',
        'mfa_verified', 'mfa_recovered', 'mfa_replaced', 'mfa_failed', 'logout', 'session_revoked', 'sessions_revoked',
        'privileged_checked')),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE canteiro_auth.mail_outbox (
    id UUID PRIMARY KEY,
    payload BYTEA NOT NULL,
    token_id UUID NOT NULL REFERENCES canteiro_auth.tokens(id),
    discarded_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    delivered_at TIMESTAMPTZ,
    lease_until TIMESTAMPTZ,
    lease_id UUID,
    attempts INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX auth_mail_pending ON canteiro_auth.mail_outbox(created_at) WHERE delivered_at IS NULL;
REVOKE ALL ON ALL TABLES IN SCHEMA canteiro_auth FROM PUBLIC;
