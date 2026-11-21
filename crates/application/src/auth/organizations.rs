//! Organizações na mesma transação que valida a sessão e o vínculo atual.
//! Seleção de empresa nunca é prova de autorização; toda operação a revalida.
use super::{AuthError, AuthErrorKind, AuthService, crypto};
use crate::organization_resources::{ResourceBoundary, ResourceKind};
use canteiro_contracts::organizations::*;
use canteiro_persistence::{
    auth::AuthTransaction,
    organizations::{StoredInvite, StoredMember, StoredOrganization},
};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
mod access;
pub mod documents;
mod estimating;
mod notifications;
mod obligations;
mod parties;
mod portfolio;
mod projects;
mod reliability;

struct Actor {
    id: Uuid,
    email: String,
    session: Uuid,
    mfa: bool,
}

fn invalid(message: &'static str) -> AuthError {
    AuthError::new(AuthErrorKind::InvalidInput, "invalid_input", message)
}
fn denied() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "organization_unavailable",
        "Esta empresa ou este registro não está disponível para seu acesso. Atualize a lista de empresas.",
    )
}
fn stale() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "version_conflict",
        "Este registro mudou desde sua consulta. Atualize os dados e confira a alteração antes de tentar novamente.",
    )
}
fn context_changed() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "context_changed",
        "A empresa ativa mudou. Atualize os dados antes de continuar; a operação não foi gravada.",
    )
}
fn invalid_invitation() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "invitation_invalid",
        "Este convite não está disponível para sua conta. Confira o código, o e-mail de acesso e a validade com quem o convidou.",
    )
}
fn already_member() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "already_member",
        "Esta pessoa já tem vínculo ativo. Para mudar o papel, use a gestão de membros da empresa.",
    )
}
fn require_mfa(actor: &Actor) -> Result<(), AuthError> {
    if actor.mfa {
        Ok(())
    } else {
        Err(AuthError::new(
            AuthErrorKind::Forbidden,
            "mfa_required",
            "Para administrar uma empresa, ative e confirme a verificação em duas etapas na página de acesso.",
        ))
    }
}
fn id(value: &str) -> Result<Uuid, AuthError> {
    let id = Uuid::parse_str(value)
        .map_err(|_| invalid("Confira o identificador da empresa ou do registro."))?;
    if id.is_nil() || id == Uuid::max() {
        return Err(invalid(
            "Confira o identificador da empresa ou do registro.",
        ));
    }
    Ok(id)
}
fn role(value: &str) -> Result<OrganizationRole, AuthError> {
    match value {
        "administrator" => Ok(OrganizationRole::Administrator),
        "member" => Ok(OrganizationRole::Member),
        _ => Err(AuthError::unavailable()),
    }
}
fn name(value: &str) -> Result<String, AuthError> {
    let value = value.trim();
    if !(2..=160).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(invalid(
            "Informe o nome da empresa com 2 a 160 caracteres, sem quebras de linha.",
        ));
    }
    Ok(value.to_owned())
}
fn deadline(
    value: Option<&str>,
    member_role: OrganizationRole,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, AuthError> {
    if member_role == OrganizationRole::Administrator && value.is_some() {
        return Err(invalid(
            "Administradores não têm expiração automática. Transfira a administração antes de encerrar o vínculo.",
        ));
    }
    value
        .map(|v| {
            let date = DateTime::parse_from_rfc3339(v)
                .map_err(|_| invalid("Informe uma data de validade com horário e fuso válidos."))?
                .with_timezone(&Utc);
            if date <= now {
                return Err(invalid("A validade do vínculo deve terminar no futuro."));
            }
            Ok(date)
        })
        .transpose()
}
fn code(tenant: Uuid) -> String {
    format!("{tenant}.{}", crypto::random_token())
}
fn parse_code(value: &str) -> Result<(Uuid, &str), AuthError> {
    let value = value.trim();
    let Some((tenant_text, secret)) = value.split_once('.') else {
        return Err(invalid_invitation());
    };
    let tenant = id(tenant_text).map_err(|_| invalid_invitation())?;
    if tenant.to_string() != tenant_text || !crypto::token_valid(secret) {
        return Err(invalid_invitation());
    }
    Ok((tenant, value))
}
fn member_view(member: StoredMember) -> Result<MemberSummary, AuthError> {
    Ok(MemberSummary {
        id: member.user_id.to_string(),
        email: member.email,
        role: role(&member.role)?,
        active: member.effective,
        valid_until: member.valid_until.map(|v| v.to_rfc3339()),
        version: member.version,
    })
}
fn summary(
    org: &StoredOrganization,
    member: &StoredMember,
) -> Result<OrganizationSummary, AuthError> {
    Ok(OrganizationSummary {
        id: org.tenant_id.to_string(),
        name: org.name.clone(),
        role: role(&member.role)?,
        version: org.version,
        valid_until: member.valid_until.map(|v| v.to_rfc3339()),
    })
}

impl AuthService {
    async fn organization_transaction(
        &self,
        token: &str,
    ) -> Result<(AuthTransaction, Actor), AuthError> {
        let mut tx = self.store.begin().await?;
        let (session, user) = self.locked(&mut tx, token).await?;
        let user = Self::authenticated(&session, &user)?;
        let actor = Actor {
            id: user.id,
            email: user.email.clone(),
            session: session.id,
            mfa: session.mfa_verified && user.mfa_secret.is_some(),
        };
        tx.org_set_actor(actor.id).await?;
        tx.touch(session.id).await?;
        Ok((tx, actor))
    }

    pub async fn organizations_context(
        &self,
        token: &str,
    ) -> Result<OrganizationResponse, AuthError> {
        self.limit("organizations-context", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let state = organization_state(&mut tx, &actor).await?;
        tx.commit().await?;
        Ok(OrganizationResponse {
            state,
            csrf: self.csrf(token),
            message: String::new(),
            invitation_code: None,
            access_code: None,
            invitation_preview: None,
        })
    }

    pub async fn organizations_execute(
        &self,
        token: &str,
        command: OrganizationCommand,
    ) -> Result<OrganizationResponse, AuthError> {
        self.limit("organizations-command", token, 120, 60).await?;
        // Cotas não podem tentar reservar uma segunda conexão enquanto a transação
        // do comando segura a única conexão de um pool pequeno.
        if matches!(
            &command,
            OrganizationCommand::Create { .. } | OrganizationCommand::RequestAccess { .. }
        ) {
            let user = self
                .store
                .user_by_session(&crypto::hash("session", token))
                .await?
                .ok_or_else(AuthError::expired)?;
            if matches!(&command, OrganizationCommand::Create { .. }) {
                self.limit("organizations-create", &user.id.to_string(), 20, 86400)
                    .await?;
            } else {
                self.limit(
                    "organization-access-request",
                    &user.id.to_string(),
                    20,
                    3600,
                )
                .await?;
            }
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        lock_command_context(&mut tx, &actor, &command).await?;
        let mut invitation_code = None;
        let mut access_code = None;
        let mut invitation_preview = None;
        let message = match command {
            OrganizationCommand::Create { name: input } => {
                require_mfa(&actor)?;
                let title = name(&input)?;
                let tenant = Uuid::new_v4();
                let access = code(tenant);
                tx.org_set_tenant(tenant).await?;
                tx.org_create(
                    tenant,
                    &title,
                    actor.id,
                    &crypto::hash("organization-access", &access),
                )
                .await?;
                if !tx
                    .org_put_member(tenant, actor.id, "administrator", true, None, None)
                    .await?
                {
                    return Err(stale());
                }
                tx.org_audit(
                    tenant,
                    actor.id,
                    "organization_created",
                    Some(tenant),
                    "first_administrator",
                )
                .await?;
                access_code = Some(access);
                "Empresa criada. Selecione-a para administrar a equipe. O código de solicitação de acesso pode ser copiado agora; ele não concede acesso por si só."
            }
            OrganizationCommand::Select {
                organization_id,
                expected_context_version,
            } => {
                let tenant = id(&organization_id)?;
                let selection = tx.org_selection(actor.session).await?;
                if selection.generation != expected_context_version {
                    return Err(context_changed());
                }
                authorize(&mut tx, &actor, tenant).await?;
                if !tx
                    .org_select(actor.session, Some(tenant), expected_context_version)
                    .await?
                {
                    return Err(context_changed());
                }
                tx.org_audit(
                    tenant,
                    actor.id,
                    "organization_selected",
                    None,
                    "explicit_selection",
                )
                .await?;
                "Empresa ativa alterada. As próximas operações usarão somente esta empresa."
            }
            OrganizationCommand::ClearSelection {
                expected_context_version,
            } => {
                let previous = tx.org_selection(actor.session).await?;
                if !tx
                    .org_select(actor.session, None, expected_context_version)
                    .await?
                {
                    return Err(context_changed());
                }
                if let Some(tenant) = previous.tenant_id {
                    tx.org_set_tenant(tenant).await?;
                    if tx
                        .org_member(tenant, actor.id)
                        .await?
                        .is_some_and(|m| m.effective)
                    {
                        tx.org_audit(
                            tenant,
                            actor.id,
                            "organization_selection_cleared",
                            None,
                            "explicit_clear",
                        )
                        .await?;
                    }
                }
                "Seleção de empresa encerrada."
            }
            OrganizationCommand::Rename { scope, name: input } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let title = name(&input)?;
                tx.org_rename(org.tenant_id, &title).await?;
                let detail = format!(
                    "name_before({}):{};name_after({}):{}",
                    org.name.chars().count(),
                    org.name,
                    title.chars().count(),
                    title
                );
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "organization_renamed",
                    Some(org.tenant_id),
                    &detail,
                )
                .await?;
                "Nome da empresa atualizado."
            }
            OrganizationCommand::Invite {
                scope,
                email,
                role: invited_role,
                valid_for_hours,
                membership_valid_until,
            } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let email = crypto::email(&email)?;
                if !(1..=168).contains(&valid_for_hours) {
                    return Err(invalid(
                        "Escolha uma validade de 1 a 168 horas para o convite.",
                    ));
                }
                let now = tx.now().await?;
                let valid_until = deadline(membership_valid_until.as_deref(), invited_role, now)?;
                // Uma pessoa já vinculada não recebe promoção por um convite antigo.
                if tx
                    .org_active_member_by_email(org.tenant_id, &email)
                    .await?
                    .is_some()
                {
                    return Err(already_member());
                }
                if tx
                    .org_invites(org.tenant_id)
                    .await?
                    .iter()
                    .filter(|v| {
                        v.revoked_at.is_none()
                            && v.accepted_at.is_none()
                            && v.expires_at > now
                            && v.membership_valid_until.is_none_or(|d| d > now)
                    })
                    .count()
                    >= 500
                {
                    return Err(invalid(
                        "Há 500 convites pendentes. Revogue ou conclua convites antes de criar outros.",
                    ));
                }
                let invite = code(org.tenant_id);
                let invitation_id = Uuid::new_v4();
                tx.org_revoke_pending_invites(org.tenant_id, &email).await?;
                tx.org_invite_create(
                    org.tenant_id,
                    invitation_id,
                    &email,
                    invited_role.as_str(),
                    &crypto::hash("organization-invitation", &invite),
                    now + Duration::hours(i64::from(valid_for_hours)),
                    valid_until,
                    actor.id,
                )
                .await?;
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "invitation_created",
                    Some(invitation_id),
                    invited_role.as_str(),
                )
                .await?;
                invitation_code = Some(invite);
                "Convite criado. Copie o código e envie-o ao e-mail informado. A pessoa precisa entrar com esse mesmo e-mail e aceitar o acesso. Nenhuma mensagem foi enviada automaticamente."
            }
            OrganizationCommand::RevokeInvite {
                scope,
                invitation_id,
                invitation_version,
            } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let invitation = id(&invitation_id)?;
                if tx.org_invite(org.tenant_id, invitation).await?.is_none() {
                    return Err(denied());
                }
                if !tx
                    .org_invite_revoke(org.tenant_id, invitation, invitation_version)
                    .await?
                {
                    return Err(stale());
                }
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "invitation_revoked",
                    Some(invitation),
                    "revoked",
                )
                .await?;
                "Convite revogado. O código não poderá conceder acesso."
            }
            OrganizationCommand::PreviewInvitation { token: input } => {
                let (org, invite) = invitation(&mut tx, &actor, &input).await?;
                invitation_preview = Some(InvitationPreview {
                    organization_name: org.name,
                    role: role(&invite.role)?,
                    expires_at: invite.expires_at.to_rfc3339(),
                    membership_valid_until: invite.membership_valid_until.map(|v| v.to_rfc3339()),
                });
                "Confira a empresa, o papel e a validade antes de aceitar. O vínculo com a empresa não concede acesso automático às obras."
            }
            OrganizationCommand::AcceptInvitation { token: input } => {
                let (org, invite) = invitation(&mut tx, &actor, &input).await?;
                if role(&invite.role)? == OrganizationRole::Administrator {
                    require_mfa(&actor)?;
                }
                let existing = tx.org_member(org.tenant_id, actor.id).await?;
                if existing.as_ref().is_some_and(|m| m.effective) {
                    return Err(already_member());
                }
                if !tx
                    .org_invite_consume(org.tenant_id, invite.id, invite.version)
                    .await?
                {
                    return Err(invalid_invitation());
                }
                if !tx
                    .org_put_member(
                        org.tenant_id,
                        actor.id,
                        &invite.role,
                        true,
                        invite.membership_valid_until,
                        existing.map(|m| m.version),
                    )
                    .await?
                {
                    return Err(stale());
                }
                tx.org_deny_pending_requests(org.tenant_id, actor.id, actor.id)
                    .await?;
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "invitation_accepted",
                    Some(invite.id),
                    &invite.role,
                )
                .await?;
                "Convite aceito com sua conta atual. Selecione a empresa para continuar."
            }
            OrganizationCommand::RotateAccessCode { scope } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let access = code(org.tenant_id);
                tx.org_set_access_hash(
                    org.tenant_id,
                    &crypto::hash("organization-access", &access),
                )
                .await?;
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "access_code_rotated",
                    None,
                    "previous_code_invalidated",
                )
                .await?;
                access_code = Some(access);
                "Novo código criado. O anterior deixou de aceitar solicitações. Copie e compartilhe este código somente com quem deve pedir acesso."
            }
            OrganizationCommand::RequestAccess {
                code: input,
                requested_role,
            } => {
                if requested_role == OrganizationRole::Administrator {
                    require_mfa(&actor)?;
                }
                let (tenant, normalized) = parse_code(&input).map_err(|_| denied())?;
                tx.org_set_tenant(tenant).await?;
                let org = tx.org_get(tenant, true).await?.ok_or_else(denied)?;
                if org.access_code_hash.as_deref()
                    != Some(crypto::hash("organization-access", normalized).as_slice())
                {
                    return Err(denied());
                }
                if tx
                    .org_member(tenant, actor.id)
                    .await?
                    .is_some_and(|m| m.effective)
                {
                    return Err(already_member());
                }
                let pending = tx.org_requests(tenant).await?;
                if pending.iter().filter(|v| v.status == "pending").count() >= 500
                    && !pending
                        .iter()
                        .any(|v| v.status == "pending" && v.user_id == actor.id)
                {
                    return Err(AuthError::new(
                        AuthErrorKind::RateLimited,
                        "requests_unavailable",
                        "Não foi possível registrar outra solicitação agora. Peça orientação ao administrador da empresa.",
                    ));
                }
                let request_id = Uuid::new_v4();
                if tx
                    .org_request_create(tenant, request_id, actor.id, requested_role.as_str())
                    .await?
                {
                    finish(
                        &mut tx,
                        &actor,
                        &org,
                        "access_requested",
                        Some(request_id),
                        requested_role.as_str(),
                    )
                    .await?;
                }
                "Solicitação registrada. Aguarde a decisão de um administrador. O pedido ainda não concede acesso à empresa."
            }
            OrganizationCommand::DecideAccess {
                scope,
                request_id,
                request_version,
                approve,
            } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let request_id = id(&request_id)?;
                let request = tx
                    .org_request(org.tenant_id, request_id)
                    .await?
                    .ok_or_else(denied)?;
                if request.status != "pending" || request.version != request_version {
                    return Err(stale());
                }
                if approve {
                    let wanted_role = role(&request.requested_role)?;
                    if wanted_role == OrganizationRole::Administrator
                        && !tx.org_user_has_mfa(request.user_id).await?
                    {
                        return Err(invalid(
                            "A pessoa deve ativar a verificação em duas etapas antes de receber a administração.",
                        ));
                    }
                    let existing = tx.org_member(org.tenant_id, request.user_id).await?;
                    if existing.as_ref().is_some_and(|m| m.effective) {
                        return Err(already_member());
                    }
                    if !tx
                        .org_put_member(
                            org.tenant_id,
                            request.user_id,
                            wanted_role.as_str(),
                            true,
                            None,
                            existing.map(|m| m.version),
                        )
                        .await?
                    {
                        return Err(stale());
                    }
                    tx.org_revoke_pending_invites(org.tenant_id, &request.email)
                        .await?;
                }
                let decision = if approve { "approved" } else { "denied" };
                if !tx
                    .org_request_decide(
                        org.tenant_id,
                        request_id,
                        request_version,
                        decision,
                        actor.id,
                    )
                    .await?
                {
                    return Err(stale());
                }
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "access_decided",
                    Some(request_id),
                    decision,
                )
                .await?;
                if approve {
                    "Solicitação aprovada. A pessoa poderá selecionar a empresa com a conta que já possui."
                } else {
                    "Solicitação recusada. Nenhum acesso foi concedido."
                }
            }
            OrganizationCommand::ChangeMember {
                scope,
                user_id,
                member_version,
                role: new_role,
                valid_until,
            } => {
                let (org, _) = scoped(&mut tx, &actor, &scope, true).await?;
                let target_id = id(&user_id)?;
                let target = tx
                    .org_member(org.tenant_id, target_id)
                    .await?
                    .ok_or_else(denied)?;
                if target.version != member_version {
                    return Err(stale());
                }
                if !target.effective {
                    return Err(invalid(
                        "Para reativar um vínculo encerrado, emita um novo convite ou decida uma nova solicitação de acesso.",
                    ));
                }
                if new_role == OrganizationRole::Administrator
                    && !tx.org_user_has_mfa(target_id).await?
                {
                    return Err(invalid(
                        "A pessoa deve ativar a verificação em duas etapas antes de receber a administração.",
                    ));
                }
                let expiry = deadline(valid_until.as_deref(), new_role, tx.now().await?)?;
                if expiry.is_some()
                    && tx
                        .access_removal_blocked(org.tenant_id, Some(target_id), None)
                        .await?
                {
                    return Err(access::responsibility_required());
                }
                protect_last_admin(
                    &mut tx,
                    &org,
                    &target,
                    new_role == OrganizationRole::Administrator,
                )
                .await?;
                if !tx
                    .org_put_member(
                        org.tenant_id,
                        target_id,
                        new_role.as_str(),
                        true,
                        expiry,
                        Some(member_version),
                    )
                    .await?
                {
                    return Err(stale());
                }
                tx.org_revoke_pending_invites(org.tenant_id, &target.email)
                    .await?;
                let detail = format!(
                    "role:{}>{};valid_until:{}>{}",
                    target.role,
                    new_role.as_str(),
                    target
                        .valid_until
                        .map(|v| v.to_rfc3339())
                        .unwrap_or_default(),
                    expiry.map(|v| v.to_rfc3339()).unwrap_or_default()
                );
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "member_changed",
                    Some(target_id),
                    &detail,
                )
                .await?;
                "Vínculo atualizado. As próximas operações revalidarão o papel e a validade."
            }
            OrganizationCommand::RemoveMember {
                scope,
                user_id,
                member_version,
            } => {
                let (org, own) = scoped(&mut tx, &actor, &scope, false).await?;
                let target_id = id(&user_id)?;
                if target_id != actor.id {
                    require_mfa(&actor)?;
                    if own.role != "administrator" {
                        return Err(denied());
                    }
                } else if own.role == "administrator" {
                    require_mfa(&actor)?;
                }
                let target = tx
                    .org_member(org.tenant_id, target_id)
                    .await?
                    .ok_or_else(denied)?;
                if target.version != member_version || !target.active {
                    return Err(stale());
                }
                protect_last_admin(&mut tx, &org, &target, false).await?;
                if tx
                    .access_removal_blocked(org.tenant_id, Some(target_id), None)
                    .await?
                {
                    return Err(access::responsibility_required());
                }
                if !tx
                    .org_put_member(
                        org.tenant_id,
                        target_id,
                        &target.role,
                        false,
                        target.valid_until,
                        Some(member_version),
                    )
                    .await?
                {
                    return Err(stale());
                }
                tx.org_revoke_pending_invites(org.tenant_id, &target.email)
                    .await?;
                tx.org_deny_pending_requests(org.tenant_id, target_id, actor.id)
                    .await?;
                finish(
                    &mut tx,
                    &actor,
                    &org,
                    "member_removed",
                    Some(target_id),
                    "active>true_to_false",
                )
                .await?;
                "Vínculo encerrado. O acesso será recusado na próxima operação e convites anteriores não poderão reativá-lo."
            }
        };
        let state = organization_state(&mut tx, &actor).await?;
        tx.commit().await?;
        Ok(OrganizationResponse {
            state,
            csrf: self.csrf(token),
            message: message.into(),
            invitation_code,
            access_code,
            invitation_preview,
        })
    }

    pub async fn organizations_members(
        &self,
        token: &str,
        organization_id: &str,
        context_version: i64,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<MemberSearch, AuthError> {
        self.limit("organizations-search", token, 120, 60).await?;
        if query.chars().count() > 120
            || query.chars().any(char::is_control)
            || !(0..=100_000).contains(&offset)
            || !(1..=100).contains(&limit)
        {
            return Err(invalid("Confira a busca e a página solicitada."));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let tenant = id(organization_id)?;
        let selected = tx.org_selection(actor.session).await?;
        if selected.tenant_id != Some(tenant) || selected.generation != context_version {
            return Err(context_changed());
        }
        let (_, own) = authorize(&mut tx, &actor, tenant).await?;
        require_mfa(&actor)?;
        if own.role != "administrator" {
            return Err(denied());
        }
        let boundary = ResourceBoundary::new(tenant, context_version).map_err(|_| denied())?;
        let search = boundary.search_scope(ResourceKind::Member);
        let tenant = search.tenant_id();
        let query = query.trim();
        let members = tx
            .org_members(tenant, query, limit, offset)
            .await?
            .into_iter()
            .map(member_view)
            .collect::<Result<Vec<_>, _>>()?;
        let total = tx.org_member_count(tenant, query).await?;
        tx.commit().await?;
        Ok(MemberSearch {
            organization_id: tenant.to_string(),
            context_version,
            query: query.into(),
            members,
            total,
        })
    }

    /// A exportação inteira compartilha a mesma autorização e transação de leitura.
    /// O teto é explícito, nunca se apresenta uma lista parcial como arquivo completo.
    pub async fn organizations_export(
        &self,
        token: &str,
        organization_id: &str,
        context_version: i64,
        query: &str,
    ) -> Result<String, AuthError> {
        self.limit("organizations-export", token, 12, 60).await?;
        if query.chars().count() > 120 || query.chars().any(char::is_control) {
            return Err(invalid("Confira o termo de busca."));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let tenant = id(organization_id)?;
        let selected = tx.org_selection(actor.session).await?;
        if selected.tenant_id != Some(tenant) || selected.generation != context_version {
            return Err(context_changed());
        }
        let (org, own) = authorize(&mut tx, &actor, tenant).await?;
        require_mfa(&actor)?;
        if own.role != "administrator" {
            return Err(denied());
        }
        let boundary = ResourceBoundary::new(tenant, context_version).map_err(|_| denied())?;
        let export = boundary
            .export_key(Uuid::new_v4(), org.version)
            .map_err(|_| denied())?;
        let query = query.trim();
        let total = tx.org_member_count(tenant, query).await?;
        if total > 10_000 {
            return Err(invalid(
                "Refine a busca para exportar até 10 mil membros por arquivo.",
            ));
        }
        let emitted = tx.now().await?;
        let mut output = format!(
            "Empresa\t{}\r\nIdentificador da empresa\t{}\r\nRevisão\t{}\r\nFiltro\t{}\r\nTotal de membros\t{}\r\nEmissão UTC\t{}\r\n\r\nIdentificador\tE-mail\tPapel\tSituação\tValidade\r\n",
            tsv(&org.name),
            tenant,
            org.version,
            tsv(query),
            total,
            emitted.to_rfc3339()
        );
        let mut offset = 0;
        while offset < total {
            let page = tx.org_members(tenant, query, 500, offset).await?;
            if page.is_empty() {
                return Err(AuthError::unavailable());
            }
            offset += i64::try_from(page.len()).map_err(|_| AuthError::unavailable())?;
            for member in page {
                output.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\r\n",
                    member.user_id,
                    tsv(&member.email),
                    role(&member.role)?.label(),
                    if member.effective {
                        "Ativo"
                    } else {
                        "Encerrado"
                    },
                    member
                        .valid_until
                        .map(|v| v.to_rfc3339())
                        .unwrap_or_default()
                ));
            }
        }
        tx.org_audit(
            tenant,
            actor.id,
            "members_exported",
            None,
            &format!("rows:{total};namespace:{}", export.as_str()),
        )
        .await?;
        tx.commit().await?;
        Ok(output)
    }
}

