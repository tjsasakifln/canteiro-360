//! Comandos monetários de alçada, com recibo durável e reautorização em cada consulta.
use super::*;
use canteiro_persistence::{
    authority_history::{AuthorityBefore, NewAuthorityEvent, StoredAuthorityEvent},
    reliability::OperationReceipt,
};
use serde::Serialize;

fn title(value: &str) -> Result<String, AuthError> {
    let value = value.trim();
    if !(2..=200).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        return Err(invalid(
            "Descreva o pedido com 2 a 200 caracteres, sem quebras de linha.",
        ));
    }
    Ok(value.to_owned())
}

#[derive(Serialize)]
struct ProposalFields {
    title: String,
    increase: Money,
    decrease: Money,
    conflicts: Vec<Uuid>,
}

impl ProposalFields {
    fn new(
        label: &str,
        increase: Money,
        decrease: Money,
        people: &[String],
    ) -> Result<Self, AuthError> {
        if people.len() > 100 {
            return Err(invalid("Confira a lista de pessoas impedidas de decidir."));
        }
        let conflicts = people
            .iter()
            .map(|value| id(value))
            .collect::<Result<BTreeSet<_>, _>>()?;
        rules::MonetaryImpact::new(increase, decrease).map_err(rule_error)?;
        Ok(Self {
            title: title(label)?,
            increase,
            decrease,
            conflicts: conflicts.into_iter().collect(),
        })
    }

    fn require_permissions(
        &self,
        data: &Snapshot,
        actor: Uuid,
        scope: Uuid,
        approval_action: AccessAction,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        if !data.permits(
            actor,
            scope,
            proposer_action(approval_action)?,
            self.increase.max(self.decrease),
            now,
        ) || self
            .conflicts
            .iter()
            .any(|&user| !data.permits(user, scope, AccessAction::View, zero(), now))
        {
            return Err(access_denied());
        }
        Ok(())
    }

    fn apply(self, request: &mut StoredRequest, policy: &StoredPolicy) {
        request.title = self.title;
        request.increase = self.increase.to_string();
        request.decrease = self.decrease.to_string();
        request.conflicted_users = self.conflicts;
        request.policy_version = policy.version;
        request.threshold.clone_from(&policy.threshold);
        request.approvals_above_threshold = policy.approvals_above_threshold;
        request.segregate_proposer = policy.segregate_proposer;
    }
}

#[derive(Serialize)]
struct Decision {
    request_id: Uuid,
    request_version: i64,
    grant_id: Uuid,
    grant_version: i64,
    approve: bool,
}

/// A requisição já foi normalizada. Empresa e tipo são externos ao hash e
/// fazem parte da identidade persistida. A geração e a revisão da organização
/// são precondições do transporte, não dados da decisão.
#[derive(Serialize)]
enum Intent {
    Propose {
        scope: Uuid,
        action: AccessAction,
        fields: ProposalFields,
    },
    Revise {
        request_id: Uuid,
        expected_version: i64,
        fields: ProposalFields,
    },
    Decide(Decision),
}

impl Intent {
    fn parse(command: AccessCommand) -> Result<Self, AuthError> {
        match command {
            AccessCommand::Propose {
                scope_id,
                action,
                title,
                increase,
                decrease,
                conflicted_user_ids,
                ..
            } => Ok(Self::Propose {
                scope: id(&scope_id)?,
                action,
                fields: ProposalFields::new(&title, increase, decrease, &conflicted_user_ids)?,
            }),
            AccessCommand::ReviseRequest {
                request_id,
                request_version,
                title,
                increase,
                decrease,
                conflicted_user_ids,
                ..
            } => Ok(Self::Revise {
                request_id: id(&request_id)?,
                expected_version: request_version,
                fields: ProposalFields::new(&title, increase, decrease, &conflicted_user_ids)?,
            }),
            AccessCommand::Decide {
                request_id,
                request_version,
                grant_id,
                grant_version,
                approve,
                ..
            } => Ok(Self::Decide(Decision {
                request_id: id(&request_id)?,
                request_version,
                grant_id: id(&grant_id)?,
                grant_version,
                approve,
            })),
            _ => unreachable!("somente atos de alçada têm intenção monetária"),
        }
    }

