//! Vínculos e seleção de empresa na mesma transação que autentica a pessoa.
//! A aplicação revalida a sessão, o vínculo e a versão antes de cada operação.
//! Nenhum executor, credencial ou segredo de convite é devolvido ao navegador.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub struct StoredOrganization {
    pub tenant_id: Uuid,
    pub name: String,
    pub version: i64,
    pub access_code_hash: Option<Vec<u8>>,
}

#[derive(sqlx::FromRow)]
pub struct StoredMembership {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub active: bool,
    pub valid_until: Option<DateTime<Utc>>,
    pub version: i64,
    pub effective: bool,
}

#[derive(sqlx::FromRow)]
pub struct StoredMember {
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub active: bool,
    pub valid_until: Option<DateTime<Utc>>,
    pub version: i64,
    pub effective: bool,
}

#[derive(sqlx::FromRow)]
pub struct StoredInvite {
    pub id: Uuid,
    pub email: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
    pub membership_valid_until: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub version: i64,
}

#[derive(sqlx::FromRow)]
pub struct StoredAccessRequest {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub requested_role: String,
    pub status: String,
    pub version: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct StoredSelection {
    pub tenant_id: Option<Uuid>,
    pub generation: i64,
}

impl AuthTransaction {
    /// Apenas a identidade autenticada pelo serviço pode fornecer este contexto.
    /// A empresa começa vazia e todo SET é local à transação atual.
    pub async fn org_set_actor(&mut self, actor: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("SELECT set_config('canteiro.actor_id',$1,true), set_config('canteiro.tenant_id','',true), set_config('canteiro.operation_id',$2,true)")
            .bind(actor.to_string()).bind(Uuid::new_v4().to_string())
            .execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_set_tenant(&mut self, tenant: Uuid) -> Result<(), AuthStoreError> {
        sqlx::query("SELECT set_config('canteiro.tenant_id',$1,true)")
            .bind(tenant.to_string())
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }

    pub async fn org_clear_tenant(&mut self) -> Result<(), AuthStoreError> {
        sqlx::query("SELECT set_config('canteiro.tenant_id','',true)")
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }

    pub async fn org_own_memberships(
        &mut self,
        actor: Uuid,
    ) -> Result<Vec<StoredMembership>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT tenant_id,user_id,role,active,valid_until,version,(active AND (valid_until IS NULL OR valid_until>clock_timestamp())) AS effective FROM canteiro.organization_members WHERE user_id=$1 AND active AND (valid_until IS NULL OR valid_until>clock_timestamp()) ORDER BY tenant_id")
            .bind(actor).fetch_all(&mut *self.tx).await?)
    }