async fn lock_command_context(
    tx: &mut AuthTransaction,
    actor: &Actor,
    command: &OrganizationCommand,
) -> Result<(), AuthError> {
    let selection = tx.org_selection(actor.session).await?;
    let target = match command {
        OrganizationCommand::Select {
            organization_id,
            expected_context_version,
        } => {
            if selection.generation != *expected_context_version {
                return Err(context_changed());
            }
            let tenant = id(organization_id)?;
            membership_before_lock(tx, actor, tenant).await?;
            Some(tenant)
        }
        OrganizationCommand::Rename { scope, .. }
        | OrganizationCommand::Invite { scope, .. }
        | OrganizationCommand::RevokeInvite { scope, .. }
        | OrganizationCommand::RotateAccessCode { scope }
        | OrganizationCommand::DecideAccess { scope, .. }
        | OrganizationCommand::ChangeMember { scope, .. }
        | OrganizationCommand::RemoveMember { scope, .. } => {
            let tenant = id(&scope.organization_id)?;
            if selection.tenant_id != Some(tenant) || selection.generation != scope.context_version
            {
                return Err(context_changed());
            }
            membership_before_lock(tx, actor, tenant).await?;
            Some(tenant)
        }
        OrganizationCommand::PreviewInvitation { token }
        | OrganizationCommand::AcceptInvitation { token } => {
            Some(invitation_before_lock(tx, actor, token).await?.0)
        }
        OrganizationCommand::RequestAccess { code, .. } => {
            let (tenant, normalized) = parse_code(code).map_err(|_| denied())?;
            tx.org_set_tenant(tenant).await?;
            let org = tx.org_get(tenant, false).await?.ok_or_else(denied)?;
            if org.access_code_hash.as_deref()
                != Some(crypto::hash("organization-access", normalized).as_slice())
            {
                return Err(denied());
            }
            Some(tenant)
        }
        _ => None,
    };
    let mut tenants = target.into_iter().collect::<Vec<_>>();
    if let Some(tenant) = selection.tenant_id {
        tx.org_set_tenant(tenant).await?;
        if tx
            .org_member(tenant, actor.id)
            .await?
            .is_some_and(|m| m.effective)
        {
            tenants.push(tenant);
        }
    }
    tenants.sort();
    tenants.dedup();
    // Um convite pode se referir a outra empresa que a selecionada. Bloquear ambas
    // em ordem estável evita ciclos entre a alteração e a leitura do estado final.
    for tenant in tenants {
        tx.org_set_tenant(tenant).await?;
        let _ = tx.org_get(tenant, true).await?;
    }
    Ok(())
}

