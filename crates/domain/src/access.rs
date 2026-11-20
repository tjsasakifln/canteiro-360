//! Permissões por vínculo e escopo exatos, sem consultas ou relógio implícito.
//!
//! A aplicação fornece a identidade autenticada, a hora do banco e os vínculos
//! atuais dentro da mesma transação da operação. Participar de uma organização
//! não é uma concessão e não autoriza nenhum dos atos definidos aqui.
use crate::decimal::Money;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AccessRole {
    Viewer,
    Manager,
    Budgeter,
    Inspector,
    TechnicalResponsible,
    Approver,
    Finance,
    Supplier,
    Client,
    Support,
}

impl AccessRole {
    pub const ALL: [Self; 10] = [
        Self::Viewer,
        Self::Manager,
        Self::Budgeter,
        Self::Inspector,
        Self::TechnicalResponsible,
        Self::Approver,
        Self::Finance,
        Self::Supplier,
        Self::Client,
        Self::Support,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Manager => "manager",
            Self::Budgeter => "budgeter",
            Self::Inspector => "inspector",
            Self::TechnicalResponsible => "technical_responsible",
            Self::Approver => "approver",
            Self::Finance => "finance",
            Self::Supplier => "supplier",
            Self::Client => "client",
            Self::Support => "support",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Leitor",
            Self::Manager => "Gestor",
            Self::Budgeter => "Orçamentista",
            Self::Inspector => "Fiscal",
            Self::TechnicalResponsible => "Responsável técnico",
            Self::Approver => "Aprovador",
            Self::Finance => "Financeiro",
            Self::Supplier => "Fornecedor",
            Self::Client => "Cliente",
            Self::Support => "Suporte",
        }
    }

    /// Limite do papel, ainda sujeito à lista expressa de atos do vínculo.
    /// Gestão, administração da empresa e suporte não recebem aprovação implícita.
    pub const fn permits(self, action: AccessAction) -> bool {
        use AccessAction as A;
        if matches!(action, A::View) {
            return true;
        }
        match self {
            Self::Viewer | Self::Client | Self::Support => false,
            Self::Manager => matches!(
                action,
                A::ProposeChange
                    | A::EditContractRecord
                    | A::ManagePortfolio
                    | A::EditProject
                    | A::ManageTechnicalObligations
            ),
            Self::Budgeter => {
                matches!(action, A::EditBudget | A::ReviewBudget | A::ProposeChange)
            }
            Self::Inspector => matches!(
                action,
                A::RecordExecution | A::ReviewMeasurement | A::ProposeChange
            ),
            Self::TechnicalResponsible => matches!(
                action,
                A::RecordExecution
                    | A::ReviewMeasurement
                    | A::ProposeChange
                    | A::TechnicalAccept
                    | A::EditProject
                    | A::ManageTechnicalObligations
            ),
            Self::Approver => matches!(
                action,
                A::ApproveBudget
                    | A::ApproveChange
                    | A::AuthorizePayment
                    | A::ApproveProjectScope
                    | A::ApproveTechnicalException
            ),
            Self::Finance => matches!(action, A::RecordPayment),
            Self::Supplier => matches!(action, A::RecordExecution | A::ProposeChange),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AccessAction {
    View,
    EditBudget,
    ReviewBudget,
    RecordExecution,
    ReviewMeasurement,
    ProposeChange,
    ApproveBudget,
    ApproveChange,
    AuthorizePayment,
    RecordPayment,
    TechnicalAccept,
    EditContractRecord,
    ManagePortfolio,
    EditProject,
    ApproveProjectScope,
    ManageTechnicalObligations,
    ApproveTechnicalException,
}

impl AccessAction {
    pub const ALL: [Self; 17] = [
        Self::View,
        Self::EditBudget,
        Self::ReviewBudget,
        Self::RecordExecution,
        Self::ReviewMeasurement,
        Self::ProposeChange,
        Self::ApproveBudget,
        Self::ApproveChange,
        Self::AuthorizePayment,
        Self::RecordPayment,
        Self::TechnicalAccept,
        Self::EditContractRecord,
        Self::ManagePortfolio,
        Self::EditProject,
        Self::ApproveProjectScope,
        Self::ManageTechnicalObligations,
        Self::ApproveTechnicalException,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::EditBudget => "edit_budget",
            Self::ReviewBudget => "review_budget",
            Self::RecordExecution => "record_execution",
            Self::ReviewMeasurement => "review_measurement",
            Self::ProposeChange => "propose_change",
            Self::ApproveBudget => "approve_budget",
            Self::ApproveChange => "approve_change",
            Self::AuthorizePayment => "authorize_payment",
            Self::RecordPayment => "record_payment",
            Self::TechnicalAccept => "technical_accept",
            Self::EditContractRecord => "edit_contract_record",
            Self::ManagePortfolio => "manage_portfolio",
            Self::EditProject => "edit_project",
            Self::ApproveProjectScope => "approve_project_scope",
            Self::ManageTechnicalObligations => "manage_technical_obligations",
            Self::ApproveTechnicalException => "approve_technical_exception",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::View => "Consultar",
            Self::EditBudget => "Editar orçamento",
            Self::ReviewBudget => "Conferir orçamento",
            Self::RecordExecution => "Registrar execução",
            Self::ReviewMeasurement => "Conferir medição",
            Self::ProposeChange => "Propor alteração",
            Self::ApproveBudget => "Aprovar orçamento",
            Self::ApproveChange => "Aprovar alteração",
            Self::AuthorizePayment => "Autorizar pagamento",
            Self::RecordPayment => "Registrar pagamento",
            Self::TechnicalAccept => "Dar aceite técnico",
            Self::EditContractRecord => "Editar referência do contrato",
            Self::ManagePortfolio => "Gerir carteira",
            Self::EditProject => "Gerir projeto",
            Self::ApproveProjectScope => "Aprovar escopo",
            Self::ManageTechnicalObligations => "Gerir obrigações técnicas",
            Self::ApproveTechnicalException => "Aprovar exceção técnica",
        }
    }

    pub const fn is_approval(self) -> bool {
        matches!(
            self,
            Self::ApproveBudget
                | Self::ApproveChange
                | Self::AuthorizePayment
                | Self::TechnicalAccept
                | Self::ApproveProjectScope
                | Self::ApproveTechnicalException
        )
    }

    /// Restringe os novos atos ao tipo de registro que lhes dá sentido.
    /// Um vínculo concedido em uma obra nunca libera outro escopo por herança.
    pub const fn applies_to(self, kind: ScopeKind) -> bool {
        match self {
            Self::EditContractRecord => matches!(kind, ScopeKind::Contract),
            Self::ManagePortfolio => matches!(kind, ScopeKind::Work),
            Self::EditProject | Self::ApproveProjectScope => matches!(kind, ScopeKind::Project),
            _ => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Work,
    Contract,
    Budget,
    Project,
}

impl ScopeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Contract => "contract",
            Self::Budget => "budget",
            Self::Project => "project",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Work => "Obra",
            Self::Contract => "Contrato",
            Self::Budget => "Orçamento",
            Self::Project => "Projeto",
        }
    }
}

