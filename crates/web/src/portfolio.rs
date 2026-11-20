use crate::parties::{RegistryField, RegistryLifetime, attempt_key, request, scope_query, url};
use canteiro_contracts::{
    forms::{DecimalKind, format_decimal_pt_br, parse_decimal_pt_br},
    portfolio::*,
};
use canteiro_domain::decimal::ExactDecimal;
use leptos::{prelude::*, task::spawn_local};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
struct WorkDraft {
    input: WorkInput,
    latitude: String,
    longitude: String,
    dimension: String,
    localities: String,
}
fn fresh() -> WorkDraft {
    WorkDraft {
        input: WorkInput {
            title: String::new(),
            development: String::new(),
            asset_name: String::new(),
            ownership: AssetOwnership::Owned,
            cost_center: String::new(),
            external_reference: String::new(),
            status: WorkStatus::Planned,
            address: String::new(),
            latitude: None,
            longitude: None,
            section: String::new(),
            dimension: ExactDecimal::ZERO,
            unit: "un".into(),
            localities: vec![],
            contract_ids: vec![],
        },
        latitude: String::new(),
        longitude: String::new(),
        dimension: String::new(),
        localities: String::new(),
    }
}
fn from_input(input: WorkInput) -> WorkDraft {
    WorkDraft {
        latitude: input
            .latitude
            .map(|v| v.to_string().replace('.', ","))
            .unwrap_or_default(),
        longitude: input
            .longitude
            .map(|v| v.to_string().replace('.', ","))
            .unwrap_or_default(),
        dimension: format_decimal_pt_br(&input.dimension.to_string()),
        localities: input
            .localities
            .iter()
            .map(|l| {
                format!(
                    "{};{};{};{}",
                    l.municipality,
                    l.uf,
                    format_decimal_pt_br(&l.percentage.to_string()),
                    format_decimal_pt_br(&l.dimension.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
        input,
    }
}
fn decimal(value: &str, kind: DecimalKind) -> Result<ExactDecimal, String> {
    parse_decimal_pt_br(value, kind)
        .map_err(|e| e.message)?
        .canonical
        .parse()
        .map_err(|_| "Confira o decimal informado.".into())
}
fn coordinate(value: &str) -> Result<Option<ExactDecimal>, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.contains('.') && value.contains(',') {
        return Err("Nas coordenadas, use um único separador decimal, sem milhar.".into());
    }
    value
        .replace(',', ".")
        .parse()
        .map(Some)
        .map_err(|_| "Confira a coordenada. Use graus decimais, por exemplo -27,5954.".into())
}
fn input_from(mut draft: WorkDraft) -> Result<WorkInput, String> {
    draft.input.latitude = coordinate(&draft.latitude)?;
    draft.input.longitude = coordinate(&draft.longitude)?;
    draft.input.dimension = decimal(&draft.dimension, DecimalKind::Quantity)?;
    draft.input.localities = vec![];
    for (index, line) in draft.localities.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let cells: Vec<_> = line.split(';').map(str::trim).collect();
        if cells.len() != 4 {
            return Err(format!(
                "Rateio, linha {}: use município;UF;percentual;dimensão.",
                index + 1
            ));
        }
        draft.input.localities.push(LocalityAllocation {
            municipality: cells[0].into(),
            uf: cells[1].into(),
            percentage: decimal(cells[2], DecimalKind::Percent)?,
            dimension: decimal(cells[3], DecimalKind::Quantity)?,
        });
    }
    draft.input.validated().map_err(|e| e.to_string())
}
fn status_value(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::Planned => "planned",
        WorkStatus::Active => "active",
        WorkStatus::Paused => "paused",
        WorkStatus::Completed => "completed",
    }
}
fn parse_status(value: &str) -> Option<WorkStatus> {
    match value {
        "planned" => Some(WorkStatus::Planned),
        "active" => Some(WorkStatus::Active),
        "paused" => Some(WorkStatus::Paused),
        "completed" => Some(WorkStatus::Completed),
        _ => None,
    }
}
async fn refresh(
    context: RwSignal<Option<PortfolioContext>>,
    message: RwSignal<String>,
    busy: RwSignal<bool>,
    query: String,
    status: String,
    alive: Arc<AtomicBool>,
) {
    if !alive.load(Ordering::Acquire) {
        return;
    }
    busy.set(true);
    let path = format!(
        "/api/portfolio/context?query={}{}",
        url(&query),
        if status.is_empty() {
            String::new()
        } else {
            format!("&status={}", url(&status))
        }
    );
    let result = request(&path, None, None).await;
    if !alive.load(Ordering::Acquire) {
        return;
    }
    match result {
        Ok(v) => context.set(Some(v)),
        Err(e) => {
            context.set(None);
            message.set(e);
        }
    }
    busy.set(false);
}
fn map_xy(work: &WorkView) -> Option<(String, String)> {
    Some((
        work.input
            .longitude?
            .checked_add("180".parse().ok()?)
            .ok()?
            .to_string(),
        "90".parse::<ExactDecimal>()
            .ok()?
            .checked_sub(work.input.latitude?)
            .ok()?
            .to_string(),
    ))
}

#[component]
pub fn PortfolioPage() -> impl IntoView {
    let lifetime = RegistryLifetime::new();
    let context = RwSignal::new(None::<PortfolioContext>);
    let draft = RwSignal::new(fresh());
    let work_id = RwSignal::new(String::new());
    let expected = RwSignal::new(None::<i64>);
    let editable = RwSignal::new(true);
    let message = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let filter = RwSignal::new(String::new());
    let reason = RwSignal::new("Cadastro da obra".to_owned());
    let attempt = RwSignal::new(None::<(String, String)>);
    let history = RwSignal::new(Vec::<WorkRevision>::new());
    let editor_tenant = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        spawn_local(refresh(
            context,
            message,
            busy,
            String::new(),
            String::new(),
            lifetime.capture(),
        ))
    });
    Effect::new(move |_| {
        if let Some(tenant) = context
            .get()
            .and_then(|c| c.scope.map(|s| s.organization_id))
        {
            if editor_tenant.get_untracked().is_none() {
                editor_tenant.set(Some(tenant));
            } else if editor_tenant.get_untracked().as_ref() != Some(&tenant) {
                message.set("A empresa mudou. O rascunho foi preservado. Selecione uma obra ou clique em Nova obra antes de continuar.".into());
            }
        }
    });
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let Some(ctx) = context.get_untracked() else {
            return;
        };
        let Some(scope) = ctx.scope else {
            return;
        };
        if editor_tenant.get_untracked().as_deref() != Some(scope.organization_id.as_str()) {
            message.set(
                "A empresa mudou. Selecione uma obra ou clique em Nova obra antes de continuar."
                    .into(),
            );
            return;
        }
        let input = match input_from(draft.get_untracked()) {
            Ok(v) => v,
            Err(e) => {
                message.set(e);
                return;
            }
        };
        let mut command = PortfolioCommand {
            context: scope.clone(),
            key: String::new(),
            work_id: work_id.get_untracked(),
            expected_version: expected.get_untracked(),
            input,
            reason: reason.get_untracked(),
        };
        command.key = match attempt_key(attempt, &command.idempotency_payload()) {
            Ok(v) => v,
            Err(e) => {
                message.set(e);
                return;
            }
        };
        busy.set(true);
        let alive = lifetime.capture();
        spawn_local(async move {
            let result = request::<PortfolioReceipt>(
                "/api/portfolio/command",
                serde_json::to_string(&command).ok(),
                Some(&ctx.csrf),
            )
            .await;
            if !alive.load(Ordering::Acquire) {
                return;
            }
            match result {
                Ok(reply) => {
                    message.set(reply.message);
                    expected.set(Some(reply.version));
                    work_id.set(reply.id.clone());
                    attempt.set(None);
                    let row = request::<WorkView>(
                        &format!("/api/portfolio/{}?{}", reply.id, scope_query(&scope)),
                        None,
                        None,
                    )
                    .await;
                    if !alive.load(Ordering::Acquire) {
                        return;
                    }
                    if let Ok(row) = row {
                        draft.set(from_input(row.input));
                        editable.set(row.can_edit);
                    }
                    refresh(
                        context,
                        message,
                        busy,
                        query.get_untracked(),
                        filter.get_untracked(),
                        alive.clone(),
                    )
                    .await;
                }
                Err(e) => message.set(e),
            }
            if alive.load(Ordering::Acquire) {
                busy.set(false);
            }
        });
    };
    view! {
        <section class="registry-page" aria-labelledby="portfolio-title">
            <p class="eyebrow">"Carteira"</p><h1 id="portfolio-title">"Empreendimentos, obras e ativos"</h1><p>"Uma identificação por obra, mesmo com vários contratos. Localização e rateio podem ser preenchidos pelo teclado."</p>
            <p class="registry-context">{move||context.get().map(|c|if c.scope.is_some(){format!("Empresa: {}",c.organization_name)}else{"Selecione uma empresa para consultar a carteira.".into()}).unwrap_or_else(||"Carregando a empresa…".into())}</p>
            <p><a href="/organizacoes">"Escolher empresa"</a>" · "<a href="/permissoes">"Gerir referências e acessos por obra"</a></p>
            <p id="portfolio-message" class="registry-message" role="status" aria-live="polite" tabindex="-1">{move||message.get()}</p><p role="status" hidden=move||!busy.get()>"Conferindo a carteira…"</p>
            <form class="registry-search" on:submit=move|ev|{ev.prevent_default();if busy.get_untracked(){return;}spawn_local(refresh(context,message,busy,query.get_untracked(),filter.get_untracked(),lifetime.capture()));}>
                <RegistryField id="work-search" label="Buscar obra, empreendimento, município ou centro de custo" value=Signal::derive(move||query.get()) on_input=Callback::new(move|v|query.set(v))/>
                <div class="registry-field"><label for="work-filter">"Situação"</label><select id="work-filter" prop:value=move||filter.get() on:change=move|e|filter.set(event_target_value(&e))><option value="">"Todas"</option><option value="planned">"Planejada"</option><option value="active">"Em execução"</option><option value="paused">"Paralisada"</option><option value="completed">"Concluída"</option></select></div><button type="submit" class="primary-button" disabled=move||busy.get()>"Buscar"</button>
            </form>
            <section class="registry-card" aria-labelledby="work-map-title"><h2 id="work-map-title">"Mapa privado de localização"</h2><p>{move||context.get().map(|c|format!("{} obras visíveis nesta busca. {} com coordenadas; {} sem localização informada.",c.total,c.located,c.total.saturating_sub(c.located))).unwrap_or_default()}</p><p>"A grade representa latitude e longitude. Nenhuma coordenada é enviada a um serviço externo. Use a lista para selecionar uma obra."</p>
                <svg class="registry-map" viewBox="0 0 360 180" role="img" aria-labelledby="map-label"><title id="map-label">"Localizações autorizadas da carteira, em graus decimais"</title><rect x="0" y="0" width="360" height="180" fill="#edf4f2" stroke="#64857d"/><line x1="180" x2="180" y1="0" y2="180" stroke="#91aaa3"/><line x1="0" x2="360" y1="90" y2="90" stroke="#91aaa3"/><text x="4" y="12" font-size="8">"90° N"</text><text x="4" y="176" font-size="8">"90° S"</text><text x="310" y="98" font-size="8">"Equador"</text>
                    {move||context.get().map(|c|c.rows.into_iter().filter_map(|row|map_xy(&row).map(|(x,y)|view!{<circle cx=x cy=y r="2.4" fill="#173e36" stroke="white" stroke-width="0.8"><title>{format!("{}: latitude {}, longitude {}",row.input.title,row.input.latitude.unwrap(),row.input.longitude.unwrap())}</title></circle>})).collect_view())}
                </svg>
            </section>
            <div class="registry-layout"><section class="registry-card" aria-labelledby="work-list-title"><h2 id="work-list-title">"Obras da carteira"</h2>
                <ul class="registry-list">{move||context.get().map(|c|c.rows.into_iter().map(|row|{let title=row.input.title.clone();let description=format!("{} · {} {} · {} vínculo(s) de contrato visível(is) · versão {}",row.input.status.label(),format_decimal_pt_br(&row.input.dimension.to_string()),row.input.unit,row.input.contract_ids.len(),row.version);view!{<li><button type="button" class="registry-list-button" disabled=move||busy.get() on:click=move |_|{editor_tenant.set(context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)));work_id.set(row.id.clone());expected.set(Some(row.version));draft.set(from_input(row.input.clone()));editable.set(row.can_edit);reason.set("Atualização da carteira".into());attempt.set(None);history.set(vec![]);message.set(if row.can_edit{"Obra aberta. Confira os dados antes de salvar."}else{"Obra disponível para consulta. Solicite permissão para gerir esta carteira antes de alterar."}.into());}><strong>{title}</strong><span>{description}</span></button></li>}}).collect_view())}</ul>
                <p hidden=move||context.get().is_none_or(|c|c.total!=0)>"Nenhuma obra disponível nesta busca. Ajuste os filtros ou confira seu acesso."</p>
            </section>
            <section class="registry-card" aria-labelledby="work-edit-title"><div class="registry-heading"><h2 id="work-edit-title">{move||if expected.get().is_some(){"Dados da obra"}else{"Incluir obra na carteira"}}</h2><button type="button" disabled=move||busy.get() on:click=move |_|{editor_tenant.set(context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)));draft.set(fresh());work_id.set(String::new());expected.set(None);editable.set(true);reason.set("Cadastro da obra".into());attempt.set(None);history.set(vec![]);}>"Nova obra"</button></div>
                <form on:submit=save><fieldset disabled=move||busy.get()||!editable.get()||context.get().is_none_or(|c|c.scope.is_none())><legend>"Identificação e localização"</legend>
                    <div hidden=move||expected.get().is_some()><label for="work-reference">"Referência de obra com acesso concedido"</label><select id="work-reference" required=true disabled=move||expected.get().is_some() prop:value=move||work_id.get() on:change=move|e|{let id=event_target_value(&e);if let Some(ctx)=context.get_untracked() && let Some(scope)=ctx.available_scopes.iter().find(|s|s.id==id){draft.update(|d|d.input.title=scope.name.clone());}work_id.set(id);}>
                        <option value="">"Selecione a obra"</option>{move||context.get().map(|c|c.available_scopes.into_iter().map(|s|view!{<option value=s.id>{s.name}</option>}).collect_view())}</select><p class="field-help">"A referência e o acesso são definidos em Permissões. Isso evita duplicar uma obra ou conceder acesso a todos os membros da empresa."</p></div>
                    <RegistryField id="work-title" label="Título da obra" required=true value=Signal::derive(move||draft.get().input.title) on_input=Callback::new(move|v|draft.update(|d|d.input.title=v))/>
                    <RegistryField id="work-development" label="Empreendimento" value=Signal::derive(move||draft.get().input.development) on_input=Callback::new(move|v|draft.update(|d|d.input.development=v))/>
                    <RegistryField id="work-asset" label="Bem, ativo ou via" value=Signal::derive(move||draft.get().input.asset_name) on_input=Callback::new(move|v|draft.update(|d|d.input.asset_name=v))/>
                    <label for="work-ownership">"Relação com o ativo"</label><select id="work-ownership" prop:value=move||match draft.get().input.ownership{AssetOwnership::Owned=>"owned",AssetOwnership::Leased=>"leased",AssetOwnership::ThirdParty=>"third_party"} on:change=move|e|{let value=event_target_value(&e);draft.update(|d|d.input.ownership=match value.as_str(){"leased"=>AssetOwnership::Leased,"third_party"=>AssetOwnership::ThirdParty,_=>AssetOwnership::Owned});}><option value="owned">"Próprio"</option><option value="leased">"Locado"</option><option value="third_party">"De terceiro"</option></select>
                    <RegistryField id="work-cost-center" label="Centro de custo" value=Signal::derive(move||draft.get().input.cost_center) on_input=Callback::new(move|v|draft.update(|d|d.input.cost_center=v))/>
                    <RegistryField id="work-external" label="Identificação externa (se houver)" value=Signal::derive(move||draft.get().input.external_reference) on_input=Callback::new(move|v|draft.update(|d|d.input.external_reference=v))/>
                    <label for="work-status">"Situação da obra"</label><select id="work-status" prop:value=move||status_value(draft.get().input.status) on:change=move|e|{if let Some(status)=parse_status(&event_target_value(&e)){draft.update(|d|d.input.status=status);}}><option value="planned">"Planejada"</option><option value="active">"Em execução"</option><option value="paused">"Paralisada"</option><option value="completed">"Concluída"</option></select>
                    <RegistryField id="work-address" label="Endereço" value=Signal::derive(move||draft.get().input.address) on_input=Callback::new(move|v|draft.update(|d|d.input.address=v))/>
                    <details><summary>"Coordenadas e trecho (opcionais)"</summary><p>"Use graus decimais. Se a localização ainda é desconhecida, deixe as duas coordenadas vazias."</p>
                        <RegistryField id="work-latitude" label="Latitude, entre -90 e 90" value=Signal::derive(move||draft.get().latitude) on_input=Callback::new(move|v|draft.update(|d|d.latitude=v))/>
                        <RegistryField id="work-longitude" label="Longitude, entre -180 e 180" value=Signal::derive(move||draft.get().longitude) on_input=Callback::new(move|v|draft.update(|d|d.longitude=v))/>
                        <RegistryField id="work-section" label="Trecho ou segmento" value=Signal::derive(move||draft.get().input.section) on_input=Callback::new(move|v|draft.update(|d|d.input.section=v))/>
                    </details>
                    <h3>"Dimensão e rateio municipal"</h3>
                    <RegistryField id="work-dimension" label="Dimensão total (vírgula decimal)" required=true value=Signal::derive(move||draft.get().dimension) on_input=Callback::new(move|v|draft.update(|d|d.dimension=v))/>
                    <label for="work-unit">"Unidade"</label><select id="work-unit" prop:value=move||draft.get().input.unit on:change=move|e|draft.update(|d|d.input.unit=event_target_value(&e))><option value="un">"unidade"</option><option value="m">"m"</option><option value="m2">"m²"</option><option value="m3">"m³"</option><option value="km">"km"</option><option value="ha">"ha"</option></select>
                    <label for="work-localities">"Municípios, um por linha"</label><textarea id="work-localities" required=true rows="5" prop:value=move||draft.get().localities on:input=move|e|draft.update(|d|d.localities=event_target_value(&e))></textarea><p class="field-help">"Separe por ponto e vírgula: município;UF;percentual;dimensão. Exemplo para uma obra com 10 km: Florianópolis;SC;100;10. Use vírgula nos decimais."</p>
                    <p role="status" aria-live="polite">{move||if draft.get().localities.is_empty(){"O percentual precisa fechar 100% e as parcelas precisam fechar a dimensão total.".into()}else{match input_from(draft.get()){Ok(_)=>"Rateio conferido: 100% e dimensão total preservados.".into(),Err(e)=>e}}}</p>
                    <details><summary>"Contratos vinculados"</summary><p>"A mesma obra pode participar de vários contratos sem aparecer novamente na carteira."</p>
                        {move||context.get().map(|c|c.contracts.into_iter().map(|contract|{let id=contract.id.clone();let event_id=contract.id;view!{<label class="registry-check"><input type="checkbox" prop:checked=move||draft.get().input.contract_ids.contains(&id) on:change=move|e|{let checked=event_target_checked(&e);draft.update(|d|{d.input.contract_ids.retain(|s|s!=&event_id);if checked{d.input.contract_ids.push(event_id.clone());}});}/>{contract.name}</label>}}).collect_view())}
                    </details>
                    <RegistryField id="work-reason" label="Motivo do cadastro ou da alteração" required=true value=Signal::derive(move||reason.get()) on_input=Callback::new(move|v|reason.set(v))/>
                    <button type="submit" class="primary-button" disabled=move||work_id.get().is_empty()>"Salvar obra"</button>
                </fieldset></form>
                <button type="button" disabled=move||busy.get()||expected.get().is_none() on:click=move |_|{let Some(ctx)=context.get_untracked()else{return;};let Some(scope)=ctx.scope else{return;};if editor_tenant.get_untracked().as_deref()!=Some(scope.organization_id.as_str()){message.set("A empresa mudou. Selecione uma obra ou clique em Nova obra antes de continuar.".into());return;}let id=work_id.get_untracked();busy.set(true);let alive=lifetime.capture();spawn_local(async move{
                    let result=request::<Vec<WorkRevision>>(&format!("/api/portfolio/{id}/history?{}",scope_query(&scope)),None,None).await;
                    if !alive.load(Ordering::Acquire){return;}
                    busy.set(false);
                    if work_id.get_untracked()!=id||context.get_untracked().and_then(|c|c.scope.map(|s|s.organization_id)).as_deref()!=Some(scope.organization_id.as_str()){return;}
                    match result{Ok(v)=>history.set(v),Err(e)=>message.set(e)}
                });}>"Consultar histórico da obra"</button>
                <ol class="registry-list">{move||history.get().into_iter().map(|h|view!{<li>{format!("Versão {} · {} · {}. {}. Dimensão salva: {} {}.",h.version,h.recorded_at,h.actor,h.reason,format_decimal_pt_br(&h.after.dimension.to_string()),h.after.unit)}</li>}).collect_view()}</ol>
            </section></div>
        </section>
    }.into_any()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_coordinate_remains_absent() {
        assert_eq!(coordinate("").unwrap(), None);
        assert_eq!(coordinate("-27,59").unwrap().unwrap().to_string(), "-27.59");
        assert!(coordinate("1.234,56").is_err());
    }
    #[test]
    fn map_uses_exact_coordinates() {
        let mut d = fresh();
        d.input.latitude = Some("-27.59".parse().unwrap());
        d.input.longitude = Some("-48.55".parse().unwrap());
        let v = WorkView {
            id: "x".into(),
            version: 1,
            input: d.input,
            can_edit: false,
        };
        assert_eq!(map_xy(&v), Some(("131.45".into(), "117.59".into())));
    }
}
