//! Configuração e decisões de alçada sob o mesmo bloqueio de organização da gestão de membros.
use super::*;
use canteiro_contracts::access::*;
use canteiro_domain::{UtcInstant, access as rules, decimal::Money};
use canteiro_persistence::access::{
    StoredApproval, StoredGrant, StoredPolicy, StoredRequest, StoredScope,
};
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, BTreeSet};

mod authority;

fn access_denied() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "access_denied",
        "Seu vínculo não permite esta ação nesta obra ou contrato. Peça ao administrador da empresa a conferência do papel, da validade e do limite de valor.",
    )
}
pub(super) fn responsibility_required() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "responsibility_required",
        "Cadastre outro titular ativo, sem vencimento, para a mesma responsabilidade antes de encerrar este vínculo.",
    )
}
fn rule_error(error: rules::AccessError) -> AuthError {
    use rules::AccessError as E;
    match error {
        E::ConflictOfInterest => AuthError::new(
            AuthErrorKind::Forbidden,
            "conflict_of_interest",
            "Este pedido precisa de outra pessoa, sem conflito de interesse. A delegação não elimina o impedimento de quem a concedeu.",
        ),
        E::ValueLimit => AuthError::new(
            AuthErrorKind::Forbidden,
            "authority_limit",
            "O maior valor entre acréscimo e redução ultrapassa sua alçada. Solicite a decisão de um responsável com limite suficiente.",
        ),
        E::InvalidAmount => invalid(
            "Acréscimo, redução e alçada devem ser valores não negativos, com até duas casas decimais.",
        ),
        E::InvalidPeriod | E::NotEffective => invalid(
            "Confira o início e o término do vínculo. A permissão precisa estar vigente no momento da operação.",
        ),
        E::InvalidPolicy => invalid(
            "A política deve exigir uma ou duas pessoas e ter um limite de valor não negativo.",
        ),
        E::SupportRequiresExpiry => invalid("O acesso de suporte precisa de uma data de término."),
        E::DelegationExceedsAuthority | E::InvalidDelegation | E::MissingSource => invalid(
            "A delegação exige outra pessoa e não pode ampliar os atos, o valor, a obra, o contrato ou a validade da concessão original. Não é permitido delegar uma delegação.",
        ),
        E::InvalidGrant => invalid(
            "Selecione os atos permitidos ao papel, sem repetição, e mantenha o direito de consulta.",
        ),
        _ => access_denied(),
    }
}
fn parse<T: DeserializeOwned>(value: &str) -> Result<T, AuthError> {
    serde_json::from_value(serde_json::Value::String(value.to_owned()))
        .map_err(|_| AuthError::unavailable())
}
fn money(value: &str) -> Result<Money, AuthError> {
    value.parse().map_err(|_| AuthError::unavailable())
}
pub(super) fn zero() -> Money {
    "0".parse().expect("zero monetário válido")
}
fn instant(value: &str) -> Result<DateTime<Utc>, AuthError> {
    value
        .parse::<UtcInstant>()
        .map(|v| v.as_datetime())
        .map_err(|_| {
            invalid("Informe data e horário com fuso explícito e até seis casas de segundo.")
        })
}
fn proposer_action(action: AccessAction) -> Result<AccessAction, AuthError> {
    match action {
        AccessAction::ApproveBudget => Ok(AccessAction::EditBudget),
        AccessAction::ApproveChange => Ok(AccessAction::ProposeChange),
        AccessAction::AuthorizePayment => Ok(AccessAction::RecordPayment),
        AccessAction::TechnicalAccept => Ok(AccessAction::ReviewMeasurement),
        AccessAction::ApproveProjectScope => Ok(AccessAction::EditProject),
        AccessAction::ApproveTechnicalException => Ok(AccessAction::ManageTechnicalObligations),
        _ => Err(invalid(
            "Selecione uma decisão de orçamento, alteração, pagamento ou aceite técnico.",
        )),
    }
}