    fn fingerprint(&self) -> Result<Vec<u8>, AuthError> {
        let canonical = serde_json::to_string(self).map_err(|_| AuthError::unavailable())?;
        Ok(crypto::hash("authority-intent-v1", &canonical))
    }
}

fn operation_conflict() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "idempotency_conflict",
        "Esta chave já foi usada para outro pedido ou decisão. Confira os dados e use outra chave para uma nova operação.",
    )
}

fn response_message(event: &StoredAuthorityEvent) -> &'static str {
    match (event.event_kind.as_str(), event.after_status.as_str()) {
        ("proposed", _) => {
            "Pedido registrado para decisão. Esse registro não executa pagamento nem altera o contrato."
        }
        ("revised", _) => {
            "Nova versão registrada. As decisões anteriores permanecem no histórico e não aprovam esta versão."
        }
        ("decision", "rejected") => "Pedido recusado. A decisão e a versão foram preservadas.",
        ("decision", "approved") => {
            "Alçada atendida para esta versão. A decisão não executa pagamento nem altera o contrato."
        }
        _ => "Decisão registrada. Ainda falta outra aprovação independente e válida.",
    }
}

fn require_effective_member(member: &StoredMember, now: DateTime<Utc>) -> Result<(), AuthError> {
    if member.valid_until.is_some_and(|end| end <= now) {
        return Err(access_denied());
    }
    Ok(())
}

async fn authorize_context(
    tx: &mut AuthTransaction,
    actor: &Actor,
    scope: &canteiro_contracts::reliability::ReliabilityScope,
) -> Result<(StoredOrganization, StoredMember), AuthError> {
    let tenant = id(&scope.organization_id)?;
    let selection = tx.org_selection(actor.session).await?;
    if selection.tenant_id != Some(tenant) || selection.generation != scope.context_version {
        return Err(context_changed());
    }
    authorize(tx, actor, tenant).await
}

fn operation_key(value: &str) -> Result<Uuid, AuthError> {
    let value = id(value)?;
    if value.get_version_num() != 4 || value.get_variant() != uuid::Variant::RFC4122 {
        return Err(invalid("Informe uma chave de operação UUID aleatória."));
    }
    Ok(value)
}

