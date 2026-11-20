//! A empresa, o vínculo e a permissão bancária são revalidados em toda operação.
use super::*;
use canteiro_contracts::parties::*;
use canteiro_persistence::parties::{PartyWrite, StoredParty};
use serde::{Serialize, de::DeserializeOwned};

pub(super) fn registry_reason(reason: &str) -> Result<&str, AuthError> {
    let reason = reason.trim();
    if !(2..=200).contains(&reason.chars().count()) || reason.chars().any(char::is_control) {
        return Err(invalid(
            "Informe o motivo com 2 a 200 caracteres, sem quebras de linha.",
        ));
    }
    Ok(reason)
}
pub(super) fn registry_key(key: &str) -> Result<Uuid, AuthError> {
    let key = id(key)?;
    if key.get_version() != Some(uuid::Version::Random) {
        return Err(invalid(
            "Atualize a página para obter uma nova identificação da tentativa.",
        ));
    }
    Ok(key)
}
pub(super) fn encode<T: Serialize>(value: &T) -> Result<String, AuthError> {
    serde_json::to_string(value).map_err(|_| AuthError::unavailable())
}
pub(super) fn decode<T: DeserializeOwned>(value: &str) -> Result<T, AuthError> {
    serde_json::from_str(value).map_err(|_| AuthError::unavailable())
}
pub(super) fn party_view(row: StoredParty) -> Result<PartyView, AuthError> {
    Ok(PartyView {
        id: row.id.to_string(),
        version: row.version,
        input: decode(&row.data)?,
    })
}
pub(super) fn registry_conflict() -> AuthError {
    AuthError::new(
        AuthErrorKind::Conflict,
        "idempotency_conflict",
        "Esta identificação já foi usada com outros dados. Confira o resultado anterior antes de enviar outra alteração.",
    )
}