pub(super) struct Snapshot {
    tenant: Uuid,
    pub(super) scopes: Vec<StoredScope>,
    grants: Vec<StoredGrant>,
    rights: Vec<rules::AccessGrant>,
    right_index: BTreeMap<Uuid, usize>,
    policies: Vec<StoredPolicy>,
    requests: Vec<StoredRequest>,
    approvals: Vec<StoredApproval>,
}
impl Snapshot {
    async fn load(tx: &mut AuthTransaction, tenant: Uuid) -> Result<Self, AuthError> {
        let mut data = Self::load_permissions(tx, tenant).await?;
        data.policies = tx.access_policies(tenant).await?;
        data.requests = tx.access_requests(tenant).await?;
        data.approvals = tx.access_approvals(tenant).await?;
        Ok(data)
    }

    pub(super) async fn load_permissions(
        tx: &mut AuthTransaction,
        tenant: Uuid,
    ) -> Result<Self, AuthError> {
        let scopes = tx.access_scopes(tenant).await?;
        let grants = tx.access_grants(tenant).await?;
        let mut data = Self {
            tenant,
            scopes,
            grants,
            rights: vec![],
            right_index: BTreeMap::new(),
            policies: vec![],
            requests: vec![],
            approvals: vec![],
        };
        data.rights = data
            .grants
            .iter()
            .map(|g| data.right(g))
            .collect::<Result<_, _>>()?;
        data.right_index = data
            .rights
            .iter()
            .enumerate()
            .map(|(i, g)| (g.id, i))
            .collect();
        let source_ends: BTreeMap<_, _> = data
            .rights
            .iter()
            .filter(|g| g.source_grant_id.is_none())
            .map(|g| (g.id, g.valid_until))
            .collect();
        for grant in &mut data.rights {
            if let Some(Some(source_end)) =
                grant.source_grant_id.and_then(|id| source_ends.get(&id))
            {
                grant.valid_until = Some(
                    grant
                        .valid_until
                        .map_or(*source_end, |end| end.min(*source_end)),
                );
            }
        }
        Ok(data)
    }
    fn scope(&self, id: Uuid) -> Result<rules::AccessScope, AuthError> {
        let scope = self
            .scopes
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(access_denied)?;
        Ok(rules::AccessScope {
            tenant_id: self.tenant,
            kind: parse(&scope.kind)?,
            id,
        })
    }
    fn right(&self, g: &StoredGrant) -> Result<rules::AccessGrant, AuthError> {
        Ok(rules::AccessGrant {
            id: g.id,
            user_id: g.user_id,
            scope: self.scope(g.scope_id)?,
            role: parse(&g.role)?,
            actions: g
                .actions
                .iter()
                .map(|a| parse(a))
                .collect::<Result<_, _>>()?,
            max_amount: money(&g.max_amount)?,
            valid_from: g.valid_from,
            valid_until: match (g.valid_until, g.member_valid_until) {
                (Some(grant), Some(member)) => Some(grant.min(member)),
                (grant, member) => grant.or(member),
            },
            revoked: g.revoked_at.is_some() || !g.member_effective,
            source_grant_id: g.source_grant_id,
        })
    }
    fn source(&self, g: &rules::AccessGrant) -> Option<&rules::AccessGrant> {
        g.source_grant_id
            .and_then(|id| self.right_index.get(&id))
            .and_then(|i| self.rights.get(*i))
    }
    pub(super) fn permits(
        &self,
        user: Uuid,
        scope: Uuid,
        action: AccessAction,
        amount: Money,
        now: DateTime<Utc>,
    ) -> bool {
        let Ok(scope) = self.scope(scope) else {
            return false;
        };
        self.rights
            .iter()
            .filter(|g| g.user_id == user && g.scope == scope)
            .any(|g| {
                rules::authorize(
                    g,
                    self.source(g),
                    &rules::PermissionRequest {
                        actor_id: user,
                        scope,
                        action,
                        amount,
                        at: now,
                    },
                )
                .is_ok()
            })
    }
    fn approval_context(&self, r: &StoredRequest) -> Result<rules::ApprovalContext, AuthError> {
        Ok(rules::ApprovalContext {
            proposal_id: r.id,
            scope: self.scope(r.scope_id)?,
            action: parse(&r.action)?,
            version: r.version,
            proposer_id: r.proposer_id,
            impact: rules::MonetaryImpact::new(money(&r.increase)?, money(&r.decrease)?)
                .map_err(rule_error)?,
            rule: rules::ApprovalRule {
                threshold: money(&r.threshold)?,
                approvals_above_threshold: u8::try_from(r.approvals_above_threshold)
                    .map_err(|_| AuthError::unavailable())?,
                segregate_proposer: r.segregate_proposer,
            },
            conflicted_users: r.conflicted_users.clone(),
        })
    }
    fn records(&self) -> Vec<rules::RecordedApproval> {
        let versions: BTreeMap<_, _> = self.grants.iter().map(|g| (g.id, g.version)).collect();
        self.approvals
            .iter()
            .filter(|a| a.approved && versions.get(&a.grant_id) == Some(&a.grant_version))
            .map(|a| rules::RecordedApproval {
                proposal_id: a.request_id,
                actor_id: a.actor_id,
                grant_id: a.grant_id,
                version: a.request_version,
            })
            .collect()
    }
    fn policy(&self, scope: Uuid, action: AccessAction) -> Result<&StoredPolicy, AuthError> {
        self.policies.iter().find(|p|p.scope_id==scope && p.action==action.as_str()).ok_or_else(||AuthError::new(AuthErrorKind::Conflict,"policy_required","Peça ao administrador para definir a regra de aprovação desta obra ou contrato antes de enviar o pedido."))
    }
}