/// Não há herança automática de obra para contrato nem de contrato para orçamento.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct AccessScope {
    pub tenant_id: Uuid,
    pub kind: ScopeKind,
    pub id: Uuid,
}

impl AccessScope {
    pub fn validate(self) -> Result<(), AccessError> {
        valid_id(self.tenant_id)?;
        valid_id(self.id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AccessError {
    #[error("O identificador informado não é válido.")]
    InvalidIdentity,
    #[error("Os valores de acréscimo, redução e alçada não podem ser negativos.")]
    InvalidAmount,
    #[error("Informe início e término de validade em ordem válida.")]
    InvalidPeriod,
    #[error("O vínculo precisa de atos explícitos, sem repetições e permitidos ao papel.")]
    InvalidGrant,
    #[error("O acesso de suporte precisa de um prazo de término.")]
    SupportRequiresExpiry,
    #[error("O vínculo não pertence à pessoa que realiza a operação.")]
    WrongIdentity,
    #[error("O vínculo não permite operar este escopo.")]
    WrongScope,
    #[error("Este ato não foi concedido ao vínculo.")]
    ActionNotGranted,
    #[error("O valor da operação ultrapassa a alçada concedida.")]
    ValueLimit,
    #[error("O vínculo foi revogado.")]
    Revoked,
    #[error("O vínculo não está vigente neste momento.")]
    NotEffective,
    #[error("A concessão original da delegação não está disponível.")]
    MissingSource,
    #[error("A delegação precisa de titular distinto e de uma concessão direta válida.")]
    InvalidDelegation,
    #[error("A delegação ultrapassa os atos, o valor, o escopo ou o prazo de quem delega.")]
    DelegationExceedsAuthority,
    #[error("A política deve exigir uma ou duas aprovações e um limiar não negativo.")]
    InvalidPolicy,
    #[error("A versão do pedido não é válida.")]
    InvalidVersion,
    #[error("Este ato não é uma aprovação.")]
    NotApproval,
    #[error("A segregação exige outra pessoa sem o conflito de interesse identificado.")]
    ConflictOfInterest,
    #[error("Há concessões com o mesmo identificador. Atualize os vínculos antes de decidir.")]
    DuplicateGrant,
}

fn valid_id(value: Uuid) -> Result<(), AccessError> {
    if value.is_nil() || value == Uuid::max() {
        Err(AccessError::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn nonnegative(value: Money) -> Result<(), AccessError> {
    if value.value().is_negative() {
        Err(AccessError::InvalidAmount)
    } else {
        Ok(())
    }
}

/// Acréscimo e redução são magnitudes independentes, nunca compensadas.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(try_from = "RawMonetaryImpact")]
pub struct MonetaryImpact {
    increase: Money,
    decrease: Money,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMonetaryImpact {
    increase: Money,
    decrease: Money,
}

impl TryFrom<RawMonetaryImpact> for MonetaryImpact {
    type Error = AccessError;

    fn try_from(value: RawMonetaryImpact) -> Result<Self, Self::Error> {
        Self::new(value.increase, value.decrease)
    }
}

impl MonetaryImpact {
    pub fn new(increase: Money, decrease: Money) -> Result<Self, AccessError> {
        nonnegative(increase)?;
        nonnegative(decrease)?;
        Ok(Self { increase, decrease })
    }

    pub fn increase(self) -> Money {
        self.increase
    }
    pub fn decrease(self) -> Money {
        self.decrease
    }

    /// Regra conservadora do produto: a maior magnitude define a alçada.
    /// Mesmo quando o líquido é zero, +100/-100 exige alçada de 100.
    pub fn authority_amount(self) -> Money {
        self.increase.max(self.decrease)
    }
    pub fn is_increase(self) -> bool {
        !self.increase.value().is_zero()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ApprovalRule {
    pub threshold: Money,
    pub approvals_above_threshold: u8,
    pub segregate_proposer: bool,
}

impl ApprovalRule {
    pub fn validate(self) -> Result<(), AccessError> {
        if self.threshold.value().is_negative()
            || !(1..=2).contains(&self.approvals_above_threshold)
        {
            return Err(AccessError::InvalidPolicy);
        }
        Ok(())
    }

    /// O limiar pertence à faixa que exige a quantidade configurada.
    pub fn required_approvals(self, impact: MonetaryImpact) -> Result<u8, AccessError> {
        self.validate()?;
        Ok(if impact.authority_amount() >= self.threshold {
            self.approvals_above_threshold
        } else {
            1
        })
    }
}

/// Fotografia atual do vínculo. O banco pode guardar outros campos de auditoria.
/// A aplicação deve marcar como revogado o vínculo de quem saiu da organização.
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AccessGrant {
    pub id: Uuid,
    pub user_id: Uuid,
    pub scope: AccessScope,
    pub role: AccessRole,
    pub actions: Vec<AccessAction>,
    pub valid_from: DateTime<Utc>,
    pub valid_until: Option<DateTime<Utc>>,
    pub max_amount: Money,
    pub revoked: bool,
    pub source_grant_id: Option<Uuid>,
}

pub fn validate_grant(grant: &AccessGrant) -> Result<(), AccessError> {
    valid_id(grant.id)?;
    valid_id(grant.user_id)?;
    grant.scope.validate()?;
    nonnegative(grant.max_amount)?;
    if grant.valid_until.is_some_and(|end| end <= grant.valid_from) {
        return Err(AccessError::InvalidPeriod);
    }
    if grant.role == AccessRole::Support && grant.valid_until.is_none() {
        return Err(AccessError::SupportRequiresExpiry);
    }
    let distinct: BTreeSet<_> = grant.actions.iter().copied().collect();
    if distinct.is_empty()
        || distinct.len() != grant.actions.len()
        || grant
            .actions
            .iter()
            .any(|action| !grant.role.permits(*action) || !action.applies_to(grant.scope.kind))
    {
        return Err(AccessError::InvalidGrant);
    }
    if let Some(source) = grant.source_grant_id {
        valid_id(source)?;
        if source == grant.id {
            return Err(AccessError::InvalidDelegation);
        }
    }
    Ok(())
}

fn effective(grant: &AccessGrant, at: DateTime<Utc>) -> Result<(), AccessError> {
    if grant.revoked {
        return Err(AccessError::Revoked);
    }
    if at < grant.valid_from || grant.valid_until.is_some_and(|end| at >= end) {
        return Err(AccessError::NotEffective);
    }
    Ok(())
}

/// Valida a emissão ou reavaliação de delegação. O delegante deve estar vigente
/// agora; o início do delegado pode ser futuro, sempre dentro do prazo da fonte.
/// Não há redelegação. Uma delegação precisa de término explícito. Acesso
/// exclusivamente de consulta não permite conceder acesso a outra pessoa.
pub fn validate_delegation(
    delegated: &AccessGrant,
    source: &AccessGrant,
    at: DateTime<Utc>,
) -> Result<(), AccessError> {
    validate_grant(delegated)?;
    validate_grant(source)?;
    if delegated.source_grant_id != Some(source.id)
        || source.source_grant_id.is_some()
        || delegated.user_id == source.user_id
    {
        return Err(AccessError::InvalidDelegation);
    }
    effective(source, at)?;
    if source
        .actions
        .iter()
        .all(|action| *action == AccessAction::View)
    {
        return Err(AccessError::ActionNotGranted);
    }
    if delegated.revoked {
        return Err(AccessError::Revoked);
    }
    let Some(delegated_end) = delegated.valid_until else {
        return Err(AccessError::DelegationExceedsAuthority);
    };
    if delegated.scope != source.scope
        || delegated.max_amount > source.max_amount
        || delegated.valid_from < source.valid_from
        || source.valid_until.is_some_and(|end| delegated_end > end)
        || delegated
            .actions
            .iter()
            .any(|action| !source.actions.contains(action))
    {
        return Err(AccessError::DelegationExceedsAuthority);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionRequest {
    pub actor_id: Uuid,
    pub scope: AccessScope,
    pub action: AccessAction,
    pub amount: Money,
    pub at: DateTime<Utc>,
}

/// Resultado efêmero de uma decisão, sem desserialização de entradas externas.
/// Não transportar como autorização para operações posteriores.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthorizedAccess {
    pub actor_id: Uuid,
    pub grant_id: Uuid,
    pub authority_holder: Uuid,
    pub root_grant_id: Uuid,
}

pub fn authorize(
    grant: &AccessGrant,
    source: Option<&AccessGrant>,
    request: &PermissionRequest,
) -> Result<AuthorizedAccess, AccessError> {
    valid_id(request.actor_id)?;
    request.scope.validate()?;
    nonnegative(request.amount)?;
    validate_grant(grant)?;
    if grant.user_id != request.actor_id {
        return Err(AccessError::WrongIdentity);
    }
    if grant.scope != request.scope {
        return Err(AccessError::WrongScope);
    }
    effective(grant, request.at)?;
    if !grant.role.permits(request.action) || !grant.actions.contains(&request.action) {
        return Err(AccessError::ActionNotGranted);
    }
    if request.amount > grant.max_amount {
        return Err(AccessError::ValueLimit);
    }
    let origin = match (grant.source_grant_id, source) {
        (None, None) => grant,
        (Some(_), Some(source)) => {
            validate_delegation(grant, source, request.at)?;
            source
        }
        (Some(_), None) => return Err(AccessError::MissingSource),
        (None, Some(_)) => return Err(AccessError::InvalidDelegation),
    };
    Ok(AuthorizedAccess {
        actor_id: request.actor_id,
        grant_id: grant.id,
        authority_holder: origin.user_id,
        root_grant_id: origin.id,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalContext {
    pub proposal_id: Uuid,
    pub scope: AccessScope,
    pub action: AccessAction,
    pub version: i64,
    pub proposer_id: Uuid,
    pub impact: MonetaryImpact,
    pub rule: ApprovalRule,
    /// Impedimentos explícitos, além da segregação do proponente quando exigida.
    pub conflicted_users: Vec<Uuid>,
}

impl ApprovalContext {
    pub fn validate(&self) -> Result<(), AccessError> {
        valid_id(self.proposal_id)?;
        valid_id(self.proposer_id)?;
        self.scope.validate()?;
        self.rule.validate()?;
        if self.version <= 0 {
            return Err(AccessError::InvalidVersion);
        }
        if !self.action.is_approval() {
            return Err(AccessError::NotApproval);
        }
        for user in &self.conflicted_users {
            valid_id(*user)?;
        }
        Ok(())
    }
}

/// O vínculo e a proposta são referências históricas, jamais prova de acesso.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RecordedApproval {
    pub proposal_id: Uuid,
    pub actor_id: Uuid,
    pub grant_id: Uuid,
    pub version: i64,
}

pub fn authorize_approval(
    context: &ApprovalContext,
    grant: &AccessGrant,
    source: Option<&AccessGrant>,
    actor_id: Uuid,
    now: DateTime<Utc>,
) -> Result<AuthorizedAccess, AccessError> {
    context.validate()?;
    let permitted = authorize(
        grant,
        source,
        &PermissionRequest {
            actor_id,
            scope: context.scope,
            action: context.action,
            amount: context.impact.authority_amount(),
            at: now,
        },
    )?;
    let participants = [permitted.actor_id, permitted.authority_holder];
    if participants
        .iter()
        .any(|user| context.conflicted_users.contains(user))
        || context.rule.segregate_proposer && participants.contains(&context.proposer_id)
    {
        return Err(AccessError::ConflictOfInterest);
    }
    Ok(permitted)
}

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
pub struct ApprovalQuorum {
    pub required: u8,
    /// Limitado a duas aprovações independentes, suficiente para esta política.
    pub valid: u8,
    pub approved: bool,
    pub approvers: Vec<Uuid>,
}

fn independent(first: AuthorizedAccess, second: AuthorizedAccess) -> bool {
    first.actor_id != second.actor_id
        && first.authority_holder != second.authority_holder
        && first.actor_id != second.authority_holder
        && first.authority_holder != second.actor_id
}

/// Revalida as concessões atuais de cada assinatura da proposta e versão exatas.
/// Concessão ausente, revogada, expirada ou inválida não contribui para o quórum.
/// Não decide efeitos financeiros nem incorpora alterações em outros registros.
pub fn approval_quorum(
    context: &ApprovalContext,
    approvals: &[RecordedApproval],
    grants: &[AccessGrant],
    now: DateTime<Utc>,
) -> Result<ApprovalQuorum, AccessError> {
    context.validate()?;
    let required = context.rule.required_approvals(context.impact)?;
    let mut by_id = BTreeMap::new();
    for grant in grants {
        if by_id.insert(grant.id, grant).is_some() {
            return Err(AccessError::DuplicateGrant);
        }
    }
    let mut candidates = Vec::new();
    for approval in approvals {
        if approval.proposal_id != context.proposal_id || approval.version != context.version {
            continue;
        }
        let Some(grant) = by_id.get(&approval.grant_id) else {
            continue;
        };
        let source = grant.source_grant_id.and_then(|id| by_id.get(&id).copied());
        if let Ok(authorized) = authorize_approval(context, grant, source, approval.actor_id, now) {
            candidates.push(authorized);
        }
    }
    candidates.sort_by_key(|c| (c.actor_id, c.authority_holder, c.grant_id));
    candidates.dedup();
    // Procurar o par inteiro evita que uma escolha gulosa esconda um par válido
    // quando uma pessoa possui mais de um vínculo ou também uma substituição.
    let mut selected = candidates
        .first()
        .map_or_else(Vec::new, |first| vec![first.actor_id]);
    'pairs: for (position, first) in candidates.iter().enumerate() {
        for second in &candidates[position + 1..] {
            if independent(*first, *second) {
                selected = vec![first.actor_id, second.actor_id];
                break 'pairs;
            }
        }
    }
    let valid = if selected.len() == 2 {
        2
    } else if selected.is_empty() {
        0
    } else {
        1
    };
    Ok(ApprovalQuorum {
        required,
        valid,
        approved: valid >= required,
        approvers: selected,
    })
}