impl AuthService {
    pub(super) async fn registry_access(
        &self,
        token: &str,
        context: &OrganizationScope,
        admin: bool,
    ) -> Result<(AuthTransaction, Actor), AuthError> {
        self.limit("registry", token, 120, 60).await?;
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let (_, member) = scoped(&mut tx, &actor, context, admin).await?;
        self.locked(&mut tx, token).await?;
        let now = tx.now().await?;
        if member.valid_until.is_some_and(|end| end <= now) {
            return Err(denied());
        }
        Ok((tx, actor))
    }
    pub(super) async fn registry_revalidate(
        &self,
        tx: &mut AuthTransaction,
        token: &str,
        actor: &Actor,
        tenant: Uuid,
        admin: bool,
    ) -> Result<DateTime<Utc>, AuthError> {
        let (session, user) = self.locked(tx, token).await?;
        let user = Self::authenticated(&session, &user)?;
        if user.id != actor.id {
            return Err(denied());
        }
        let member = tx
            .org_member(tenant, actor.id)
            .await?
            .filter(|m| m.effective)
            .ok_or_else(denied)?;
        let now = tx.now().await?;
        if member.valid_until.is_some_and(|end| end <= now) {
            return Err(denied());
        }
        if admin
            && (member.role != "administrator"
                || !session.mfa_verified
                || user.mfa_secret.is_none())
        {
            return Err(denied());
        }
        Ok(now)
    }
    pub async fn parties_context(
        &self,
        token: &str,
        query: &str,
        page: u32,
    ) -> Result<PartiesContext, AuthError> {
        self.limit("parties-context", token, 120, 60).await?;
        let query = query.trim();
        if query.chars().count() > 160 || query.chars().any(char::is_control) || page > 10000 {
            return Err(invalid("Confira a busca e a página do cadastro."));
        }
        let (mut tx, actor) = self.organization_transaction(token).await?;
        let selected = tx.org_selection(actor.session).await?;
        let mut response = PartiesContext {
            csrf: self.csrf(token),
            scope: None,
            organization_name: String::new(),
            can_manage: false,
            rows: vec![],
            total: 0,
            query: query.to_owned(),
            page,
        };
        if let Some(tenant) = selected.tenant_id {
            let (org, member) = authorize(&mut tx, &actor, tenant).await?;
            self.locked(&mut tx, token).await?;
            let now = tx.now().await?;
            if member.valid_until.is_some_and(|end| end <= now) {
                return Err(denied());
            }
            response.scope = Some(OrganizationScope {
                organization_id: tenant.to_string(),
                context_version: selected.generation,
                organization_version: org.version,
            });
            response.organization_name = org.name;
            response.can_manage = member.role == "administrator" && actor.mfa;
            response.total = tx.parties_count(tenant, query).await?;
            response.rows = tx
                .parties_page(tenant, query, i64::from(page) * 50, 50)
                .await?
                .into_iter()
                .map(party_view)
                .collect::<Result<_, _>>()?;
            self.registry_revalidate(&mut tx, token, &actor, tenant, false)
                .await?;
        }
        tx.commit().await?;
        Ok(response)
    }
    pub async fn party_detail(
        &self,
        token: &str,
        context: &OrganizationScope,
        party: &str,
    ) -> Result<PartyView, AuthError> {
        let (mut tx, actor) = self.registry_access(token, context, false).await?;
        let tenant = id(&context.organization_id)?;
        let row = tx.party_get(tenant, id(party)?).await?.ok_or_else(denied)?;
        let result = party_view(row)?;
        self.registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        tx.commit().await?;
        Ok(result)
    }
    pub async fn parties_export(
        &self,
        token: &str,
        context: &OrganizationScope,
        query: &str,
    ) -> Result<Vec<PartyView>, AuthError> {
        if query.chars().count() > 160 {
            return Err(invalid("Use uma busca com até 160 caracteres."));
        }
        let (mut tx, actor) = self.registry_access(token, context, false).await?;
        let tenant = id(&context.organization_id)?;
        let count = tx.parties_count(tenant, query).await?;
        if count > 10000 {
            return Err(invalid(
                "Refine a busca para exportar até dez mil cadastros por arquivo.",
            ));
        }
        let mut rows = Vec::new();
        for offset in (0..count).step_by(100) {
            rows.extend(
                tx.parties_page(tenant, query, offset, 100)
                    .await?
                    .into_iter()
                    .map(party_view)
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        // The exported document is validated by the same domain used in the API.
        for row in &rows {
            row.input
                .clone()
                .validated()
                .map_err(|_| AuthError::unavailable())?;
        }
        self.registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn party_bank_history(
        &self,
        token: &str,
        context: &OrganizationScope,
        party: &str,
    ) -> Result<BankHistory, AuthError> {
        let (mut tx, actor) = self.registry_access(token, context, true).await?;
        let tenant = id(&context.organization_id)?;
        let party = id(party)?;
        let row = tx.party_get(tenant, party).await?.ok_or_else(denied)?;
        let versions = tx
            .party_bank_versions(tenant, party)
            .await?
            .into_iter()
            .map(|v| {
                Ok(BankVersion {
                    version: v.version,
                    account: decode(&v.account)?,
                    actor: v.actor,
                    recorded_at: v.recorded_at.to_rfc3339(),
                    reason: v.reason,
                })
            })
            .collect::<Result<_, AuthError>>()?;
        self.registry_revalidate(&mut tx, token, &actor, tenant, true)
            .await?;
        tx.commit().await?;
        Ok(BankHistory {
            party_id: party.to_string(),
            party_version: row.version,
            versions,
        })
    }
    pub async fn party_revisions(
        &self,
        token: &str,
        context: &OrganizationScope,
        party: &str,
    ) -> Result<Vec<PartyRevision>, AuthError> {
        let (mut tx, actor) = self.registry_access(token, context, false).await?;
        let tenant = id(&context.organization_id)?;
        let party = id(party)?;
        tx.party_get(tenant, party).await?.ok_or_else(denied)?;
        let rows = tx
            .party_history(tenant, party)
            .await?
            .into_iter()
            .map(|r| {
                Ok(PartyRevision {
                    version: r.version,
                    before: r.before_data.as_deref().map(decode).transpose()?,
                    after: decode(&r.after_data)?,
                    actor: r.actor,
                    recorded_at: r.recorded_at.to_rfc3339(),
                    reason: r.reason,
                })
            })
            .collect::<Result<_, AuthError>>()?;
        self.registry_revalidate(&mut tx, token, &actor, tenant, false)
            .await?;
        tx.commit().await?;
        Ok(rows)
    }
    pub async fn parties_execute(
        &self,
        token: &str,
        command: PartyCommand,
    ) -> Result<PartyReceipt, AuthError> {
        let (mut tx, actor) = self.registry_access(token, command.context(), true).await?;
        let tenant = id(&command.context().organization_id)?;
        let key = registry_key(command.key())?;
        let act = match &command {
            PartyCommand::Save { .. } => "party_save",
            PartyCommand::Import { .. } => "party_import",
            PartyCommand::SetBank { .. } => "party_bank",
        };
        let fingerprint = crypto::hash(act, &encode(&command.idempotency_payload())?);
        if let Some(saved) = tx.registry_receipt(tenant, act, key).await? {
            if saved.actor_id != actor.id || saved.fingerprint != fingerprint {
                return Err(registry_conflict());
            }
            let reply = decode(&saved.result)?;
            self.registry_revalidate(&mut tx, token, &actor, tenant, true)
                .await?;
            tx.commit().await?;
            return Ok(reply);
        }
        let mut reply = PartyReceipt {
            ids: vec![],
            versions: vec![],
            reused: 0,
            message: String::new(),
        };
        match command {
            PartyCommand::Save {
                id: party,
                expected_version,
                input,
                reason,
                ..
            } => {
                let input = input.validated().map_err(|e| invalid(e.message))?;
                let reason = registry_reason(&reason)?;
                let party = party.as_deref().map(id).transpose()?;
                if party.is_some() != expected_version.is_some() {
                    return Err(stale());
                }
                let (saved, version, reused) = save_party(
                    &mut tx,
                    tenant,
                    actor.id,
                    party,
                    expected_version,
                    &input,
                    reason,
                )
                .await?;
                reply.ids.push(saved.to_string());
                reply.versions.push(version);
                reply.reused = usize::from(reused);
                reply.message=if reused{"Este CPF ou CNPJ já está cadastrado nesta empresa. O cadastro existente foi localizado; revise-o antes de alterar."}else{"Cadastro salvo. A alteração não cria acesso ao sistema."}.into();
            }
            PartyCommand::Import { rows, .. } => {
                if rows.is_empty() || rows.len() > 100 {
                    return Err(invalid("Importe entre um e cem cadastros por vez."));
                }
                let rows = rows
                    .into_iter()
                    .map(|r| r.validated().map_err(|e| invalid(e.message)))
                    .collect::<Result<Vec<_>, _>>()?;
                for input in rows {
                    let (saved, version, reused) = save_party(
                        &mut tx,
                        tenant,
                        actor.id,
                        None,
                        None,
                        &input,
                        "Importação de cadastros",
                    )
                    .await?;
                    reply.ids.push(saved.to_string());
                    reply.versions.push(version);
                    reply.reused += usize::from(reused);
                }
                reply.message = format!(
                    "Importação conferida: {} entradas, {} cadastros existentes reutilizados.",
                    reply.ids.len(),
                    reply.reused
                );
            }
            PartyCommand::SetBank {
                id: party,
                expected_version,
                account,
                reason,
                ..
            } => {
                let party = id(&party)?;
                let account = account.validated().map_err(|e| invalid(e.message))?;
                let reason = registry_reason(&reason)?;
                let current = tx.party_get(tenant, party).await?.ok_or_else(denied)?;
                let input: PartyInput = decode(&current.data)?;
                if !input.roles.contains(&PartyRole::Supplier) || !input.active {
                    return Err(invalid("A conta bancária exige um fornecedor ativo."));
                }
                let (_, version, _) = save_party(
                    &mut tx,
                    tenant,
                    actor.id,
                    Some(party),
                    Some(expected_version),
                    &input,
                    "Atualização dos dados bancários",
                )
                .await?;
                tx.party_record_bank(tenant, party, version, &encode(&account)?, actor.id, reason)
                    .await?;
                tx.org_audit(
                    tenant,
                    actor.id,
                    "party_bank_changed",
                    Some(party),
                    &format!("version:{version};previous_version:{}", current.version),
                )
                .await?;
                reply.ids.push(party.to_string());
                reply.versions.push(version);
                reply.message="Nova versão bancária salva. As versões anteriores permanecem vinculadas aos registros que as utilizaram.".into();
            }
        }
        self.registry_revalidate(&mut tx, token, &actor, tenant, true)
            .await?;
        tx.registry_record(tenant, act, key, actor.id, &fingerprint, &encode(&reply)?)
            .await?;
        tx.commit().await?;
        Ok(reply)
    }
}

async fn save_party(
    tx: &mut AuthTransaction,
    tenant: Uuid,
    actor: Uuid,
    party: Option<Uuid>,
    expected: Option<i64>,
    input: &PartyInput,
    reason: &str,
) -> Result<(Uuid, i64, bool), AuthError> {
    if let Some(tax) = &input.tax_id
        && let Some(existing) = tx.party_by_tax_id(tenant, tax).await?
    {
        if party.is_none() {
            return Ok((existing.id, existing.version, true));
        }
        if party != Some(existing.id) {
            return Err(AuthError::new(
                AuthErrorKind::Conflict,
                "identity_exists",
                "Este CPF ou CNPJ pertence a outro cadastro desta empresa. Abra o cadastro existente para reutilizá-lo.",
            ));
        }
    }
    let current = match party {
        Some(p) => Some(tx.party_get(tenant, p).await?.ok_or_else(denied)?),
        None => None,
    };
    if let Some(row) = &current
        && Some(row.version) != expected
    {
        return Err(stale());
    }
    let version = expected.unwrap_or(0).checked_add(1).ok_or_else(stale)?;
    if expected.is_some_and(|v| v < 1) {
        return Err(stale());
    }
    let party = party.unwrap_or_else(Uuid::new_v4);
    let data = encode(input)?;
    let profile = input.professional.as_ref();
    if !tx
        .party_write(&PartyWrite {
            tenant,
            id: party,
            actor,
            expected,
            kind: input.kind.as_str(),
            tax_id: input.tax_id.as_deref(),
            name: &input.name,
            customer: input.roles.contains(&PartyRole::Customer),
            supplier: input.roles.contains(&PartyRole::Supplier),
            professional: profile.is_some(),
            active: input.active,
            council: profile.map_or("", |p| p.council.as_str()),
            registration: profile.map_or("", |p| p.registration.as_str()),
            uf: profile.map_or("", |p| p.uf.as_str()),
            valid_from: profile
                .and_then(|p| p.valid_from)
                .map(|d| d.as_naive_date()),
            valid_until: profile
                .and_then(|p| p.valid_until)
                .map(|d| d.as_naive_date()),
            data: &data,
        })
        .await?
    {
        return Err(stale());
    }
    tx.party_record_revision(
        tenant,
        party,
        version,
        current.as_ref().map(|r| r.data.as_str()),
        &data,
        actor,
        reason,
    )
    .await?;
    tx.org_audit(
        tenant,
        actor,
        "party_saved",
        Some(party),
        &format!("version:{version};active:{}", input.active),
    )
    .await?;
    Ok((party, version, false))
}
