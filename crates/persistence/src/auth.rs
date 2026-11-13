//! Persistência de identidades globais. Tipos internos nunca são serializáveis.
//! A autorização e a política criptográfica vivem em application::auth.
use crate::Database;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
#[error("Acesso temporariamente indisponível.")]
pub struct AuthStoreError;
impl From<sqlx::Error> for AuthStoreError {
    fn from(_: sqlx::Error) -> Self {
        Self
    }
}

#[derive(Clone)]
pub struct AuthStore {
    database: Database,
}
#[derive(sqlx::FromRow)]
pub struct StoredUser {
    pub id: Uuid,
    pub email: String,
    pub password_hash: String,
    pub password_version: i64,
    pub verified: bool,
    pub privileged: bool,
    pub mfa_secret: Option<Vec<u8>>,
    pub mfa_pending: Option<Vec<u8>>,
    pub mfa_pending_until: Option<DateTime<Utc>>,
    pub mfa_pending_session: Option<Uuid>,
    pub mfa_last_step: i64,
}
#[derive(sqlx::FromRow)]
pub struct StoredSession {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub password_version: Option<i64>,
    pub stage: String,
    pub mfa_verified: bool,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
#[derive(sqlx::FromRow)]
pub struct StoredToken {
    pub id: Uuid,
    pub user_id: Uuid,
}
#[derive(sqlx::FromRow)]
pub struct StoredMail {
    pub id: Uuid,
    pub payload: Vec<u8>,
    pub lease_id: Uuid,
    pub expires_at: DateTime<Utc>,
}
pub struct AuthTransaction {
    tx: Transaction<'static, Postgres>,
}

impl AuthStore {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
    pub async fn begin(&self) -> Result<AuthTransaction, AuthStoreError> {
        Ok(AuthTransaction {
            tx: self.database.pool.begin().await?,
        })
    }
    pub async fn user_by_email(&self, email: &str) -> Result<Option<StoredUser>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id, email, password_hash, password_version, verified, privileged, mfa_secret, mfa_pending, mfa_pending_until, mfa_pending_session, mfa_last_step FROM canteiro_auth.users WHERE email=$1")
            .bind(email).fetch_optional(&self.database.pool).await?)
    }
    pub async fn user_by_session(&self, hash: &[u8]) -> Result<Option<StoredUser>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT u.id, u.email, u.password_hash, u.password_version, u.verified, u.privileged, u.mfa_secret, u.mfa_pending, u.mfa_pending_until, u.mfa_pending_session, u.mfa_last_step FROM canteiro_auth.users u JOIN canteiro_auth.sessions s ON s.user_id=u.id WHERE s.token_hash=$1 AND s.revoked_at IS NULL AND s.expires_at>clock_timestamp() AND s.last_seen_at>clock_timestamp()-interval '30 minutes'")
            .bind(hash).fetch_optional(&self.database.pool).await?)
    }
    /// Limite compartilhado por processos. Incremento atômico em janela fixa.
    pub async fn attempt(
        &self,
        bucket: &[u8],
        maximum: i32,
        seconds: i32,
    ) -> Result<bool, AuthStoreError> {
        let count: i32 = sqlx::query_scalar("INSERT INTO canteiro_auth.rate_limits (bucket,window_start,attempts) VALUES ($1,clock_timestamp(),1) ON CONFLICT(bucket) DO UPDATE SET attempts=CASE WHEN canteiro_auth.rate_limits.window_start<=clock_timestamp()-($2::integer * interval '1 second') THEN 1 ELSE LEAST(canteiro_auth.rate_limits.attempts+1,1000000) END, window_start=CASE WHEN canteiro_auth.rate_limits.window_start<=clock_timestamp()-($2::integer * interval '1 second') THEN clock_timestamp() ELSE canteiro_auth.rate_limits.window_start END RETURNING attempts")
            .bind(bucket).bind(seconds).fetch_one(&self.database.pool).await?;
        Ok(count <= maximum)
    }
    pub async fn claim_mail(&self, limit: i64) -> Result<Vec<StoredMail>, AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.mail_outbox m SET payload='\\x'::bytea,discarded_at=clock_timestamp() FROM canteiro_auth.tokens t WHERE m.token_id=t.id AND m.delivered_at IS NULL AND m.discarded_at IS NULL AND (t.used_at IS NOT NULL OR t.expires_at<=clock_timestamp())")
            .execute(&self.database.pool).await?;
        let lease = Uuid::new_v4();
        Ok(sqlx::query_as("WITH selected AS (SELECT m.id,t.expires_at FROM canteiro_auth.mail_outbox m JOIN canteiro_auth.tokens t ON t.id=m.token_id WHERE m.delivered_at IS NULL AND m.discarded_at IS NULL AND t.used_at IS NULL AND t.expires_at>clock_timestamp() AND (m.lease_until IS NULL OR m.lease_until<=clock_timestamp()) ORDER BY m.created_at LIMIT $1 FOR UPDATE OF m SKIP LOCKED) UPDATE canteiro_auth.mail_outbox m SET lease_until=clock_timestamp()+interval '60 seconds',lease_id=$2,attempts=attempts+1 FROM selected WHERE m.id=selected.id RETURNING m.id,m.payload,m.lease_id,selected.expires_at")
            .bind(limit.clamp(1,25)).bind(lease).fetch_all(&self.database.pool).await?)
    }
    pub async fn delivered(&self, id: Uuid, lease: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.mail_outbox SET delivered_at=clock_timestamp(),payload='\\x'::bytea WHERE id=$1 AND lease_id=$2 AND lease_until>clock_timestamp() AND delivered_at IS NULL AND discarded_at IS NULL")
            .bind(id).bind(lease).execute(&self.database.pool).await?;
        Ok(())
    }
}