async fn recipient(
    tx: &mut AuthTransaction,
    tenant: Uuid,
    email: &str,
    end: Option<DateTime<Utc>>,
) -> Result<StoredMember, AuthError> {
    let email = crypto::email(email)?;
    let member=tx.org_active_member_by_email(tenant,&email).await?.ok_or_else(||invalid("O destinatário precisa ser um membro ativo desta empresa. Confira o e-mail com o administrador."))?;
    if member
        .valid_until
        .is_some_and(|limit| end.is_none_or(|end| end > limit))
    {
        return Err(invalid(
            "O acesso não pode terminar depois do vínculo da pessoa com a empresa.",
        ));
    }
    Ok(member)
}
fn validate_actions(actions: &[AccessAction]) -> Result<(), AuthError> {
    if !actions.contains(&AccessAction::View) {
        return Err(invalid(
            "Inclua o direito de consulta junto aos demais atos do vínculo.",
        ));
    }
    Ok(())
}

impl AuthService {
    pub async fn access_context(&self, token: &str) -> Result<AccessResponse, AuthError> {
        self.limit("access-context", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let state = access_state(&mut tx, &actor).await?;
        tx.commit().await?;
        Ok(AccessResponse {
            state,
            csrf: self.csrf(token),
            message: String::new(),
            receipt: None,
        })
    }
    pub async fn access_execute(
        &self,
        token: &str,
        command: AccessCommand,
    ) -> Result<AccessResponse, AuthError> {
        self.access_execute_with_key(token, command, None).await
    }

    pub async fn access_execute_with_key(
        &self,
        token: &str,
        command: AccessCommand,
        key: Option<&str>,
    ) -> Result<AccessResponse, AuthError> {
        self.limit("access-command", token, 120, 60).await?;
        if let Some(act) = command.authority_act() {
            return authority::execute(self, token, command, act, key).await;
        }
        if key.is_some() {
            return Err(invalid("Esta ação não recebe uma chave de operação."));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let admin = matches!(
            &command,
            AccessCommand::CreateScope { .. }
                | AccessCommand::Grant { .. }
                | AccessCommand::SetPolicy { .. }
        );
        let (org, own) = scoped(&mut tx, &actor, command.context(), admin).await?;
        let tenant = org.tenant_id;
        let data = Snapshot::load(&mut tx, tenant).await?;
        let now = tx.now().await?;
        if own.valid_until.is_some_and(|end| end <= now) {
            return Err(access_denied());
        }
        let (event, subject, detail, message) = match command {
            AccessCommand::CreateScope {
                kind,
                name: input,
                parent_id,
                ..
            } => {
                if data.scopes.len() >= 500 {
                    return Err(invalid(
                        "O limite desta configuração é de 500 obras, contratos e orçamentos por empresa.",
                    ));
                }
                let name = name(&input)?;
                let parent = parent_id.as_deref().map(id).transpose()?;
                if match kind {
                    ScopeKind::Work | ScopeKind::Project => parent.is_some(),
                    ScopeKind::Contract => parent
                        .is_none_or(|p| !data.scopes.iter().any(|s| s.id == p && s.kind == "work")),
                    ScopeKind::Budget => parent.is_none_or(|p| {
                        !data
                            .scopes
                            .iter()
                            .any(|s| s.id == p && s.kind == "contract")
                    }),
                } {
                    return Err(invalid(
                        "Vincule o contrato a uma obra, e o orçamento a um contrato desta empresa.",
                    ));
                }
                let new_id = Uuid::new_v4();
                tx.access_create_scope(tenant, new_id, kind.as_str(), &name, parent, actor.id)
                    .await?;
                (
                    "access_scope_created",
                    new_id,
                    format!(
                        "kind:{};parent:{}",
                        kind.as_str(),
                        parent.map(|p| p.to_string()).unwrap_or_default()
                    ),
                    "Referência criada. Conceda os acessos necessários à equipe.",
                )
            }
            AccessCommand::Grant {
                scope_id,
                user_email,
                role,
                actions,
                max_amount,
                valid_from,
                valid_until,
                required,
                ..
            } => {
                if data.grants.len() >= 2000 {
                    return Err(invalid(
                        "O limite desta configuração é de 2000 vínculos por empresa.",
                    ));
                }
                validate_actions(&actions)?;
                let scope_id = id(&scope_id)?;
                let start = instant(&valid_from)?;
                let end = valid_until.as_deref().map(instant).transpose()?;
                if end.is_some_and(|e| e <= now) {
                    return Err(invalid("O término do acesso precisa estar no futuro."));
                }
                let member = recipient(&mut tx, tenant, &user_email, end).await?;
                let new_id = Uuid::new_v4();
                let grant = rules::AccessGrant {
                    id: new_id,
                    user_id: member.user_id,
                    scope: data.scope(scope_id)?,
                    role,
                    actions: actions.clone(),
                    max_amount,
                    valid_from: start,
                    valid_until: end,
                    revoked: false,
                    source_grant_id: None,
                };
                rules::validate_grant(&grant).map_err(rule_error)?;
                if required
                    && (actions.iter().all(|a| *a == AccessAction::View)
                        || end.is_some()
                        || start > now
                        || member.valid_until.is_some()
                        || !matches!(
                            role,
                            AccessRole::Manager
                                | AccessRole::TechnicalResponsible
                                | AccessRole::Approver
                        ))
                {
                    return Err(invalid(
                        "A responsabilidade obrigatória exige gestor, responsável técnico ou aprovador ativo, sem vencimento do acesso ou da participação na empresa.",
                    ));
                }
                let stored = StoredGrant {
                    id: new_id,
                    scope_id,
                    user_id: member.user_id,
                    email: member.email,
                    role: role.as_str().into(),
                    actions: actions.iter().map(|a| a.as_str().into()).collect(),
                    max_amount: max_amount.to_string(),
                    valid_from: start,
                    valid_until: end,
                    revoked_at: None,
                    source_grant_id: None,
                    substitution: false,
                    reason: String::new(),
                    version: 1,
                    member_effective: true,
                    member_valid_until: member.valid_until,
                    required,
                };
                tx.access_insert_grant(tenant, &stored, actor.id).await?;
                (
                    "access_grant_created",
                    new_id,
                    format!(
                        "scope:{scope_id};person:{};role:{};limit:{max_amount};required:{required}",
                        member.user_id,
                        role.as_str()
                    ),
                    "Acesso concedido. A pessoa só poderá atuar nesta obra ou contrato, nos atos e limites definidos.",
                )
            }
            AccessCommand::Delegate {
                source_grant_id,
                source_version,
                user_email,
                actions,
                max_amount,
                valid_from,
                valid_until,
                substitution,
                reason: input_reason,
                ..
            } => {
                require_mfa(&actor)?;
                if data.grants.len() >= 2000 {
                    return Err(invalid(
                        "O limite desta configuração é de 2000 vínculos por empresa.",
                    ));
                }
                let reason = input_reason.trim().to_owned();
                if !(2..=200).contains(&reason.chars().count())
                    || reason.chars().any(char::is_control)
                {
                    return Err(invalid(
                        "Informe o motivo da delegação ou substituição com 2 a 200 caracteres, sem quebras de linha.",
                    ));
                }
                let source_id = id(&source_grant_id)?;
                let source = data
                    .grants
                    .iter()
                    .find(|g| g.id == source_id && g.user_id == actor.id)
                    .ok_or_else(access_denied)?;
                if source.version != source_version {
                    return Err(stale());
                }
                validate_actions(&actions)?;
                let start = instant(&valid_from)?;
                let end = instant(&valid_until)?;
                if end <= now {
                    return Err(invalid("O término da delegação precisa estar no futuro."));
                }
                let member = recipient(&mut tx, tenant, &user_email, Some(end)).await?;
                let new_id = Uuid::new_v4();
                let mut grant = data.right(source)?;
                grant.id = new_id;
                grant.user_id = member.user_id;
                grant.actions = actions.clone();
                grant.max_amount = max_amount;
                grant.valid_from = start;
                grant.valid_until = Some(end);
                grant.source_grant_id = Some(source.id);
                rules::validate_delegation(&grant, &data.right(source)?, now)
                    .map_err(rule_error)?;
                let stored = StoredGrant {
                    id: new_id,
                    scope_id: source.scope_id,
                    user_id: member.user_id,
                    email: member.email,
                    role: source.role.clone(),
                    actions: actions.iter().map(|a| a.as_str().into()).collect(),
                    max_amount: max_amount.to_string(),
                    valid_from: start,
                    valid_until: Some(end),
                    revoked_at: None,
                    source_grant_id: Some(source.id),
                    substitution,
                    reason: reason.clone(),
                    version: 1,
                    member_effective: true,
                    member_valid_until: member.valid_until,
                    required: false,
                };
                tx.access_insert_grant(tenant, &stored, actor.id).await?;
                (
                    "access_delegated",
                    new_id,
                    format!(
                        "source:{};person:{};limit:{max_amount};substitution:{substitution};reason:{reason}",
                        source.id, member.user_id
                    ),
                    "Delegação registrada. Ela depende da validade do vínculo original e termina na data definida.",
                )
            }
            AccessCommand::RevokeGrant {
                grant_id,
                grant_version,
                ..
            } => {
                require_mfa(&actor)?;
                let grant_id = id(&grant_id)?;
                let grant = data
                    .grants
                    .iter()
                    .find(|g| g.id == grant_id)
                    .ok_or_else(access_denied)?;
                let delegated_by_actor = grant.source_grant_id.is_some_and(|id| {
                    data.grants
                        .iter()
                        .any(|s| s.id == id && s.user_id == actor.id)
                });
                if own.role != "administrator" && !delegated_by_actor {
                    return Err(access_denied());
                }
                if grant.version != grant_version {
                    return Err(stale());
                }
                if tx
                    .access_removal_blocked(tenant, None, Some(grant_id))
                    .await?
                {
                    return Err(responsibility_required());
                }
                if !tx
                    .access_revoke_grant(tenant, grant_id, grant_version)
                    .await?
                {
                    return Err(stale());
                }
                (
                    "access_revoked",
                    grant_id,
                    format!(
                        "scope:{};person:{};version:{grant_version}",
                        grant.scope_id, grant.user_id
                    ),
                    "Acesso encerrado. Delegações que dependem dele também deixam de autorizar operações.",
                )
            }
            AccessCommand::SetPolicy {
                scope_id,
                action,
                threshold,
                approvals_above_threshold,
                segregate_proposer,
                expected_version,
                ..
            } => {
                proposer_action(action)?;
                let scope_id = id(&scope_id)?;
                data.scope(scope_id)?;
                rules::ApprovalRule {
                    threshold,
                    approvals_above_threshold,
                    segregate_proposer,
                }
                .validate()
                .map_err(rule_error)?;
                let previous = data
                    .policies
                    .iter()
                    .find(|p| p.scope_id == scope_id && p.action == action.as_str());
                if previous.map(|p| p.version) != expected_version {
                    return Err(stale());
                }
                let version =
                    previous.map_or(Ok(1), |p| p.version.checked_add(1).ok_or_else(stale))?;
                tx.access_put_policy(
                    tenant,
                    &StoredPolicy {
                        scope_id,
                        action: action.as_str().into(),
                        threshold: threshold.to_string(),
                        approvals_above_threshold: i16::from(approvals_above_threshold),
                        segregate_proposer,
                        version,
                    },
                )
                .await?;
                let before = previous
                    .map(|p| {
                        format!(
                            "{}/{}/{}",
                            p.threshold, p.approvals_above_threshold, p.segregate_proposer
                        )
                    })
                    .unwrap_or_else(|| "none".into());
                (
                    "access_policy_changed",
                    scope_id,
                    format!(
                        "act:{};version:{version};before:{before};after:{threshold}/{approvals_above_threshold}/{segregate_proposer}",
                        action.as_str()
                    ),
                    "Regra de aprovação salva. Pedidos pendentes que usavam outra versão precisam ser revisados antes da decisão.",
                )
            }
            AccessCommand::Propose { .. }
            | AccessCommand::ReviseRequest { .. }
            | AccessCommand::Decide { .. } => {
                unreachable!("os atos de alçada usam o fluxo próprio")
            }
        };
        finish(&mut tx, &actor, &org, event, Some(subject), &detail).await?;
        let state = access_state(&mut tx, &actor).await?;
        tx.commit().await?;
        Ok(AccessResponse {
            state,
            csrf: self.csrf(token),
            message: message.into(),
            receipt: None,
        })
    }
}

async fn access_state(tx: &mut AuthTransaction, actor: &Actor) -> Result<AccessState, AuthError> {
    let selection = tx.org_selection(actor.session).await?;
    let mut state = AccessState {
        current_user_id: actor.id.to_string(),
        organization: None,
        context_version: selection.generation,
        mfa_verified: actor.mfa,
        can_manage: false,
        scopes: vec![],
        members: vec![],
        grants: vec![],
        policies: vec![],
        requests: vec![],
    };
    let Some(tenant) = selection.tenant_id else {
        return Ok(state);
    };
    tx.org_set_tenant(tenant).await?;
    if tx
        .org_member(tenant, actor.id)
        .await?
        .is_none_or(|m| !m.effective)
    {
        return Ok(state);
    }
    let (org, own) = authorize(tx, actor, tenant).await?;
    let data = Snapshot::load(tx, tenant).await?;
    let now = tx.now().await?;
    if own.valid_until.is_some_and(|end| end <= now) {
        return Ok(state);
    }
    state.organization = Some(summary(&org, &own)?);
    state.can_manage = own.role == "administrator" && actor.mfa;
    let readable: BTreeSet<Uuid> = data
        .scopes
        .iter()
        .filter(|s| data.permits(actor.id, s.id, AccessAction::View, zero(), now))
        .map(|s| s.id)
        .collect();
    for scope in &data.scopes {
        if state.can_manage || readable.contains(&scope.id) {
            state.scopes.push(ScopeView {
                id: scope.id.to_string(),
                kind: parse(&scope.kind)?,
                name: scope.name.clone(),
                parent_id: scope
                    .parent_id
                    .filter(|p| state.can_manage || readable.contains(p))
                    .map(|p| p.to_string()),
                version: scope.version,
            });
        }
    }
    let mut visible_users = BTreeSet::from([actor.id]);
    for (g, right) in data.grants.iter().zip(data.rights.iter()) {
        if !state.can_manage && !readable.contains(&g.scope_id) {
            continue;
        }
        let effective = rules::authorize(
            right,
            data.source(right),
            &rules::PermissionRequest {
                actor_id: g.user_id,
                scope: right.scope,
                action: AccessAction::View,
                amount: zero(),
                at: now,
            },
        )
        .is_ok();
        visible_users.insert(g.user_id);
        let source_owned = g.source_grant_id.is_some_and(|id| {
            data.grants
                .iter()
                .any(|s| s.id == id && s.user_id == actor.id)
        });
        state.grants.push(GrantView {
            id: g.id.to_string(),
            scope_id: g.scope_id.to_string(),
            user_id: g.user_id.to_string(),
            user_email: g.email.clone(),
            role: right.role,
            actions: right.actions.clone(),
            max_amount: right.max_amount,
            valid_from: g.valid_from.to_rfc3339(),
            valid_until: right.valid_until.map(|d| d.to_rfc3339()),
            effective,
            revoked: g.revoked_at.is_some(),
            source_grant_id: g.source_grant_id.map(|id| id.to_string()),
            substitution: g.substitution,
            reason: g.reason.clone(),
            required: g.required,
            version: g.version,
            can_delegate: actor.mfa
                && g.user_id == actor.id
                && effective
                && g.source_grant_id.is_none()
                && right.actions.iter().any(|a| *a != AccessAction::View),
            can_revoke: actor.mfa && g.revoked_at.is_none() && (state.can_manage || source_owned),
        });
    }
    for p in &data.policies {
        if state.can_manage || readable.contains(&p.scope_id) {
            state.policies.push(PolicyView {
                scope_id: p.scope_id.to_string(),
                action: parse(&p.action)?,
                threshold: money(&p.threshold)?,
                approvals_above_threshold: u8::try_from(p.approvals_above_threshold)
                    .map_err(|_| AuthError::unavailable())?,
                segregate_proposer: p.segregate_proposer,
                version: p.version,
            });
        }
    }
    let members = if state.can_manage {
        tx.org_members(tenant, "", 500, 0).await?
    } else {
        let mut members = vec![];
        for user in &visible_users {
            if let Some(m) = tx.org_member(tenant, *user).await? {
                members.push(m);
            }
        }
        members
    };
    for m in members {
        if m.effective {
            state.members.push(member_view(m)?);
        }
    }
    let records = data.records();
    for r in &data.requests {
        if !readable.contains(&r.scope_id) {
            continue;
        }
        let context = data.approval_context(r)?;
        let policy_changed = data.policy(r.scope_id, context.action)?.version != r.policy_version;
        let quorum =
            rules::approval_quorum(&context, &records, &data.rights, now).map_err(rule_error)?;
        let already_decided = data.approvals.iter().any(|a| {
            a.request_id == r.id && a.request_version == r.version && a.actor_id == actor.id
        });
        let mut allowed = vec![];
        let mut resolvers = BTreeSet::new();
        let mut reason = None;
        for (g, right) in data
            .grants
            .iter()
            .zip(data.rights.iter())
            .filter(|(_, right)| right.scope == context.scope)
        {
            match rules::authorize_approval(&context, right, data.source(right), right.user_id, now)
            {
                Ok(_) => {
                    resolvers.insert(g.email.clone());
                    if right.user_id == actor.id
                        && actor.mfa
                        && !already_decided
                        && !policy_changed
                        && r.status == "pending"
                    {
                        allowed.push(g.id.to_string());
                    }
                }
                Err(e)
                    if right.user_id == actor.id
                        && right.scope == context.scope
                        && right.actions.contains(&context.action) =>
                {
                    reason = Some(e);
                }
                _ => {}
            }
        }
        let denial = if !allowed.is_empty() || r.status != "pending" {
            None
        } else {
            let (code, message, next) = if policy_changed {
                (
                    "policy_changed",
                    "A regra de aprovação mudou.",
                    "O proponente precisa revisar este pedido.",
                )
            } else if already_decided {
                (
                    "already_decided",
                    "Sua decisão já consta nesta versão.",
                    "Aguarde outra pessoa autorizada ou confira o resultado.",
                )
            } else if !actor.mfa {
                (
                    "mfa_required",
                    "A decisão exige confirmação em duas etapas.",
                    "Ative a proteção na página de acesso e entre novamente.",
                )
            } else if reason == Some(rules::AccessError::ConflictOfInterest) {
                (
                    "conflict_of_interest",
                    "Você ou o titular da delegação está impedido de decidir este pedido.",
                    "Solicite a decisão de outra pessoa sem o conflito identificado.",
                )
            } else {
                (
                    "access_denied",
                    "Seu vínculo não autoriza esta decisão ou não tem limite suficiente.",
                    "Peça a um responsável listado para decidir ou ao administrador para conferir os acessos.",
                )
            };
            Some(AccessExplanation {
                code: code.into(),
                message: message.into(),
                next_step: next.into(),
                resolvers: resolvers.into_iter().collect(),
            })
        };
        let approvals = data
            .approvals
            .iter()
            .filter(|a| a.request_id == r.id)
            .map(|a| ApprovalView {
                user_email: a.email.clone(),
                approved: a.approved,
                decided_at: a.created_at.to_rfc3339(),
                request_version: a.request_version,
            })
            .collect();
        state.requests.push(RequestView {
            id: r.id.to_string(),
            scope_id: r.scope_id.to_string(),
            title: r.title.clone(),
            action: context.action,
            proposer_id: r.proposer_id.to_string(),
            proposer_email: r.proposer_email.clone(),
            conflicted_user_ids: r.conflicted_users.iter().map(|id| id.to_string()).collect(),
            increase: context.impact.increase(),
            decrease: context.impact.decrease(),
            authority_amount: context.impact.authority_amount(),
            contains_increase: context.impact.is_increase(),
            status: r.status.clone(),
            version: r.version,
            policy_version: r.policy_version,
            policy_changed,
            required_approvals: quorum.required,
            valid_approvals: if r.status == "pending" {
                quorum.valid
            } else {
                u8::try_from(r.valid_approvals).map_err(|_| AuthError::unavailable())?
            },
            approvals,
            allowed_grant_ids: allowed,
            can_revise: r.status == "pending"
                && r.proposer_id == actor.id
                && data.permits(
                    actor.id,
                    r.scope_id,
                    proposer_action(context.action)?,
                    context.impact.authority_amount(),
                    now,
                ),
            denial,
        });
    }
    Ok(state)
}

#[cfg(test)]
mod expiry_tests {
    use super::*;

    #[test]
    fn membership_expiry_remains_binding_after_the_database_snapshot() {
        let tenant = Uuid::new_v4();
        let scope_id = Uuid::new_v4();
        let user = Uuid::new_v4();
        let end = instant("2026-11-14T12:00:00Z").unwrap();
        let data = Snapshot {
            tenant,
            scopes: vec![StoredScope {
                id: scope_id,
                kind: "contract".into(),
                name: "Contrato fictício".into(),
                parent_id: Some(Uuid::new_v4()),
                version: 1,
            }],
            grants: vec![],
            rights: vec![],
            right_index: BTreeMap::new(),
            policies: vec![],
            requests: vec![],
            approvals: vec![],
        };
        // The SELECT ran just before membership expiry; the final authorization
        // uses the later database clock, not this previously computed boolean.
        let raw = StoredGrant {
            id: Uuid::new_v4(),
            scope_id,
            user_id: user,
            email: "fiscal@example.test".into(),
            role: "approver".into(),
            actions: vec!["view".into(), "approve_change".into()],
            max_amount: "100.00".into(),
            valid_from: end - Duration::days(1),
            valid_until: Some(end + Duration::days(10)),
            revoked_at: None,
            source_grant_id: None,
            substitution: false,
            reason: String::new(),
            version: 1,
            member_effective: true,
            member_valid_until: Some(end),
            required: false,
        };
        let grant = data.right(&raw).unwrap();
        let mut request = rules::PermissionRequest {
            actor_id: user,
            scope: grant.scope,
            action: AccessAction::ApproveChange,
            amount: "100.00".parse().unwrap(),
            at: end - Duration::microseconds(1),
        };
        assert!(rules::authorize(&grant, None, &request).is_ok());
        request.at = end;
        assert_eq!(
            rules::authorize(&grant, None, &request),
            Err(rules::AccessError::NotEffective)
        );
        // A delegated vote is equally dependent on the source person's expiry.
        let mut delegated = grant.clone();
        delegated.id = Uuid::new_v4();
        delegated.user_id = Uuid::new_v4();
        delegated.valid_until = Some(end);
        delegated.source_grant_id = Some(grant.id);
        request.actor_id = delegated.user_id;
        assert_eq!(
            rules::authorize(&delegated, Some(&grant), &request),
            Err(rules::AccessError::NotEffective)
        );
    }
}
