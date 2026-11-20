//! Todas as leituras e alterações revalidam empresa, sessão e concessão explícita.
use super::{
    access::{Snapshot, zero},
    *,
};
use canteiro_contracts::{access::AccessAction, projects::*, reliability::ReliabilityScope};
use canteiro_domain::{EntityId, projects::ProjectError};
use canteiro_persistence::{
    access::StoredGrant,
    projects::{ProjectWrite, StoredProjectVersion},
};
use std::collections::BTreeSet;

pub(super) struct RecordAccess {
    pub tenant: Uuid,
    pub actor: Actor,
    pub snapshot: Snapshot,
    pub context: ReliabilityScope,
    pub organization_name: String,
    pub administrator: bool,
    pub member_until: Option<DateTime<Utc>>,
}
pub(super) fn record_denied() -> AuthError {
    AuthError::new(
        AuthErrorKind::Forbidden,
        "record_unavailable",
        "Este registro não está disponível para seu acesso. Confira a empresa e solicite permissão para esta obra ou projeto.",
    )
}
pub(super) fn record_conflict() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "version_conflict",
        "Este registro mudou. Seu preenchimento foi mantido; confira a versão atual antes de salvar novamente.",
    )
}
pub(super) fn repeated_conflict() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "idempotency_conflict",
        "Esta tentativa já foi usada com outro conteúdo. Confira o registro salvo antes de iniciar outra alteração.",
    )
}
pub(super) fn operation_id(value: &str) -> Result<Uuid, AuthError> {
    let key = id(value)?;
    if key.get_version() != Some(uuid::Version::Random) {
        return Err(invalid(
            "Atualize a página para obter uma nova identificação da tentativa.",
        ));
    }
    Ok(key)
}
pub(super) fn reason(value: &str) -> Result<&str, AuthError> {
    let value = value.trim();
    if !(2..=500).contains(&value.chars().count()) || value.chars().any(char::is_control) {
        Err(invalid(
            "Informe um motivo entre 2 e 500 caracteres, sem quebras de linha.",
        ))
    } else {
        Ok(value)
    }
}
fn project_error(e: ProjectError) -> AuthError {
    invalid(match e {
        ProjectError::InvalidText => {
            "Informe título, disciplina, objetivo e escopo nos limites indicados."
        }
        ProjectError::InvalidList => {
            "Use até 100 entregas e 50 partes interessadas, sem repetir identificações."
        }
        ProjectError::IncompleteCommitment => {
            "Cada compromisso precisa de responsável, prazo e critério de aceite antes da aprovação."
        }
        ProjectError::InvalidState => {
            "A ação não está disponível neste estado. Confira o escopo aprovado e a versão atual."
        }
        ProjectError::InvalidDelivery => "A entrega não pertence a esta versão ou já foi aceita.",
        ProjectError::PendingDelivery => {
            "Todas as entregas precisam ser aceitas antes do encerramento."
        }
    })
}
impl RecordAccess {
    pub fn permits(&self, scope: Uuid, action: AccessAction, now: DateTime<Utc>) -> bool {
        self.member_until.is_none_or(|end| end > now)
            && self
                .snapshot
                .permits(self.actor.id, scope, action, zero(), now)
    }
    pub fn require(
        &self,
        scope: Uuid,
        action: AccessAction,
        now: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        if !self.permits(scope, AccessAction::View, now) || !self.permits(scope, action, now) {
            Err(record_denied())
        } else {
            Ok(())
        }
    }
}
impl AuthService {
    pub(super) async fn record_access(
        &self,
        token: &str,
        context: Option<&ReliabilityScope>,
    ) -> Result<(AuthTransaction, Option<RecordAccess>), AuthError> {
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let selected = tx.org_selection(actor.session).await?;
        let Some(tenant) = selected.tenant_id else {
            if context.is_some() {
                return Err(context_changed());
            }
            return Ok((tx, None));
        };
        if let Some(context) = context
            && (id(&context.organization_id)? != tenant
                || context.context_version != selected.generation)
        {
            return Err(context_changed());
        }
        let (org, member) = authorize(&mut tx, &actor, tenant).await?;
        let snapshot = Snapshot::load_permissions(&mut tx, tenant).await?;
        self.locked(&mut tx, token).await?;
        let now = tx.now().await?;
        if member.valid_until.is_some_and(|until| until <= now) {
            return Err(record_denied());
        }
        Ok((
            tx,
            Some(RecordAccess {
                tenant,
                actor,
                snapshot,
                context: ReliabilityScope {
                    organization_id: tenant.to_string(),
                    context_version: selected.generation,
                },
                organization_name: org.name,
                administrator: member.role == "administrator",
                member_until: member.valid_until,
            }),
        ))
    }
    pub(super) async fn record_revalidate(
        &self,
        tx: &mut AuthTransaction,
        token: &str,
        access: &RecordAccess,
        scope: Uuid,
        action: AccessAction,
    ) -> Result<(), AuthError> {
        self.locked(tx, token).await?;
        access.require(scope, action, tx.now().await?)
    }
    pub async fn projects_context(&self, token: &str) -> Result<ProjectsContext, AuthError> {
        self.limit("projects-context", token, 120, 60).await?;
        let (mut tx, access) = self.record_access(token, None).await?;
        let mut out = ProjectsContext {
            csrf: self.csrf(token),
            context: None,
            organization_name: String::new(),
            can_create: false,
            projects: vec![],
            professionals: vec![],
            parties: vec![],
            works: vec![],
        };
        if let Some(a) = access {
            let now = tx.now().await?;
            let scopes: Vec<_> = a
                .snapshot
                .scopes
                .iter()
                .filter(|s| s.kind == "project" && a.permits(s.id, AccessAction::View, now))
                .map(|s| s.id)
                .collect();
            let rows = tx.project_list(a.tenant, &scopes).await?;
            out.projects = rows
                .into_iter()
                .map(|row| project_view(row, &a, now))
                .collect::<Result<_, _>>()?;
            out.can_create = a.administrator && a.actor.mfa;
            if out.can_create || !out.projects.is_empty() {
                out.professionals = choices(tx.project_choices(a.tenant, true).await?);
                out.parties = choices(tx.project_choices(a.tenant, false).await?);
            }
            for s in a
                .snapshot
                .scopes
                .iter()
                .filter(|s| s.kind == "work" && a.permits(s.id, AccessAction::View, now))
            {
                if tx.project_work_exists(a.tenant, s.id).await? {
                    out.works.push(RecordChoice {
                        id: s.id.to_string(),
                        name: s.name.clone(),
                    });
                }
            }
            // A validade pode terminar durante uma consulta. Não publicar dados depois dela.
            self.locked(&mut tx, token).await?;
            let final_now = tx.now().await?;
            if a.member_until.is_some_and(|until| until <= final_now)
                || scopes
                    .iter()
                    .any(|scope| !a.permits(*scope, AccessAction::View, final_now))
                || out.works.iter().any(|work| {
                    id(&work.id).is_ok_and(|scope| !a.permits(scope, AccessAction::View, final_now))
                })
            {
                return Err(record_denied());
            }
            out.context = Some(a.context);
            out.organization_name = a.organization_name;
        }
        tx.commit().await?;
        Ok(out)
    }
    pub async fn projects_history(
        &self,
        token: &str,
        context: &ReliabilityScope,
        project: &str,
    ) -> Result<Vec<ProjectRecord>, AuthError> {
        let (mut tx, a) = self.record_access(token, Some(context)).await?;
        let a = a.ok_or_else(record_denied)?;
        let project = id(project)?;
        a.require(project, AccessAction::View, tx.now().await?)?;
        let rows = tx.project_history(a.tenant, project).await?;
        self.record_revalidate(&mut tx, token, &a, project, AccessAction::View)
            .await?;
        let now = tx.now().await?;
        let result = rows
            .into_iter()
            .map(|r| project_view(r, &a, now))
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn projects_execute(
        &self,
        token: &str,
        input: ProjectCommand,
    ) -> Result<ProjectReply, AuthError> {
        self.limit("projects-command", token, 120, 60).await?;
        let key = operation_id(&input.operation_key)?;
        let reason = reason(&input.reason)?;
        let fingerprint = crypto::hash(
            "project-command",
            &serde_json::to_string(&input).map_err(|_| AuthError::unavailable())?,
        );
        let (mut tx, a) = self.record_access(token, Some(&input.context)).await?;
        let mut a = a.ok_or_else(record_denied)?;
        let project = input.project_id.as_deref().map(id).transpose()?;
        let action = match &input.change {
            ProjectChange::Approve => AccessAction::ApproveProjectScope,
            ProjectChange::Accept { .. } => AccessAction::TechnicalAccept,
            _ => AccessAction::EditProject,
        };
        if let Some(project) = project {
            a.require(project, action, tx.now().await?)?;
        } else {
            require_mfa(&a.actor)?;
            if !a.administrator
                || input.expected_version.is_some()
                || !matches!(input.change, ProjectChange::Save { .. })
            {
                return Err(record_denied());
            }
        }
        if let Some(row) = tx.project_operation(a.tenant, key).await? {
            if row.actor_id != a.actor.id
                || row.fingerprint != fingerprint
                || project.is_some_and(|p| p != row.project_id)
            {
                return Err(repeated_conflict());
            }
            self.record_revalidate(&mut tx, token, &a, row.project_id, action)
                .await?;
            let record = project_view(row, &a, tx.now().await?)?;
            tx.commit().await?;
            return Ok(ProjectReply {
                project: record,
                repeated: true,
            });
        }
        let current = if let Some(project) = project {
            Some(
                tx.project_current(a.tenant, project)
                    .await?
                    .ok_or_else(record_denied)?,
            )
        } else {
            None
        };
        if current.as_ref().map(|r| r.version) != input.expected_version {
            return Err(record_conflict());
        }
        if matches!(input.change, ProjectChange::Approve)
            && current.as_ref().is_some_and(|r| r.actor_id == a.actor.id)
        {
            return Err(AuthError::new(
                AuthErrorKind::Forbidden,
                "self_approval",
                "A aprovação do escopo exige outra pessoa com a permissão de aprovar, diferente de quem preparou esta revisão.",
            ));
        }
        let assigning = matches!(
            input.change,
            ProjectChange::Save { .. }
                | ProjectChange::SetDelivery { .. }
                | ProjectChange::RemoveDelivery { .. }
                | ProjectChange::Approve
                | ProjectChange::Start
        );
        let old = current
            .as_ref()
            .map(|r| {
                serde_json::from_str::<ProjectSnapshot>(&r.payload)
                    .map_err(|_| AuthError::unavailable())
            })
            .transpose()?;
        let next = match input.change {
            ProjectChange::Save { spec } => match &old {
                Some(old) => old.revise(spec),
                None => ProjectSnapshot::draft(spec),
            },
            ProjectChange::SetDelivery { delivery } => {
                let old = old.as_ref().ok_or_else(record_denied)?;
                let mut spec = old.spec.clone();
                if let Some(found) = spec.deliveries.iter_mut().find(|d| d.id == delivery.id) {
                    *found = delivery;
                } else {
                    spec.deliveries.push(delivery);
                }
                old.revise(spec)
            }
            ProjectChange::RemoveDelivery { delivery_id } => {
                let old = old.as_ref().ok_or_else(record_denied)?;
                let delivery = delivery_id
                    .parse::<EntityId>()
                    .map_err(|_| invalid("Confira a identificação da entrega."))?;
                let mut spec = old.spec.clone();
                if !spec.deliveries.iter().any(|d| d.id == delivery) {
                    return Err(project_error(ProjectError::InvalidDelivery));
                }
                spec.deliveries.retain(|d| d.id != delivery);
                old.revise(spec)
            }
            ProjectChange::Approve => old.as_ref().ok_or_else(record_denied)?.approve(),
            ProjectChange::Start => old.as_ref().ok_or_else(record_denied)?.start(),
            ProjectChange::Accept { delivery_id } => {
                old.as_ref().ok_or_else(record_denied)?.accept(
                    delivery_id
                        .parse()
                        .map_err(|_| invalid("Confira a identificação da entrega."))?,
                )
            }
            ProjectChange::Close => old.as_ref().ok_or_else(record_denied)?.close(),
        }
        .map_err(project_error)?;
        let all_parties: BTreeSet<_> = next
            .spec
            .stakeholder_ids
            .iter()
            .copied()
            .chain(next.spec.deliveries.iter().filter_map(|d| d.responsible_id))
            .collect();
        let approved = next.status != ProjectStatus::Draft;
        for party in &all_parties {
            let p = tx
                .project_party(a.tenant, party.as_uuid())
                .await?
                .ok_or_else(|| {
                    invalid("Uma das pessoas não pertence a esta empresa. Confira os cadastros.")
                })?;
            if assigning && !p.active {
                return Err(invalid(
                    "Uma das pessoas está inativa. Selecione um cadastro vigente.",
                ));
            }
            for delivery in next
                .spec
                .deliveries
                .iter()
                .filter(|d| d.responsible_id == Some(*party))
            {
                if !p.professional {
                    return Err(invalid(
                        "O responsável precisa estar cadastrado como profissional, sem exigir conta de acesso.",
                    ));
                }
                if assigning
                    && approved
                    && delivery
                        .due_on
                        .is_none_or(|d| !p.effective_on(d.as_naive_date()))
                {
                    return Err(invalid(
                        "A atuação do profissional não cobre o compromisso. Registre um novo vínculo vigente ou escolha outro responsável.",
                    ));
                }
            }
        }
        if let Some(work) = next.spec.work_id {
            a.require(work.as_uuid(), AccessAction::View, tx.now().await?)?;
            if !tx.project_work_exists(a.tenant, work.as_uuid()).await? {
                return Err(invalid("Selecione uma obra da carteira desta empresa."));
            }
        }
        let new_id = project.unwrap_or_else(Uuid::new_v4);
        if project.is_none() {
            tx.access_create_scope(
                a.tenant,
                new_id,
                "project",
                &next.spec.title,
                None,
                a.actor.id,
            )
            .await?;
            let now = tx.now().await?;
            let grant = StoredGrant {
                id: Uuid::new_v4(),
                scope_id: new_id,
                user_id: a.actor.id,
                email: a.actor.email.clone(),
                role: "manager".into(),
                actions: vec![
                    "view".into(),
                    "edit_project".into(),
                    "manage_technical_obligations".into(),
                ],
                max_amount: "0".into(),
                valid_from: now,
                valid_until: a.member_until,
                revoked_at: None,
                source_grant_id: None,
                substitution: false,
                reason: "Criação do projeto".into(),
                version: 1,
                member_effective: true,
                member_valid_until: a.member_until,
                required: false,
            };
            tx.access_insert_grant(a.tenant, &grant, a.actor.id).await?;
            a.snapshot = Snapshot::load_permissions(&mut tx, a.tenant).await?;
        }
        let payload = serde_json::to_string(&next).map_err(|_| AuthError::unavailable())?;
        let party_ids: Vec<_> = all_parties.into_iter().map(|p| p.as_uuid()).collect();
        let row = tx
            .project_write(&ProjectWrite {
                tenant: a.tenant,
                id: new_id,
                expected: input.expected_version,
                payload: &payload,
                actor: a.actor.id,
                operation_key: key,
                fingerprint: &fingerprint,
                reason,
                party_ids: &party_ids,
                work_id: next.spec.work_id.map(|w| w.as_uuid()),
            })
            .await?
            .ok_or_else(record_conflict)?;
        tx.org_audit(
            a.tenant,
            a.actor.id,
            "project_revision",
            Some(new_id),
            &format!(
                "version:{};state:{:?};reason:{}",
                row.version, next.status, reason
            ),
        )
        .await?;
        self.record_revalidate(&mut tx, token, &a, new_id, action)
            .await?;
        let record = project_view(row, &a, tx.now().await?)?;
        tx.commit().await?;
        Ok(ProjectReply {
            project: record,
            repeated: false,
        })
    }
}
pub(super) fn choices(rows: Vec<(Uuid, String)>) -> Vec<RecordChoice> {
    rows.into_iter()
        .map(|(id, name)| RecordChoice {
            id: id.to_string(),
            name,
        })
        .collect()
}
fn project_view(
    row: StoredProjectVersion,
    a: &RecordAccess,
    now: DateTime<Utc>,
) -> Result<ProjectRecord, AuthError> {
    Ok(ProjectRecord {
        id: row.project_id.to_string(),
        version: row.version,
        snapshot: serde_json::from_str(&row.payload).map_err(|_| AuthError::unavailable())?,
        recorded_at: row.recorded_at.to_rfc3339(),
        actor_id: row.actor_id.to_string(),
        reason: row.reason,
        can_edit: a.permits(row.project_id, AccessAction::EditProject, now),
        can_approve: a.permits(row.project_id, AccessAction::ApproveProjectScope, now),
        can_accept: a.permits(row.project_id, AccessAction::TechnicalAccept, now),
    })
}
