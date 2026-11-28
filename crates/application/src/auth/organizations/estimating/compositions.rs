use super::*;
use canteiro_domain::compositions::{CompositionError, CostMemory};

async fn calculating_book(
    tx: &mut AuthTransaction,
    tenant: Uuid,
    scope: Uuid,
    candidate: &Composition,
) -> Result<(CompositionBook, CatalogLibrary, Vec<(String, u32)>), AuthError> {
    let mut book = CompositionBook::default();
    let rows = tx.composition_graph(tenant, scope).await?;
    for old in rows {
        if old.code != candidate.code {
            let composition: Composition =
                serde_json::from_str(&old.payload).map_err(|_| AuthError::unavailable())?;
            book.insert(composition)
                .map_err(|_| AuthError::unavailable())?;
        }
    }
    book.insert(candidate.clone())
        .map_err(|_| invalid("Confira as linhas e incidências da composição."))?;
    let sources = book
        .catalog_dependencies(&candidate.code)
        .map_err(|error| match error {
            CompositionError::Cycle(path) => invalid(format!("Composição circular: {path}.")),
            _ => invalid("Composição vinculada inexistente ou incompatível."),
        })?;
    let mut catalogs = CatalogLibrary::default();
    for (catalog, edition) in &sources {
        let loaded = super::catalogs::load_catalog(tx, tenant, scope, catalog, *edition).await?;
        catalogs
            .publish(loaded)
            .map_err(|_| AuthError::unavailable())?;
    }
    // Uma composição de custo informado também precisa apontar para
    // itens reais de edições históricas publicadas. O preço informado não
    // autoriza referências de catálogo inexistentes ou unidades implícitas.
    for line in &candidate.lines {
        if let ComponentSource::Catalog {
            catalog_id,
            edition,
            code,
        } = &line.source
        {
            let edition = catalogs
                .edition(catalog_id, *edition)
                .map_err(|_| invalid("A edição histórica da linha não está disponível."))?;
            let item = edition
                .get(code)
                .ok_or_else(|| invalid("O código do insumo não existe na edição informada."))?;
            canteiro_domain::catalogs::require_matching_units(
                item.unit,
                line.unit,
                line.conversion.as_ref(),
                item.price,
            )
            .map_err(|_| invalid("Confira a unidade da linha e a conversão documentada."))?;
        }
    }
    // Uma mudança de unidade na composição revisada não pode romper
    // referências que outros serviços já possuem para esta composição.
    book.validate_dependents_of(&candidate.code).map_err(|_| {
        invalid("A unidade informada é incompatível com composições vinculadas existentes.")
    })?;
    Ok((book, catalogs, sources.into_iter().collect()))
}
impl AuthService {
    /// Recupera a revisão pelo código, sem depender da lista limitada da página.
    pub async fn estimating_composition_for_edit(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        code: &str,
    ) -> Result<CompositionRecord, AuthError> {
        self.limit("estimating-composition-lookup", token, 60, 60)
            .await?;
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        let row = tx
            .composition_lookup(a.tenant, scope, code)
            .await?
            .ok_or_else(|| invalid("Composição não encontrada no orçamento."))?;
        let record = composition_view(row)?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        tx.commit().await?;
        Ok(record)
    }

    /// A revisão exibida e o grafo de cálculo são lidos sob o mesmo bloqueio.
    pub async fn estimating_composition_memory(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        code: &str,
    ) -> Result<(CompositionRecord, CostMemory), AuthError> {
        self.limit("estimating-calculate", token, 30, 60).await?;
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        tx.composition_lock_scope(a.tenant, scope).await?;
        let row = tx
            .composition_lookup(a.tenant, scope, code)
            .await?
            .ok_or_else(|| invalid("Composição não encontrada no orçamento."))?;
        let record = composition_view(row)?;
        let (book, catalogs, _) =
            match calculating_book(&mut tx, a.tenant, scope, &record.composition).await {
                Ok(result) => result,
                Err(error) => {
                    self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
                        .await?;
                    return Err(error);
                }
            };
        let memory = book.calculate(&record.code, &catalogs).map_err(|_| {
            invalid("Composição circular, fonte ausente, unidade incompatível ou cálculo inválido.")
        })?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        tx.commit().await?;
        Ok((record, memory))
    }

