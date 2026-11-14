//! Persistência de alçadas. Identidade, papel e escopo são revalidados pela aplicação na mesma transação.
use crate::auth::{AuthStoreError, AuthTransaction};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, sqlx::FromRow)]
pub struct StoredScope {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub parent_id: Option<Uuid>,
    pub version: i64,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredGrant {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub role: String,
    pub actions: Vec<String>,
    pub max_amount: String,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub source_grant_id: Option<Uuid>,
    pub substitution: bool,
    pub reason: String,
    pub version: i64,
    pub member_effective: bool,
    pub member_valid_until: Option<DateTime<Utc>>,
    pub required: bool,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredPolicy {
    pub scope_id: Uuid,
    pub action: String,
    pub threshold: String,
    pub approvals_above_threshold: i16,
    pub segregate_proposer: bool,
    pub version: i64,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredRequest {
    pub id: Uuid,
    pub scope_id: Uuid,
    pub version: i64,
    pub status: String,
    pub valid_approvals: i16,
    pub proposer_id: Uuid,
    pub proposer_email: String,
    pub title: String,
    pub action: String,
    pub increase: String,
    pub decrease: String,
    pub policy_version: i64,
    pub threshold: String,
    pub approvals_above_threshold: i16,
    pub segregate_proposer: bool,
    pub conflicted_users: Vec<Uuid>,
}
#[derive(Clone, sqlx::FromRow)]
pub struct StoredApproval {
    pub request_id: Uuid,
    pub request_version: i64,
    pub actor_id: Uuid,
    pub email: String,
    pub grant_id: Uuid,
    pub grant_version: i64,
    pub approved: bool,
    pub created_at: DateTime<Utc>,
}

impl AuthTransaction {
    pub async fn access_scopes(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredScope>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT id,kind,name,parent_id,version FROM canteiro.access_scopes WHERE tenant_id=$1 ORDER BY kind DESC,name,id")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn access_create_scope(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        kind: &str,
        name: &str,
        parent: Option<Uuid>,
        actor: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.access_scopes(tenant_id,id,kind,name,parent_id,created_by) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(tenant).bind(id).bind(kind).bind(name).bind(parent).bind(actor).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn access_grants(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredGrant>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT g.id,g.scope_id,g.user_id,u.email,g.role,g.actions,g.max_amount::text AS max_amount,g.valid_from,g.valid_until,g.revoked_at,g.source_grant_id,g.substitution,g.reason,g.version,(m.active AND (m.valid_until IS NULL OR m.valid_until>clock_timestamp())) AS member_effective,m.valid_until AS member_valid_until,EXISTS(SELECT 1 FROM canteiro.access_requirements r WHERE r.tenant_id=g.tenant_id AND r.scope_id=g.scope_id AND r.role=g.role AND g.source_grant_id IS NULL AND r.action=ANY(g.actions) AND g.max_amount>=r.min_amount AND g.valid_until IS NULL AND g.valid_from<=clock_timestamp() AND g.revoked_at IS NULL AND m.active AND m.valid_until IS NULL) AS required FROM canteiro.access_grants g JOIN canteiro.organization_members m ON m.tenant_id=g.tenant_id AND m.user_id=g.user_id JOIN canteiro_auth.users u ON u.id=g.user_id WHERE g.tenant_id=$1 ORDER BY g.created_at,g.id")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn access_insert_grant(
        &mut self,
        tenant: Uuid,
        grant: &StoredGrant,
        actor: Uuid,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.access_grants(tenant_id,id,scope_id,user_id,role,actions,max_amount,valid_from,valid_until,source_grant_id,substitution,created_by,reason) VALUES($1,$2,$3,$4,$5,$6,$7::text::numeric,$8,$9,$10,$11,$12,$13)")
            .bind(tenant).bind(grant.id).bind(grant.scope_id).bind(grant.user_id).bind(&grant.role).bind(&grant.actions).bind(&grant.max_amount).bind(grant.valid_from).bind(grant.valid_until).bind(grant.source_grant_id).bind(grant.substitution).bind(actor).bind(&grant.reason).execute(&mut *self.tx).await?;
        if grant.required {
            for action in grant.actions.iter().filter(|a| a.as_str() != "view") {
                sqlx::query("INSERT INTO canteiro.access_requirements(tenant_id,scope_id,role,action,min_amount) VALUES($1,$2,$3,$4,$5::text::numeric) ON CONFLICT(tenant_id,scope_id,role,action) DO UPDATE SET min_amount=GREATEST(canteiro.access_requirements.min_amount,EXCLUDED.min_amount)")
                    .bind(tenant).bind(grant.scope_id).bind(&grant.role).bind(action).bind(&grant.max_amount).execute(&mut *self.tx).await?;
            }
        }
        Ok(())
    }
    pub async fn access_revoke_grant(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        version: i64,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("UPDATE canteiro.access_grants SET revoked_at=clock_timestamp(),version=version+1 WHERE tenant_id=$1 AND id=$2 AND version=$3 AND revoked_at IS NULL")
            .bind(tenant).bind(id).bind(version).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    /// Verifica sucessão antes da escrita para devolver orientação útil. Trigger é a defesa concorrente.
    pub async fn access_removal_blocked(
        &mut self,
        tenant: Uuid,
        user: Option<Uuid>,
        grant: Option<Uuid>,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM canteiro.access_requirements r WHERE r.tenant_id=$1 AND EXISTS(SELECT 1 FROM canteiro.access_grants old WHERE old.tenant_id=r.tenant_id AND old.scope_id=r.scope_id AND old.role=r.role AND old.revoked_at IS NULL AND (($2::uuid IS NOT NULL AND old.user_id=$2) OR ($3::uuid IS NOT NULL AND old.id=$3))) AND NOT EXISTS(SELECT 1 FROM canteiro.access_grants g JOIN canteiro.organization_members m ON m.tenant_id=g.tenant_id AND m.user_id=g.user_id WHERE g.tenant_id=r.tenant_id AND g.scope_id=r.scope_id AND g.role=r.role AND r.action=ANY(g.actions) AND g.max_amount>=r.min_amount AND g.source_grant_id IS NULL AND g.revoked_at IS NULL AND g.valid_from<=clock_timestamp() AND g.valid_until IS NULL AND m.active AND m.valid_until IS NULL AND ($2::uuid IS NULL OR g.user_id<>$2) AND ($3::uuid IS NULL OR g.id<>$3)))")
            .bind(tenant).bind(user).bind(grant).fetch_one(&mut *self.tx).await?)
    }
    pub async fn access_policies(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredPolicy>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT scope_id,action,threshold::text AS threshold,approvals_above_threshold,segregate_proposer,version FROM canteiro.access_policies WHERE tenant_id=$1 ORDER BY scope_id,action")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn access_put_policy(
        &mut self,
        tenant: Uuid,
        policy: &StoredPolicy,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("INSERT INTO canteiro.access_policies(tenant_id,scope_id,action,threshold,approvals_above_threshold,segregate_proposer,version) VALUES($1,$2,$3,$4::text::numeric,$5,$6,$7) ON CONFLICT(tenant_id,scope_id,action) DO UPDATE SET threshold=EXCLUDED.threshold,approvals_above_threshold=EXCLUDED.approvals_above_threshold,segregate_proposer=EXCLUDED.segregate_proposer,version=EXCLUDED.version")
            .bind(tenant).bind(policy.scope_id).bind(&policy.action).bind(&policy.threshold).bind(policy.approvals_above_threshold).bind(policy.segregate_proposer).bind(policy.version).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn access_requests(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredRequest>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT r.id,r.scope_id,r.current_version AS version,r.status,r.valid_approvals,r.created_by AS proposer_id,u.email AS proposer_email,v.title,v.action,v.increase::text AS increase,v.decrease::text AS decrease,v.policy_version,v.threshold::text AS threshold,v.approvals_above_threshold,v.segregate_proposer,v.conflicted_users FROM canteiro.access_requests r JOIN canteiro.access_request_versions v ON v.tenant_id=r.tenant_id AND v.request_id=r.id AND v.version=r.current_version JOIN canteiro_auth.users u ON u.id=r.created_by WHERE r.tenant_id=$1 ORDER BY r.created_at DESC,r.id")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn access_save_request(
        &mut self,
        tenant: Uuid,
        request: &StoredRequest,
        new: bool,
    ) -> Result<(), AuthStoreError> {
        if new {
            sqlx::query("INSERT INTO canteiro.access_requests(tenant_id,id,scope_id,created_by) VALUES($1,$2,$3,$4)")
                .bind(tenant).bind(request.id).bind(request.scope_id).bind(request.proposer_id).execute(&mut *self.tx).await?;
        } else {
            sqlx::query("UPDATE canteiro.access_requests SET current_version=$3,status='pending',valid_approvals=0,updated_at=clock_timestamp() WHERE tenant_id=$1 AND id=$2")
                .bind(tenant).bind(request.id).bind(request.version).execute(&mut *self.tx).await?;
        }
        sqlx::query("INSERT INTO canteiro.access_request_versions(tenant_id,request_id,version,title,action,increase,decrease,policy_version,threshold,approvals_above_threshold,segregate_proposer,conflicted_users) VALUES($1,$2,$3,$4,$5,$6::text::numeric,$7::text::numeric,$8,$9::text::numeric,$10,$11,$12)")
            .bind(tenant).bind(request.id).bind(request.version).bind(&request.title).bind(&request.action).bind(&request.increase).bind(&request.decrease).bind(request.policy_version).bind(&request.threshold).bind(request.approvals_above_threshold).bind(request.segregate_proposer).bind(&request.conflicted_users).execute(&mut *self.tx).await?;
        Ok(())
    }
    pub async fn access_approvals(
        &mut self,
        tenant: Uuid,
    ) -> Result<Vec<StoredApproval>, AuthStoreError> {
        Ok(sqlx::query_as("SELECT a.request_id,a.request_version,a.actor_id,u.email,a.grant_id,a.grant_version,a.approved,a.created_at FROM canteiro.access_approvals a JOIN canteiro_auth.users u ON u.id=a.actor_id WHERE a.tenant_id=$1 ORDER BY a.created_at,a.actor_id")
            .bind(tenant).fetch_all(&mut *self.tx).await?)
    }
    pub async fn access_decide(
        &mut self,
        tenant: Uuid,
        approval: &StoredApproval,
    ) -> Result<bool, AuthStoreError> {
        Ok(sqlx::query("INSERT INTO canteiro.access_approvals(tenant_id,request_id,request_version,actor_id,grant_id,grant_version,approved) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING")
            .bind(tenant).bind(approval.request_id).bind(approval.request_version).bind(approval.actor_id).bind(approval.grant_id).bind(approval.grant_version).bind(approval.approved).execute(&mut *self.tx).await?.rows_affected()==1)
    }
    pub async fn access_request_status(
        &mut self,
        tenant: Uuid,
        id: Uuid,
        status: &str,
        count: u8,
    ) -> Result<(), AuthStoreError> {
        sqlx::query("UPDATE canteiro.access_requests SET status=$3,valid_approvals=$4,updated_at=clock_timestamp() WHERE tenant_id=$1 AND id=$2 AND status='pending'")
            .bind(tenant).bind(id).bind(status).bind(i16::from(count)).execute(&mut *self.tx).await?;
        Ok(())
    }
}
