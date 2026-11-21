use super::*;
use canteiro_domain::compositions::CostMemory;

fn dependencies(composition:&Composition,keys:&mut BTreeSet<(String,u32)>){
    for line in &composition.lines{
        if let ComponentSource::Catalog{catalog_id,edition,..}=&line.source{
            keys.insert((catalog_id.clone(),*edition));
        }
    }
}
async fn calculating_book(
    tx:&mut AuthTransaction,tenant:Uuid,scope:Uuid,candidate:&Composition,
)->Result<(CompositionBook,CatalogLibrary,Vec<(String,u32)>),AuthError>{
    let mut book=CompositionBook::default();
    let mut sources=BTreeSet::new();
    let rows=tx.composition_list(tenant,&[scope]).await?;
    for old in rows{
        if old.code!=candidate.code{
            let composition:Composition=serde_json::from_str(&old.payload)
                .map_err(|_|AuthError::unavailable())?;
            dependencies(&composition,&mut sources);
            book.insert(composition).map_err(|_|AuthError::unavailable())?;
        }
    }
    dependencies(candidate,&mut sources);
    book.insert(candidate.clone())
        .map_err(|_|invalid("Confira as linhas e incidências da composição."))?;
    let mut catalogs=CatalogLibrary::default();
    for (catalog,edition) in &sources{
        let loaded=super::catalogs::load_catalog(tx,tenant,scope,catalog,*edition).await?;
        catalogs.publish(loaded).map_err(|_|AuthError::unavailable())?;
    }
    Ok((book,catalogs,sources.into_iter().collect()))
}
impl AuthService{
    pub async fn estimating_composition_calculate(
        &self,token:&str,context:&ReliabilityScope,scope_text:&str,candidate:Composition,
    )->Result<CostMemory,AuthError>{
        self.limit("estimating-calculate",token,30,60).await?;
        candidate.validate().map_err(|_|invalid("Confira os dados e as bases de incidência."))?;
        let (mut tx,access)=self.record_access(token,Some(context)).await?;
        let a=access.ok_or_else(record_denied)?;
        let scope=id(scope_text)?;
        require_budget(&a,scope,AccessAction::View,tx.now().await?)?;
        let (book,catalogs,_)=calculating_book(&mut tx,a.tenant,scope,&candidate).await?;
        let memory=book.calculate(&candidate.code,&catalogs)
            .map_err(|_|invalid("Composição circular, fonte ausente, unidade incompatível ou cálculo inválido."))?;
        self.record_revalidate(&mut tx,token,&a,scope,AccessAction::View).await?;
        tx.commit().await?;
        Ok(memory)
    }
    pub async fn estimating_composition_write(
        &self,token:&str,input:CompositionWriteCommand,
    )->Result<CompositionRecord,AuthError>{
        self.limit("estimating-composition-write",token,60,60).await?;
        let key=operation_id(&input.operation_key)?;
        let reason=reason(&input.reason)?;
        input.composition.validate()
            .map_err(|_|invalid("Confira a identificação e as bases da composição."))?;
        let fingerprint=crypto::hash("composition-command",
            &serde_json::to_string(&input).map_err(|_|AuthError::unavailable())?);
        let (mut tx,access)=self.record_access(token,Some(&input.context)).await?;
        let a=access.ok_or_else(record_denied)?;
        let scope=id(&input.scope_id)?;
        require_budget(&a,scope,AccessAction::EditBudget,tx.now().await?)?;
        if let Some(saved)=tx.composition_operation(a.tenant,key).await?{
            if saved.actor_id!=a.actor.id || saved.scope_id!=scope
                || saved.code!=input.composition.code || saved.fingerprint!=fingerprint{
                return Err(repeated_conflict());
            }
            self.record_revalidate(&mut tx,token,&a,scope,AccessAction::EditBudget).await?;
            let record=composition_view(saved)?;
            tx.commit().await?;
            return Ok(record);
        }
        let current=tx.composition_current(
            a.tenant,scope,&input.composition.code).await?;
        if current.as_ref().map(|r|r.version)!=input.expected_version{
            return Err(record_conflict());
        }
        let (book,catalogs,bindings)=calculating_book(
            &mut tx,a.tenant,scope,&input.composition).await?;
        book.calculate(&input.composition.code,&catalogs)
            .map_err(|_|invalid("Revise o ciclo da composição, as unidades e a base de cada incidência."))?;
        let payload=serde_json::to_string(&input.composition)
            .map_err(|_|AuthError::unavailable())?;
        let saved=tx.composition_write(&CompositionWrite{
            tenant:a.tenant,scope,code:&input.composition.code,expected:input.expected_version,
            actor:a.actor.id,operation_key:key,fingerprint:&fingerprint,
            reason,payload:&payload,bindings:&bindings,
        }).await?.ok_or_else(record_conflict)?;
        tx.org_audit(a.tenant,a.actor.id,"composition_revision",Some(scope),
            &format!("code:{};version:{}",input.composition.code,saved.version)).await?;
        self.record_revalidate(&mut tx,token,&a,scope,AccessAction::EditBudget).await?;
        let view=composition_view(saved)?;
        tx.commit().await?;
        Ok(view)
    }
}