    pub async fn org_create(
        &mut self,
        id: Uuid,
        name: &str,
        actor: Uuid,
        access_hash: &[u8],
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.organizations(tenant_id,name,created_by,access_code_hash) VALUES($1,$2,$3,$4)")
            .bind(id).bind(name).bind(actor).bind(access_hash).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_get(
        &mut self,
        id: Uuid,
        lock: bool,
    ) -> Result<Option<StoredOrganization>, AuthStoreError> {
        let query = if lock {
            "SELECT tenant_id,name,version,access_code_hash FROM canteiro.organizations WHERE tenant_id=$1 FOR UPDATE"
        } else {
            "SELECT tenant_id,name,version,access_code_hash FROM canteiro.organizations WHERE tenant_id=$1"
        };
        Ok(sqlx::query_as(query)
            .bind(id)
            .fetch_optional(&mut *self.tx)
            .await?)
    }

    pub async fn org_bump_version(
        &mut self,
        id: Uuid,
        expected: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro.organizations SET version=version+1 WHERE tenant_id=$1 AND version=$2 AND version<9223372036854775807")
            .bind(id).bind(expected).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    pub async fn org_rename(&mut self, tenant: Uuid, name: &str) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.organizations SET name=$2 WHERE tenant_id=$1")
            .bind(tenant)
            .bind(name)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }

    pub async fn org_set_access_hash(
        &mut self,
        id: Uuid,
        hash: &[u8],
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.organizations SET access_code_hash=$2 WHERE tenant_id=$1")
            .bind(id)
            .bind(hash)
            .execute(&mut *self.tx)
            .await?;
        Ok(())
    }

    pub async fn org_user_has_mfa(&mut self, user: Uuid) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM canteiro_auth.users WHERE id=$1 AND verified AND mfa_secret IS NOT NULL)")
            .bind(user).fetch_one(&mut *self.tx).await?)
    }

    pub async fn org_member(
        &mut self,
        tenant: Uuid,
        user: Uuid,
    ) -> Result<Option<StoredMember>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT m.user_id,u.email,m.role,m.active,m.valid_until,m.version,(m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())) AS effective FROM canteiro.organization_members m JOIN canteiro_auth.users u ON u.id=m.user_id WHERE m.tenant_id=$1 AND m.user_id=$2")
            .bind(tenant).bind(user).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn org_members(
        &mut self,
        tenant: Uuid,
        search: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<StoredMember>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT m.user_id,u.email,m.role,m.active,m.valid_until,m.version,(m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())) AS effective FROM canteiro.organization_members m JOIN canteiro_auth.users u ON u.id=m.user_id WHERE m.tenant_id=$1 AND strpos(lower(u.email),lower($2))>0 ORDER BY lower(u.email),m.user_id LIMIT $3 OFFSET $4")
            .bind(tenant).bind(search).bind(limit.clamp(1,500)).bind(offset.max(0)).fetch_all(&mut *self.tx).await?)
    }

    pub async fn org_active_member_by_email(
        &mut self,
        tenant: Uuid,
        email: &str,
    ) -> Result<Option<StoredMember>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT m.user_id,u.email,m.role,m.active,m.valid_until,m.version,true AS effective FROM canteiro.organization_members m JOIN canteiro_auth.users u ON u.id=m.user_id WHERE m.tenant_id=$1 AND u.email=$2 AND m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())")
            .bind(tenant).bind(email).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn org_member_count(
        &mut self,
        tenant: Uuid,
        search: &str,
    ) -> Result<i64, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_members m JOIN canteiro_auth.users u ON u.id=m.user_id WHERE m.tenant_id=$1 AND strpos(lower(u.email),lower($2))>0")
            .bind(tenant).bind(search).fetch_one(&mut *self.tx).await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn org_put_member(
        &mut self,
        tenant: Uuid,
        user: Uuid,
        role: &str,
        active: bool,
        valid_until: Option<DateTime<Utc>>,
        expected: Option<i64>,
    ) -> Result<bool, AuthStoreError> {
        let affected = if let Some(version) = expected {
            sqlx::query("UPDATE canteiro.organization_members SET role=$3,active=$4,valid_until=$5,version=version+1,updated_at=clock_timestamp() WHERE tenant_id=$1 AND user_id=$2 AND version=$6 AND version<9223372036854775807")
                .bind(tenant).bind(user).bind(role).bind(active).bind(valid_until).bind(version).execute(&mut *self.tx).await?.rows_affected()
        } else {
            sqlx::query("INSERT INTO canteiro.organization_members(tenant_id,user_id,role,active,valid_until) VALUES($1,$2,$3,$4,$5) ON CONFLICT(tenant_id,user_id) DO NOTHING")
                .bind(tenant).bind(user).bind(role).bind(active).bind(valid_until).execute(&mut *self.tx).await?.rows_affected()
        };
        Ok(affected == 1)
    }

    pub async fn org_admin_count(&mut self, tenant: Uuid) -> Result<i64, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM canteiro.organization_members WHERE tenant_id=$1 AND active AND role='administrator'")
            .bind(tenant).fetch_one(&mut *self.tx).await?)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn org_invite_create(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        email: &str,
        role: &str,
        hash: &[u8],
        expires: DateTime<Utc>,
        membership_valid_until: Option<DateTime<Utc>>,
        actor: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.organization_invites(tenant_id,id,email,role,token_hash,expires_at,membership_valid_until,created_by) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(tenant).bind(id).bind(email).bind(role).bind(hash).bind(expires).bind(membership_valid_until).bind(actor).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_invite_by_hash(
        &mut self,
        tenant: Uuid,
        hash: &[u8],
    ) -> Result<Option<StoredInvite>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,email,role,expires_at,membership_valid_until,revoked_at,accepted_at,version FROM canteiro.organization_invites WHERE tenant_id=$1 AND token_hash=$2")
            .bind(tenant).bind(hash).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn org_invite(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredInvite>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,email,role,expires_at,membership_valid_until,revoked_at,accepted_at,version FROM canteiro.organization_invites WHERE tenant_id=$1 AND id=$2")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn org_invites(&mut self, tenant: Uuid) -> Result<Vec<StoredInvite>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,email,role,expires_at,membership_valid_until,revoked_at,accepted_at,version FROM canteiro.organization_invites WHERE tenant_id=$1 ORDER BY (revoked_at IS NULL AND accepted_at IS NULL AND expires_at>clock_timestamp() AND (membership_valid_until IS NULL OR membership_valid_until>clock_timestamp())) DESC,created_at DESC,id LIMIT 500")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }

    pub async fn org_invite_revoke(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        expected: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro.organization_invites SET revoked_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND revoked_at IS NULL AND accepted_at IS NULL AND version<9223372036854775807")
            .bind(tenant).bind(id).bind(expected).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    pub async fn org_revoke_pending_invites(
        &mut self,
        tenant: Uuid,
        email: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.organization_invites SET revoked_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND email=$2 AND revoked_at IS NULL AND accepted_at IS NULL AND version<9223372036854775807")
            .bind(tenant).bind(email).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_invite_consume(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        expected: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro.organization_invites SET accepted_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND revoked_at IS NULL AND accepted_at IS NULL AND expires_at>clock_timestamp() AND (membership_valid_until IS NULL OR membership_valid_until>clock_timestamp()) AND version<9223372036854775807")
            .bind(tenant).bind(id).bind(expected).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    pub async fn org_request_create(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        user: Uuid,
        role: &str,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("INSERT INTO canteiro.organization_access_requests(tenant_id,id,user_id,requested_role) VALUES($1,$2,$3,$4) ON CONFLICT(tenant_id,user_id) WHERE status='pending' DO NOTHING")
            .bind(tenant).bind(id).bind(user).bind(role).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    pub async fn org_request(
        &mut self,
        tenant: Uuid,
        id: Uuid,
    ) -> Result<Option<StoredAccessRequest>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT r.id,r.user_id,u.email,r.requested_role,r.status,r.version,r.created_at FROM canteiro.organization_access_requests r JOIN canteiro_auth.users u ON u.id=r.user_id WHERE r.tenant_id=$1 AND r.id=$2")
            .bind(tenant).bind(id).fetch_optional(&mut *self.tx).await?)
    }

    pub async fn org_requests(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredAccessRequest>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT r.id,r.user_id,u.email,r.requested_role,r.status,r.version,r.created_at FROM canteiro.organization_access_requests r JOIN canteiro_auth.users u ON u.id=r.user_id WHERE r.tenant_id=$1 ORDER BY (r.status='pending') DESC,r.created_at DESC,r.id LIMIT 500")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }

    pub async fn org_request_decide(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        expected: i64,
        status: &str,
        actor: Uuid,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro.organization_access_requests SET status=$4,decided_by=$5,decided_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND status='pending' AND version<9223372036854775807")
            .bind(tenant).bind(id).bind(expected).bind(status).bind(actor).execute(&mut *self.tx).await?.rows_affected()==1)
    }

    pub async fn org_deny_pending_requests(
        &mut self,
        tenant: Uuid,
        user: Uuid,
        actor: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.organization_access_requests SET status='denied',decided_by=$3,decided_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND user_id=$2 AND status='pending' AND version<9223372036854775807")
            .bind(tenant).bind(user).bind(actor).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_audit(
        &mut self,
        tenant: Uuid,
        actor: Uuid,
        event: &str,
        subject: Option<Uuid>,
        detail: &str,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.organization_audit(tenant_id,id,actor_id,event,subject_id,detail) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(tenant).bind(Uuid::new_v4()).bind(actor).bind(event).bind(subject).bind(detail).execute(&mut *self.tx).await?;
        Ok(())
    }

    pub async fn org_selection(
        &mut self,
        session: Uuid,
    ) -> Result<StoredSelection, AuthStoreError> {
        sqlx::query("INSERT INTO canteiro_auth.organization_selections(session_id) VALUES($1) ON CONFLICT(session_id) DO NOTHING")
            .bind(session).execute(&mut *self.tx).await?;
        Ok(sqlx::query_as("SELECT tenant_id,generation FROM canteiro_auth.organization_selections WHERE session_id=$1 FOR UPDATE")
            .bind(session).fetch_one(&mut *self.tx).await?)
    }

    pub async fn org_select(
        &mut self,
        session: Uuid,
        tenant: Option<Uuid>,
        expected_generation: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro_auth.organization_selections SET tenant_id=$2,generation=generation+1 WHERE session_id=$1 AND generation=$3 AND generation<9223372036854775807")
            .bind(session).bind(tenant).bind(expected_generation).execute(&mut *self.tx).await?.rows_affected()==1)
    }
}
