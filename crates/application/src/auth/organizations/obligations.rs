use super::{
    projects::{
        RecordAccess, choices, operation_id, reason, record_conflict, record_denied,
        repeated_conflict,
    },
    *,
};
use canteiro_contracts::{access::AccessAction, obligations::*, reliability::ReliabilityScope};
use canteiro_domain::obligations::ObligationError;
use canteiro_persistence::obligations::{ObligationWrite, StoredObligationVersion};

fn rule_error(error: ObligationError) -> AuthError {
    invalid(match error {
        ObligationError::InvalidText => {
            "Informe disciplina, descrição, emissor e identificação do documento nos limites indicados."
        }
        ObligationError::InvalidPeriod => {
            "Confira a vigência e a data da atividade. O documento precisa cobrir o evento informado."
        }
        ObligationError::ProfessionalRequired => {
            "Indique o profissional responsável pela atividade."
        }
        ObligationError::EvidenceRequired => {
            "Vincule uma versão de arquivo aceita. Número provisório ou exemplo não comprova a obrigação."
        }
        ObligationError::InvalidException => {
            "A exceção exige fonte identificável, anexo e fundamento específico. Dispensa de campo no SICOP não é dispensa legal."
        }
        ObligationError::RegistrationTiming => {
            "O registro ultrapassa a data do evento aplicável ou o prazo de 30 dias das demais atividades. Confira as datas ou apresente a hipótese específica para aprovação."
        }
        ObligationError::RrtEventRequired => {
            "Informe o primeiro evento aplicável ao RRT de projeto ou o término da outra atividade, conforme o art. 2º da Resolução CAU 91/2014."
        }
        ObligationError::WrongCouncil => {
            "Informe CREA para ART e CAU para RRT, conforme o documento do conselho."
        }
    })
}
fn view(
    row: StoredObligationVersion,
    a: &RecordAccess,
    now: DateTime<Utc>,
) -> Result<ObligationRecord, AuthError> {
    Ok(ObligationRecord {
        id: row.obligation_id.to_string(),
        scope_id: row.scope_id.to_string(),
        version: row.version,
        spec: serde_json::from_str(&row.payload).map_err(|_| AuthError::unavailable())?,
        confirmed: row.confirmed,
        document_sha256: row.document_sha256,
        professional_name: row.professional_name,
        replaced_by: row.replaced_by.map(|id| id.to_string()),
        actor_id: row.actor_id.to_string(),
        recorded_at: row.recorded_at.to_rfc3339(),
        reason: row.reason,
        can_edit: a.permits(row.scope_id, AccessAction::ManageTechnicalObligations, now),
        can_exception: a.permits(row.scope_id, AccessAction::ApproveTechnicalException, now),
    })
}
impl AuthService {
    pub async fn obligations_context(&self, token: &str) -> Result<ObligationsContext, AuthError> {
        self.limit("obligations-context", token, 120, 60).await?;
        let (mut tx, a) = self.record_access(token, None).await?;
        let now = tx.now().await?;
        let mut result = ObligationsContext {
            csrf: self.csrf(token),
            context: None,
            organization_name: String::new(),
            today: now
                .with_timezone(&chrono_tz::America::Sao_Paulo)
                .date_naive()
                .to_string(),
            scopes: vec![],
            obligations: vec![],
            professionals: vec![],
            documents: vec![],
        };
        if let Some(a) = a {
            result.scopes = a
                .snapshot
                .scopes
                .iter()
                .filter(|s| a.permits(s.id, AccessAction::View, now))
                .map(|s| TechnicalScope {
                    id: s.id.to_string(),
                    name: s.name.clone(),
                    can_edit: a.permits(s.id, AccessAction::ManageTechnicalObligations, now),
                    can_exception: a.permits(s.id, AccessAction::ApproveTechnicalException, now),
                })
                .collect();
            let scopes = result
                .scopes
                .iter()
                .map(|s| id(&s.id))
                .collect::<Result<Vec<_>, _>>()?;
            result.obligations = tx
                .obligation_list(a.tenant, &scopes)
                .await?
                .into_iter()
                .map(|r| view(r, &a, now))
                .collect::<Result<_, _>>()?;
            if !scopes.is_empty() {
                result.professionals = choices(tx.project_choices(a.tenant, true).await?);
            }
            result.documents = tx
                .technical_documents(a.tenant, &scopes)
                .await?
                .into_iter()
                .map(|d| DocumentChoice {
                    document_id: d.document_id.to_string(),
                    version: d.version,
                    scope_id: d.scope_id.to_string(),
                    label: d.label,
                    sha256: d.sha256,
                })
                .collect();
            self.locked(&mut tx, token).await?;
            let final_now = tx.now().await?;
            if a.member_until.is_some_and(|until| until <= final_now)
                || scopes
                    .iter()
                    .any(|scope| !a.permits(*scope, AccessAction::View, final_now))
            {
                return Err(record_denied());
            }
            result.context = Some(a.context);
            result.organization_name = a.organization_name;
        }
        tx.commit().await?;
        Ok(result)
    }
    pub async fn obligations_history(
        &self,
        token: &str,
        context: &ReliabilityScope,
        obligation: &str,
    ) -> Result<Vec<ObligationRecord>, AuthError> {
        let (mut tx, a) = self.record_access(token, Some(context)).await?;
        let a = a.ok_or_else(record_denied)?;
        let id = id(obligation)?;
        let row = tx
            .obligation_current(a.tenant, id)
            .await?
            .ok_or_else(record_denied)?;
        let scope = row.scope_id;
        a.require(scope, AccessAction::View, tx.now().await?)?;
        let rows = tx.obligation_history(a.tenant, id).await?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        let now = tx.now().await?;
        let result = rows
            .into_iter()
            .map(|r| view(r, &a, now))
            .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn obligations_execute(
        &self,
        token: &str,
        input: ObligationCommand,
    ) -> Result<ObligationReply, AuthError> {
        self.limit("obligations-command", token, 120, 60).await?;
        let key = operation_id(&input.operation_key)?;
        let reason = reason(&input.reason)?;
        input.spec.validate(input.confirm).map_err(rule_error)?;
        let scope = id(&input.scope_id)?;
        let record_id = input.obligation_id.as_deref().map(id).transpose()?;
        if record_id.is_some() != input.expected_version.is_some() {
            return Err(record_conflict());
        }
        let fingerprint = crypto::hash(
            "obligation-command",
            &serde_json::to_string(&input).map_err(|_| AuthError::unavailable())?,
        );
        let (mut tx, a) = self.record_access(token, Some(&input.context)).await?;
        let a = a.ok_or_else(record_denied)?;
        let action = if input.confirm && input.spec.exception.is_some() {
            AccessAction::ApproveTechnicalException
        } else {
            AccessAction::ManageTechnicalObligations
        };
        a.require(scope, action, tx.now().await?)?;
        if let Some(saved) = tx.obligation_operation(a.tenant, key).await? {
            if saved.actor_id != a.actor.id
                || saved.scope_id != scope
                || saved.fingerprint != fingerprint
                || record_id.is_some_and(|id| saved.obligation_id != id)
            {
                return Err(repeated_conflict());
            }
            self.record_revalidate(&mut tx, token, &a, scope, action)
                .await?;
            let record = view(saved, &a, tx.now().await?)?;
            tx.commit().await?;
            return Ok(ObligationReply {
                obligation: record,
                repeated: true,
            });
        }
        let old = if let Some(id) = record_id {
            Some(
                tx.obligation_current(a.tenant, id)
                    .await?
                    .ok_or_else(record_denied)?,
            )
        } else {
            None
        };
        if old.as_ref().map(|r| r.version) != input.expected_version {
            return Err(record_conflict());
        }
        if old.as_ref().is_some_and(|r| r.scope_id != scope) {
            return Err(record_denied());
        }
        if old
            .as_ref()
            .is_some_and(|r| r.confirmed || r.replaced_by.is_some())
        {
            return Err(invalid(
                "Este documento já foi conferido ou substituído. Cadastre uma substituição para preservar seu histórico.",
            ));
        }
        if input.confirm && input.spec.exception.is_some() {
            let previous=old.as_ref().ok_or_else(||invalid("Salve a exceção em elaboração para que outra pessoa confira o fundamento e a aprove."))?;
            if previous.actor_id == a.actor.id {
                return Err(AuthError::new(
                    AuthErrorKind::Forbidden,
                    "self_approval",
                    "A exceção precisa ser aprovada por outra pessoa com permissão própria.",
                ));
            }
            let previous_spec: ObligationSpec =
                serde_json::from_str(&previous.payload).map_err(|_| AuthError::unavailable())?;
            if previous_spec != input.spec {
                return Err(invalid(
                    "A aprovação não pode alterar a exceção apresentada. Salve a nova redação para outra pessoa conferir.",
                ));
            }
        }
        let professional = if let Some(person) = input.spec.professional_id {
            let p = tx
                .project_party(a.tenant, person.as_uuid())
                .await?
                .ok_or_else(|| invalid("O profissional não pertence a esta empresa."))?;
            if !p.professional {
                return Err(invalid(
                    "Selecione uma pessoa cadastrada como profissional.",
                ));
            }
            if input.confirm && !p.effective_on(input.spec.activity_on.as_naive_date()) {
                return Err(invalid(
                    "A atuação do profissional não cobre a data da atividade. Registre um novo vínculo vigente antes de confirmar.",
                ));
            }
            if input.confirm && input.spec.exception.is_none() {
                let council = p.council.to_uppercase();
                if (input.spec.kind == ObligationKind::Art && !council.starts_with("CREA"))
                    || (input.spec.kind == ObligationKind::Rrt && !council.starts_with("CAU"))
                {
                    return Err(invalid(
                        "O conselho do profissional não corresponde ao tipo ART ou RRT informado.",
                    ));
                }
            }
            Some(p)
        } else {
            None
        };
        let document = if let (Some(document), Some(version)) =
            (input.spec.document_id, input.spec.document_version)
        {
            let d=tx.document_version(a.tenant,document.as_uuid(),version).await?.ok_or_else(||invalid("O anexo não existe ou ainda não concluiu a verificação de conteúdo. Envie e confira o arquivo primeiro."))?;
            if d.scope_id != scope || !matches!(d.category.as_str(), "technical" | "evidence") {
                return Err(invalid(
                    "O anexo precisa pertencer ao mesmo escopo e à categoria técnica ou evidência.",
                ));
            }
            if input.confirm
                && matches!(input.spec.kind, ObligationKind::Art | ObligationKind::Rrt)
                && !matches!(d.content_type.as_str(), "application/pdf" | "image/png")
            {
                return Err(invalid(
                    "Anexe a via do conselho em PDF ou imagem PNG para conferir ART ou RRT.",
                ));
            }
            Some(d)
        } else {
            None
        };
        if let Some(previous_id) = input.spec.replaces_id {
            let previous = tx
                .obligation_current(a.tenant, previous_id.as_uuid())
                .await?
                .ok_or_else(record_denied)?;
            if previous.scope_id != scope {
                return Err(record_denied());
            }
            let previous_spec: ObligationSpec =
                serde_json::from_str(&previous.payload).map_err(|_| AuthError::unavailable())?;
            if !previous.confirmed
                || previous.replaced_by.is_some()
                || record_id == Some(previous_id.as_uuid())
                || previous_spec.kind != input.spec.kind
                || previous_spec.activity != input.spec.activity
                || !previous_spec
                    .discipline
                    .eq_ignore_ascii_case(&input.spec.discipline)
            {
                return Err(invalid(
                    "Substitua um documento conferido e ainda vigente no cadastro, da mesma natureza, atividade e disciplina.",
                ));
            }
            if input.confirm
                && previous_spec.document_id == input.spec.document_id
                && previous_spec.document_version == input.spec.document_version
            {
                return Err(invalid(
                    "A substituição exige uma nova versão documental. O anexo anterior deve permanecer no histórico.",
                ));
            }
        }
        let payload = serde_json::to_string(&input.spec).map_err(|_| AuthError::unavailable())?;
        let row = tx
            .obligation_write(&ObligationWrite {
                tenant: a.tenant,
                id: record_id.unwrap_or_else(Uuid::new_v4),
                scope,
                expected: input.expected_version,
                payload: &payload,
                confirmed: input.confirm,
                professional_id: input.spec.professional_id.map(|p| p.as_uuid()),
                professional_name: professional.as_ref().map(|p| p.name.as_str()),
                document_id: input.spec.document_id.map(|p| p.as_uuid()),
                document_version: input.spec.document_version,
                sha256: document.as_ref().map(|d| d.sha256.as_str()),
                replaces_id: input.spec.replaces_id.map(|p| p.as_uuid()),
                actor: a.actor.id,
                operation_key: key,
                fingerprint: &fingerprint,
                reason,
            })
            .await?
            .ok_or_else(record_conflict)?;
        tx.org_audit(
            a.tenant,
            a.actor.id,
            "technical_obligation_revision",
            Some(row.obligation_id),
            &format!(
                "version:{};confirmed:{};reason:{}",
                row.version, input.confirm, reason
            ),
        )
        .await?;
        if input.confirm {
            tx.user_notice_resolve_for_target(a.tenant, a.actor.id, row.obligation_id)
                .await?;
        } else {
            tx.user_notice_record(
                a.tenant,
                a.actor.id,
                key,
                "technical_obligation",
                scope,
                row.obligation_id,
                input.spec.valid_until.map(|day| day.as_naive_date()),
                true,
            )
            .await?;
        }
        self.record_revalidate(&mut tx, token, &a, scope, action)
            .await?;
        let result = view(row, &a, tx.now().await?)?;
        tx.commit().await?;
        Ok(ObligationReply {
            obligation: result,
            repeated: false,
        })
    }
}