    pub async fn estimating_composition_calculate(
        &self,
        token: &str,
        context: &ReliabilityScope,
        scope_text: &str,
        candidate: Composition,
    ) -> Result<CostMemory, AuthError> {
        self.limit("estimating-calculate", token, 30, 60).await?;
        candidate
            .validate()
            .map_err(|_| invalid("Confira os dados e as bases de incidência."))?;
        let (mut tx, access) = self.record_access(token, Some(context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(scope_text)?;
        require_budget(&a, scope, AccessAction::View, tx.now().await?)?;
        // A prévia usa o mesmo grafo estável das revisões e da memória salva.
        tx.composition_lock_scope(a.tenant, scope).await?;
        let (book, catalogs, _) = match calculating_book(&mut tx, a.tenant, scope, &candidate).await
        {
            Ok(result) => result,
            Err(error) => {
                self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
                    .await?;
                return Err(error);
            }
        };
        let memory = book.calculate(&candidate.code, &catalogs).map_err(|_| {
            invalid("Composição circular, fonte ausente, unidade incompatível ou cálculo inválido.")
        })?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::View)
            .await?;
        tx.commit().await?;
        Ok(memory)
    }
    pub async fn estimating_composition_write(
        &self,
        token: &str,
        input: CompositionWriteCommand,
    ) -> Result<CompositionRecord, AuthError> {
        self.limit("estimating-composition-write", token, 60, 60)
            .await?;
        let key = operation_id(&input.operation_key)?;
        let reason = reason(&input.reason)?;
        input
            .composition
            .validate()
            .map_err(|_| invalid("Confira a identificação e as bases da composição."))?;
        let fingerprint = crypto::hash(
            "composition-command",
            &serde_json::to_string(&input).map_err(|_| AuthError::unavailable())?,
        );
        let (mut tx, access) = self.record_access(token, Some(&input.context)).await?;
        let a = access.ok_or_else(record_denied)?;
        let scope = id(&input.scope_id)?;
        require_budget(&a, scope, AccessAction::EditBudget, tx.now().await?)?;
        // Adquirir antes da leitura da versão e do grafo evita que duas
        // revisões de códigos distintos criem um ciclo por write skew.
        tx.composition_lock_scope(a.tenant, scope).await?;
        if let Some(saved) = tx.composition_operation(a.tenant, key).await? {
            if saved.actor_id != a.actor.id
                || saved.scope_id != scope
                || saved.code != input.composition.code
                || saved.fingerprint != fingerprint
            {
                return Err(repeated_conflict());
            }
            self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
                .await?;
            let record = composition_view(saved)?;
            tx.commit().await?;
            return Ok(record);
        }
        let current = tx
            .composition_current(a.tenant, scope, &input.composition.code)
            .await?;
        if current.as_ref().map(|r| r.version) != input.expected_version {
            return Err(record_conflict());
        }
        let (book, catalogs, bindings) =
            match calculating_book(&mut tx, a.tenant, scope, &input.composition).await {
                Ok(result) => result,
                Err(error) => {
                    self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
                        .await?;
                    return Err(error);
                }
            };
        book.calculate(&input.composition.code, &catalogs)
            .map_err(|_| {
                invalid("Revise o ciclo da composição, as unidades e a base de cada incidência.")
            })?;
        let payload =
            serde_json::to_string(&input.composition).map_err(|_| AuthError::unavailable())?;
        let saved = tx
            .composition_write(&CompositionWrite {
                tenant: a.tenant,
                scope,
                code: &input.composition.code,
                expected: input.expected_version,
                actor: a.actor.id,
                operation_key: key,
                fingerprint: &fingerprint,
                reason,
                payload: &payload,
                bindings: &bindings,
            })
            .await?
            .ok_or_else(record_conflict)?;
        tx.org_audit(
            a.tenant,
            a.actor.id,
            "composition_revision",
            Some(scope),
            &format!("code:{};version:{}", input.composition.code, saved.version),
        )
        .await?;
        self.record_revalidate(&mut tx, token, &a, scope, AccessAction::EditBudget)
            .await?;
        let view = composition_view(saved)?;
        tx.commit().await?;
        Ok(view)
    }
}