impl AuthTransaction {
    pub async fn now(&mut self) -> Result<DateTime<Utc>, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *self.tx)
            .await?)
    }
    pub async fn session(
        &mut self,
        hash: &[u8],
        lock: bool,
    ) -> Result<Option<StoredSession>, AuthStoreError> {
        let query = if lock {
            "SELECT id,user_id,password_version,stage,mfa_verified,created_at,expires_at FROM canteiro_auth.sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp() AND last_seen_at>clock_timestamp()-interval '30 minutes' FOR UPDATE"
        } else {
            "SELECT id,user_id,password_version,stage,mfa_verified,created_at,expires_at FROM canteiro_auth.sessions WHERE token_hash=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp() AND last_seen_at>clock_timestamp()-interval '30 minutes'"
        };
        Ok(sqlx::query_as(query)
            .bind(hash)
            .fetch_optional(&mut *self.tx)
            .await?)
    }
    pub async fn user(&mut self, id: Uuid) -> Result<Option<StoredUser>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,email,password_hash,password_version,verified,privileged,mfa_secret,mfa_pending,mfa_pending_until,mfa_pending_session,mfa_last_step FROM canteiro_auth.users WHERE id=$1 FOR UPDATE")
            .bind(id).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn user_by_email(
        &mut self,
        email: &str,
    ) -> Result<Option<StoredUser>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,email,password_hash,password_version,verified,privileged,mfa_secret,mfa_pending,mfa_pending_until,mfa_pending_session,mfa_last_step FROM canteiro_auth.users WHERE email=$1 FOR UPDATE")
            .bind(email).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn insert_user(
        &mut self,
        id: Uuid,
        email: &str,
        hash: &str,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("INSERT INTO canteiro_auth.users(id,email,password_hash) VALUES ($1,$2,$3) ON CONFLICT(email) DO NOTHING")
            .bind(id).bind(email).bind(hash).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    pub async fn set_password(
        &mut self,
        id: Uuid,
        hash: &str,
        increment: bool,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.users SET password_hash=$2,password_version=password_version+CASE WHEN $3 THEN 1 ELSE 0 END, mfa_pending=CASE WHEN $3 THEN NULL ELSE mfa_pending END, mfa_pending_until=CASE WHEN $3 THEN NULL ELSE mfa_pending_until END, mfa_pending_session=CASE WHEN $3 THEN NULL ELSE mfa_pending_session END WHERE id=$1")
            .bind(id).bind(hash).bind(increment).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn verify_email(&mut self, id: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.users SET verified=true WHERE id=$1")
            .bind(id)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn insert_session(
        &mut self,
        id: Uuid,
        hash: &[u8],
        user: Option<(Uuid, i64)>,
        stage: &str,
        mfa: bool,
        seconds: i32,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro_auth.sessions(id,token_hash,user_id,password_version,stage,mfa_verified,expires_at) VALUES($1,$2,$3,$4,$5,$6,clock_timestamp()+($7::integer * interval '1 second'))")
            .bind(id).bind(hash).bind(user.map(|v|v.0)).bind(user.map(|v|v.1)).bind(stage).bind(mfa).bind(seconds).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn touch(&mut self, id: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.sessions SET last_seen_at=clock_timestamp() WHERE id=$1")
            .bind(id)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn revoke(&mut self, id: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND revoked_at IS NULL").bind(id).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn revoke_owned(&mut self, id: Uuid, user: Uuid) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro_auth.sessions SET revoked_at=clock_timestamp() WHERE id=$1 AND user_id=$2 AND revoked_at IS NULL").bind(id).bind(user).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    pub async fn revoke_all(&mut self, user: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.sessions SET revoked_at=clock_timestamp() WHERE user_id=$1 AND revoked_at IS NULL").bind(user).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn sessions(&mut self, user: Uuid) -> Result<Vec<StoredSession>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,user_id,password_version,stage,mfa_verified,created_at,expires_at FROM canteiro_auth.sessions WHERE user_id=$1 AND revoked_at IS NULL AND expires_at>clock_timestamp() AND last_seen_at>clock_timestamp()-interval '30 minutes' ORDER BY created_at DESC LIMIT 50")
            .bind(user).fetch_all(&mut *self.tx).await?)
    }
    pub async fn issue_token(
        &mut self,
        user: Uuid,
        purpose: &str,
        hash: &[u8],
    ) -> Result<Uuid, AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.tokens SET used_at=clock_timestamp() WHERE user_id=$1 AND purpose=$2 AND used_at IS NULL").bind(user).bind(purpose).execute(&mut *self.tx).await?;
        sqlx::query("UPDATE canteiro_auth.mail_outbox m SET payload='\\x'::bytea,discarded_at=clock_timestamp() FROM canteiro_auth.tokens t WHERE m.token_id=t.id AND t.user_id=$1 AND t.purpose=$2 AND t.used_at IS NOT NULL AND m.delivered_at IS NULL AND m.discarded_at IS NULL").bind(user).bind(purpose).execute(&mut *self.tx).await?;
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO canteiro_auth.tokens(id,user_id,purpose,token_hash,expires_at) VALUES ($1,$2,$3,$4,clock_timestamp()+interval '15 minutes')")
            .bind(id).bind(user).bind(purpose).bind(hash).execute(&mut *self.tx).await?;
        Ok(id)
    }
    pub async fn token(
        &mut self,
        hash: &[u8],
        purpose: &str,
    ) -> Result<Option<StoredToken>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,user_id FROM canteiro_auth.tokens WHERE token_hash=$1 AND purpose=$2 AND used_at IS NULL AND expires_at>clock_timestamp()")
            .bind(hash).bind(purpose).fetch_optional(&mut *self.tx).await?)
    }
    pub async fn consume_token(&mut self, id: Uuid) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro_auth.tokens SET used_at=clock_timestamp() WHERE id=$1 AND used_at IS NULL AND expires_at>clock_timestamp()")
            .bind(id).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    pub async fn pending_mfa(
        &mut self,
        user: Uuid,
        session: Uuid,
        secret: &[u8],
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.users SET mfa_pending=$2,mfa_pending_until=clock_timestamp()+interval '10 minutes',mfa_pending_session=$3 WHERE id=$1")
            .bind(user).bind(secret).bind(session).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn enable_mfa(
        &mut self,
        user: Uuid,
        secret: &[u8],
        step: i64,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.users SET mfa_secret=$2,mfa_pending=NULL,mfa_pending_until=NULL,mfa_pending_session=NULL,mfa_last_step=$3 WHERE id=$1")
            .bind(user).bind(secret).bind(step).execute(&mut *self.tx).await?;
        sqlx::query("DELETE FROM canteiro_auth.recovery_codes WHERE user_id=$1")
            .bind(user)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn record_mfa_step(&mut self, user: Uuid, step: i64) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro_auth.users SET mfa_last_step=$2 WHERE id=$1")
            .bind(user)
            .bind(step)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn recovery_codes(
        &mut self,
        user: Uuid,
        codes: &[Vec<u8>],
    ) -> Result<(), AuthStoreError> {
        for code in codes {
            sqlx::query(
                "INSERT INTO canteiro_auth.recovery_codes(user_id,code_hash) VALUES($1,$2)",
            )
            .bind(user)
            .bind(code)
            .execute(&mut *self.tx)
            .await?;
        }
        Ok(())
    }
    pub async fn consume_recovery(
        &mut self,
        user: Uuid,
        hash: &[u8],
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro_auth.recovery_codes SET used_at=clock_timestamp() WHERE user_id=$1 AND code_hash=$2 AND used_at IS NULL")
            .bind(user).bind(hash).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    pub async fn enqueue_mail(
        &mut self,
        id: Uuid,
        token_id: Uuid,
        payload: &[u8],
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro_auth.mail_outbox(id,token_id,payload) VALUES($1,$2,$3)")
            .bind(id)
            .bind(token_id)
            .bind(payload)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn audit(&mut self, user: Option<Uuid>, event: &str) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro_auth.audit(id,user_id,event) VALUES($1,$2,$3)")
            .bind(Uuid::new_v4())
            .bind(user)
            .bind(event)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }
    pub async fn commit(self) -> Result<(), AuthStoreError> {
        self.tx.commit().await?;
        Ok(())
    }
}