async fn membership_before_lock(
    tx: &mut AuthTransaction,
    actor: &Actor,
    tenant: Uuid,
) -> Result<(), AuthError> {
    tx.org_set_tenant(tenant).await?;
    if tx
        .org_member(tenant, actor.id)
        .await?
        .is_none_or(|m| !m.effective)
    {
        return Err(denied());
    }
    Ok(())
}

async fn authorize(
    tx: &mut AuthTransaction,
    actor: &Actor,
    tenant: Uuid,
) -> Result<(StoredOrganization, StoredMember), AuthError> {
    membership_before_lock(tx, actor, tenant).await?;
    // O bloqueio da organização serializa todas as alterações de vínculo.
    // O ator é conferido depois desse bloqueio, inclusive se esperou outro administrador.
    let org = tx.org_get(tenant, true).await?.ok_or_else(denied)?;
    let own = tx
        .org_member(tenant, actor.id)
        .await?
        .filter(|m| m.effective)
        .ok_or_else(denied)?;
    Ok((org, own))
}
async fn scoped(
    tx: &mut AuthTransaction,
    actor: &Actor,
    scope: &OrganizationScope,
    admin: bool,
) -> Result<(StoredOrganization, StoredMember), AuthError> {
    let tenant = id(&scope.organization_id)?;
    let selection = tx.org_selection(actor.session).await?;
    if selection.tenant_id != Some(tenant) || selection.generation != scope.context_version {
        return Err(context_changed());
    }
    let (org, own) = authorize(tx, actor, tenant).await?;
    if admin {
        require_mfa(actor)?;
        if own.role != "administrator" {
            return Err(denied());
        }
    }
    if org.version != scope.organization_version {
        return Err(stale());
    }
    Ok((org, own))
}
async fn finish(
    tx: &mut AuthTransaction,
    actor: &Actor,
    org: &StoredOrganization,
    event: &str,
    subject: Option<Uuid>,
    detail: &str,
) -> Result<(), AuthError> {
    if !tx.org_bump_version(org.tenant_id, org.version).await? {
        return Err(stale());
    }
    tx.org_audit(org.tenant_id, actor.id, event, subject, detail)
        .await?;
    Ok(())
}
async fn protect_last_admin(
    tx: &mut AuthTransaction,
    org: &StoredOrganization,
    target: &StoredMember,
    remains_admin: bool,
) -> Result<(), AuthError> {
    if target.active
        && target.role == "administrator"
        && !remains_admin
        && tx.org_admin_count(org.tenant_id).await? <= 1
    {
        return Err(AuthError::new(
            AuthErrorKind::Conflict,
            "last_administrator",
            "Esta empresa precisa de um administrador. Promova outro membro com verificação em duas etapas antes de retirar este vínculo.",
        ));
    }
    Ok(())
}
async fn invitation(
    tx: &mut AuthTransaction,
    actor: &Actor,
    input: &str,
) -> Result<(StoredOrganization, StoredInvite), AuthError> {
    let (tenant, _) = invitation_before_lock(tx, actor, input).await?;
    let org = tx
        .org_get(tenant, true)
        .await?
        .ok_or_else(invalid_invitation)?;
    // Uma revogação ou expiração ocorrida durante a espera prevalece.
    let (_, invite) = invitation_before_lock(tx, actor, input).await?;
    Ok((org, invite))
}
async fn invitation_before_lock(
    tx: &mut AuthTransaction,
    actor: &Actor,
    input: &str,
) -> Result<(Uuid, StoredInvite), AuthError> {
    let (tenant, normalized) = parse_code(input)?;
    tx.org_set_tenant(tenant).await?;
    let invite = tx
        .org_invite_by_hash(tenant, &crypto::hash("organization-invitation", normalized))
        .await?
        .ok_or_else(invalid_invitation)?;
    let now = tx.now().await?;
    if invite.email != actor.email
        || invite.revoked_at.is_some()
        || invite.accepted_at.is_some()
        || invite.expires_at <= now
        || invite.membership_valid_until.is_some_and(|v| v <= now)
    {
        return Err(invalid_invitation());
    }
    Ok((tenant, invite))
}
async fn organization_state(
    tx: &mut AuthTransaction,
    actor: &Actor,
) -> Result<OrganizationState, AuthError> {
    let selected = tx.org_selection(actor.session).await?;
    tx.org_clear_tenant().await?;
    let memberships = tx.org_own_memberships(actor.id).await?;
    let mut organizations = Vec::new();
    for membership in memberships {
        tx.org_set_tenant(membership.tenant_id).await?;
        if let Some(org) = tx.org_get(membership.tenant_id, false).await? {
            organizations.push(OrganizationSummary {
                id: org.tenant_id.to_string(),
                name: org.name,
                role: role(&membership.role)?,
                version: org.version,
                valid_until: membership.valid_until.map(|v| v.to_rfc3339()),
            });
        }
    }
    organizations.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    let mut generation = selected.generation;
    let mut active = None;
    if let Some(tenant) = selected.tenant_id {
        tx.org_set_tenant(tenant).await?;
        // Locks also guard snapshots against a simultaneous member revocation.
        let before = tx
            .org_member(tenant, actor.id)
            .await?
            .filter(|m| m.effective);
        let org = if before.is_some() {
            tx.org_get(tenant, true).await?
        } else {
            None
        };
        let member = if org.is_some() {
            tx.org_member(tenant, actor.id)
                .await?
                .filter(|m| m.effective)
        } else {
            None
        };
        if let (Some(org), Some(own)) = (org, member) {
            let public_org = summary(&org, &own)?;
            let admin = own.role == "administrator" && actor.mfa;
            let members = if admin {
                tx.org_members(tenant, "", 100, 0)
                    .await?
                    .into_iter()
                    .map(member_view)
                    .collect::<Result<Vec<_>, _>>()?
            } else {
                vec![member_view(own)?]
            };
            let now = tx.now().await?;
            let invitations = if admin {
                tx.org_invites(tenant)
                    .await?
                    .into_iter()
                    .map(|v| {
                        Ok(InvitationSummary {
                            id: v.id.to_string(),
                            email: v.email,
                            role: role(&v.role)?,
                            expires_at: v.expires_at.to_rfc3339(),
                            membership_valid_until: v
                                .membership_valid_until
                                .map(|d| d.to_rfc3339()),
                            status: if v.revoked_at.is_some() {
                                "revoked"
                            } else if v.accepted_at.is_some() {
                                "accepted"
                            } else if v.expires_at <= now
                                || v.membership_valid_until.is_some_and(|d| d <= now)
                            {
                                "expired"
                            } else {
                                "pending"
                            }
                            .into(),
                            version: v.version,
                        })
                    })
                    .collect::<Result<Vec<_>, AuthError>>()?
            } else {
                vec![]
            };
            let access_requests = if admin {
                tx.org_requests(tenant)
                    .await?
                    .into_iter()
                    .map(|v| {
                        Ok(AccessRequestSummary {
                            id: v.id.to_string(),
                            email: v.email,
                            requested_role: role(&v.requested_role)?,
                            status: v.status,
                            version: v.version,
                            created_at: v.created_at.to_rfc3339(),
                        })
                    })
                    .collect::<Result<Vec<_>, AuthError>>()?
            } else {
                vec![]
            };
            if let Some(item) = organizations.iter_mut().find(|v| v.id == public_org.id) {
                *item = public_org.clone();
            }
            active = Some(OrganizationDetail {
                organization: public_org,
                members,
                invitations,
                access_requests,
            });
        } else {
            organizations.retain(|v| v.id != tenant.to_string());
            if !tx.org_select(actor.session, None, generation).await? {
                return Err(context_changed());
            }
            generation += 1;
        }
    }
    Ok(OrganizationState {
        current_user_id: actor.id.to_string(),
        organizations,
        active,
        context_version: generation,
        mfa_verified: actor.mfa,
    })
}
fn tsv(value: &str) -> String {
    let clean = value.replace(['\t', '\r', '\n'], " ");
    if clean.trim_start().starts_with(['=', '+', '-', '@']) {
        format!("'{clean}")
    } else {
        clean
    }
}

#[cfg(test)]
mod tests;