impl AuthService {
    pub async fn access_operation(
        &self,
        token: &str,
        scope: &canteiro_contracts::reliability::ReliabilityScope,
        act: &str,
        key: &str,
    ) -> Result<Option<AccessResponse>, AuthError> {
        self.limit("access-operation", token, 120, 60).await?;
        let key = operation_key(key)?;
        if !matches!(
            act,
            "authority_propose" | "authority_revise" | "authority_decide"
        ) {
            return Err(invalid(
                "A operação informada não é um pedido ou decisão de alçada.",
            ));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let (org, member) = authorize_context(&mut tx, &actor, scope).await?;
        if let Some(stored) = tx.operation_receipt(org.tenant_id, act, key).await? {
            let data = Snapshot::load(&mut tx, org.tenant_id).await?;
            let event = replay(self, token, &mut tx, &actor, &member, &data, &stored).await?;
            return response(self, token, tx, &actor, event).await.map(Some);
        }
        self.locked(&mut tx, token).await?;
        require_effective_member(&member, tx.now().await?)?;
        tx.commit().await?;
        Ok(None)
    }
}

pub(super) async fn execute(
    service: &AuthService,
    token: &str,
    command: AccessCommand,
    act: &str,
    key: Option<&str>,
) -> Result<AccessResponse, AuthError> {
    let (mut tx, actor) = service.organization_transaction(token).await?;
    let expected_organization = command.context().organization_version;
    let scope = canteiro_contracts::reliability::ReliabilityScope {
        organization_id: command.context().organization_id.clone(),
        context_version: command.context().context_version,
    };
    let (org, member) = authorize_context(&mut tx, &actor, &scope).await?;
    let tenant = org.tenant_id;
    let operation = key.map(operation_key).transpose()?;
    let data = Snapshot::load(&mut tx, tenant).await?;
    service.locked(&mut tx, token).await?;
    let now = tx.now().await?;
    require_effective_member(&member, now)?;
    let intent = Intent::parse(command)?;
    let fingerprint = operation.map(|_| intent.fingerprint()).transpose()?;

    if let Some(operation) = operation
        && let Some(stored) = tx.operation_receipt(tenant, act, operation).await?
    {
        let event = replay(service, token, &mut tx, &actor, &member, &data, &stored).await?;
        if Some(&stored.fingerprint) != fingerprint.as_ref() {
            return Err(operation_conflict());
        }
        return response(service, token, tx, &actor, event).await;
    }

    if org.version != expected_organization {
        return Err(stale());
    }
    let (event, audit_event, detail) = match intent {
        Intent::Propose {
            scope,
            action,
            fields,
        } => propose(&mut tx, &actor, &data, scope, action, fields, now).await?,
        Intent::Revise {
            request_id,
            expected_version,
            fields,
        } => {
            revise(
                &mut tx,
                &actor,
                &data,
                request_id,
                expected_version,
                fields,
                now,
            )
            .await?
        }
        Intent::Decide(decision) => decide(&mut tx, &actor, &data, decision, now).await?,
    };
    if let Some((operation, fingerprint)) = operation.zip(fingerprint) {
        let receipt = OperationReceipt {
            actor_id: actor.id,
            scope_id: event.scope_id,
            fingerprint,
            result_id: event.event_id,
            result_version: event.request_version,
        };
        if !tx
            .insert_operation_receipt(tenant, act, operation, &receipt)
            .await?
        {
            return Err(operation_conflict());
        }
    }
    finish(
        &mut tx,
        &actor,
        &org,
        audit_event,
        Some(event.request_id),
        &detail,
    )
    .await?;
    response(service, token, tx, &actor, event).await
}

async fn response(
    service: &AuthService,
    token: &str,
    mut tx: AuthTransaction,
    actor: &Actor,
    event: StoredAuthorityEvent,
) -> Result<AccessResponse, AuthError> {
    let state = access_state(&mut tx, actor).await?;
    let message = response_message(&event).into();
    let receipt = AuthorityReceipt {
        event_id: event.event_id.to_string(),
        request_id: event.request_id.to_string(),
        version: event.request_version,
        status: event.after_status,
        valid_approvals: u8::try_from(event.after_valid_approvals)
            .map_err(|_| AuthError::unavailable())?,
        recorded_at: event.recorded_at.to_rfc3339(),
    };
    tx.commit().await?;
    Ok(AccessResponse {
        state,
        csrf: service.csrf(token),
        message,
        receipt: Some(receipt),
    })
}

async fn replay(
    service: &AuthService,
    token: &str,
    tx: &mut AuthTransaction,
    actor: &Actor,
    member: &StoredMember,
    data: &Snapshot,
    stored: &OperationReceipt,
) -> Result<StoredAuthorityEvent, AuthError> {
    if stored.actor_id != actor.id {
        return Err(access_denied());
    }
    let event = tx
        .authority_event(data.tenant, stored.result_id, actor.id)
        .await?
        .filter(|event| {
            event.actor_id == actor.id
                && event.scope_id == stored.scope_id
                && event.request_version == stored.result_version
        })
        .ok_or_else(access_denied)?;
    let request = tx
        .access_request_version(data.tenant, event.request_id, event.request_version)
        .await?
        .filter(|request| request.scope_id == event.scope_id)
        .ok_or_else(access_denied)?;
    service.locked(tx, token).await?;
    let now = tx.now().await?;
    require_effective_member(member, now)?;
    if !data.permits(actor.id, request.scope_id, AccessAction::View, zero(), now) {
        return Err(access_denied());
    }
    let context = data.approval_context(&request)?;
    if let Some(grant_id) = event.grant_id {
        require_mfa(actor)?;
        let right = data
            .right_index
            .get(&grant_id)
            .map(|&index| &data.rights[index])
            .ok_or_else(access_denied)?;
        rules::authorize_approval(&context, right, data.source(right), actor.id, now)
            .map_err(rule_error)?;
    } else if request.proposer_id != actor.id
        || !data.permits(
            actor.id,
            request.scope_id,
            proposer_action(context.action)?,
            context.impact.authority_amount(),
            now,
        )
    {
        return Err(access_denied());
    }
    Ok(event)
}

type Mutation = (StoredAuthorityEvent, &'static str, String);

async fn propose(
    tx: &mut AuthTransaction,
    actor: &Actor,
    data: &Snapshot,
    scope: Uuid,
    action: AccessAction,
    fields: ProposalFields,
    now: DateTime<Utc>,
) -> Result<Mutation, AuthError> {
    if data.requests.len() >= 1000 {
        return Err(invalid(
            "O limite desta etapa é de 1000 pedidos de decisão por empresa.",
        ));
    }
    fields.require_permissions(data, actor.id, scope, action, now)?;
    let policy = data.policy(scope, action)?;
    let request = StoredRequest {
        id: Uuid::new_v4(),
        scope_id: scope,
        version: 1,
        status: "pending".into(),
        valid_approvals: 0,
        proposer_id: actor.id,
        proposer_email: actor.email.clone(),
        title: fields.title,
        action: action.as_str().into(),
        increase: fields.increase.to_string(),
        decrease: fields.decrease.to_string(),
        policy_version: policy.version,
        threshold: policy.threshold.clone(),
        approvals_above_threshold: policy.approvals_above_threshold,
        segregate_proposer: policy.segregate_proposer,
        conflicted_users: fields.conflicts,
    };
    if !tx.access_save_request(data.tenant, &request, None).await? {
        return Err(stale());
    }
    let event = tx
        .append_authority_event(NewAuthorityEvent {
            tenant_id: data.tenant,
            actor_id: actor.id,
            scope_id: scope,
            request_id: request.id,
            request_version: request.version,
            event_kind: "proposed",
            before: None,
            after_status: "pending",
            after_valid_approvals: 0,
            grant_id: None,
            reason: "request_registered",
        })
        .await?;
    Ok((
        event,
        "authority_requested",
        format!(
            "scope:{scope};act:{};increase:{};decrease:{};policy:{}",
            request.action, request.increase, request.decrease, policy.version
        ),
    ))
}

async fn revise(
    tx: &mut AuthTransaction,
    actor: &Actor,
    data: &Snapshot,
    request_id: Uuid,
    expected_version: i64,
    fields: ProposalFields,
    now: DateTime<Utc>,
) -> Result<Mutation, AuthError> {
    let before = data
        .requests
        .iter()
        .find(|request| request.id == request_id && request.proposer_id == actor.id)
        .ok_or_else(access_denied)?;
    let action = parse(&before.action)?;
    fields.require_permissions(data, actor.id, before.scope_id, action, now)?;
    if before.version != expected_version || before.status != "pending" {
        return Err(stale());
    }
    let policy = data.policy(before.scope_id, action)?;
    let mut revised = before.clone();
    revised.version = before.version.checked_add(1).ok_or_else(stale)?;
    fields.apply(&mut revised, policy);
    if !tx
        .access_save_request(data.tenant, &revised, Some(expected_version))
        .await?
    {
        return Err(stale());
    }
    let event = tx
        .append_authority_event(NewAuthorityEvent {
            tenant_id: data.tenant,
            actor_id: actor.id,
            scope_id: revised.scope_id,
            request_id,
            request_version: revised.version,
            event_kind: "revised",
            before: Some(before_state(before)),
            after_status: "pending",
            after_valid_approvals: 0,
            grant_id: None,
            reason: "request_revised",
        })
        .await?;
    Ok((
        event,
        "authority_revised",
        format!(
            "version:{};increase:{};decrease:{};policy:{}",
            revised.version, revised.increase, revised.decrease, policy.version
        ),
    ))
}

async fn decide(
    tx: &mut AuthTransaction,
    actor: &Actor,
    data: &Snapshot,
    decision: Decision,
    now: DateTime<Utc>,
) -> Result<Mutation, AuthError> {
    require_mfa(actor)?;
    let Decision {
        request_id,
        request_version,
        grant_id,
        grant_version,
        approve,
    } = decision;
    let request = data
        .requests
        .iter()
        .find(|request| request.id == request_id)
        .ok_or_else(access_denied)?;
    if !data.permits(actor.id, request.scope_id, AccessAction::View, zero(), now) {
        return Err(access_denied());
    }
    if request.version != request_version || request.status != "pending" {
        return Err(stale());
    }
    let policy = data.policy(request.scope_id, parse(&request.action)?)?;
    if policy.version != request.policy_version {
        return Err(AuthError::new(
            AuthErrorKind::Conflict,
            "policy_changed",
            "A regra de aprovação mudou. O proponente precisa revisar o pedido antes de uma nova decisão.",
        ));
    }
    let grant = data
        .grants
        .iter()
        .find(|grant| grant.id == grant_id && grant.user_id == actor.id)
        .ok_or_else(access_denied)?;
    if grant.version != grant_version {
        return Err(stale());
    }
    let right = data
        .right_index
        .get(&grant.id)
        .map(|&index| &data.rights[index])
        .ok_or_else(access_denied)?;
    let context = data.approval_context(request)?;
    rules::authorize_approval(&context, right, data.source(right), actor.id, now)
        .map_err(rule_error)?;
    let approval = StoredApproval {
        request_id,
        request_version,
        actor_id: actor.id,
        email: actor.email.clone(),
        grant_id,
        grant_version,
        approved: approve,
        created_at: now,
    };
    if !tx.access_decide(data.tenant, &approval).await? {
        return Err(AuthError::new(
            AuthErrorKind::Conflict,
            "already_decided",
            "Sua decisão já foi registrada nesta versão. Atualize o pedido para conferir o resultado.",
        ));
    }
    let mut records = data.records();
    if approve {
        records.push(rules::RecordedApproval {
            proposal_id: request_id,
            actor_id: actor.id,
            grant_id,
            version: request_version,
        });
    }
    let quorum =
        rules::approval_quorum(&context, &records, &data.rights, now).map_err(rule_error)?;
    let (status, reason) = match (approve, quorum.approved) {
        (false, _) => ("rejected", "request_rejected"),
        (true, true) => ("approved", "authority_completed"),
        (true, false) => ("pending", "approval_recorded"),
    };
    if !tx
        .access_request_status(data.tenant, request, status, quorum.valid)
        .await?
    {
        return Err(stale());
    }
    let event = tx
        .append_authority_event(NewAuthorityEvent {
            tenant_id: data.tenant,
            actor_id: actor.id,
            scope_id: request.scope_id,
            request_id,
            request_version,
            event_kind: "decision",
            before: Some(before_state(request)),
            after_status: status,
            after_valid_approvals: i16::from(quorum.valid),
            grant_id: Some(grant_id),
            reason,
        })
        .await?;
    Ok((
        event,
        "authority_decided",
        format!(
            "version:{request_version};grant:{grant_id};grant_version:{grant_version};approve:{approve};status:{status};valid:{};required:{}",
            quorum.valid, quorum.required
        ),
    ))
}

fn before_state(request: &StoredRequest) -> AuthorityBefore<'_> {
    AuthorityBefore {
        version: request.version,
        status: &request.status,
        valid_approvals: request.valid_approvals,
    }
}
