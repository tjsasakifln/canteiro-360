use crate::projects::{
    area, context_changed, display_date, display_instant, hidden, option_list,
    preserved_context_page, shell, text_field,
};
use canteiro_contracts::obligations::*;
use leptos::prelude::*;
use std::collections::BTreeMap;

fn value(draft: &BTreeMap<String, String>, key: &str, fallback: String) -> String {
    draft.get(key).cloned().unwrap_or(fallback)
}
fn kind_value(kind: ObligationKind) -> &'static str {
    match kind {
        ObligationKind::Art => "art",
        ObligationKind::Rrt => "rrt",
        ObligationKind::Designation => "designation",
        ObligationKind::License => "license",
        ObligationKind::Insurance => "insurance",
        ObligationKind::Guarantee => "guarantee",
        ObligationKind::Cno => "cno",
    }
}
fn framework_value(framework: RrtFramework) -> &'static str {
    match framework {
        RrtFramework::Project => "project",
        RrtFramework::EnvironmentPlanning => "environment_planning",
        RrtFramework::ProjectCoordination => "project_coordination",
        RrtFramework::SafetySystemDesign => "safety_system_design",
        RrtFramework::FireProtectionDesign => "fire_protection_design",
        RrtFramework::OtherActivities => "other_activities",
    }
}
fn event_value(event: RrtEvent) -> &'static str {
    match event {
        RrtEvent::ProjectCompletion => "project_completion",
        RrtEvent::FinalDelivery => "final_delivery",
        RrtEvent::Submission => "submission",
        RrtEvent::Publication => "publication",
        RrtEvent::OtherActivityCompletion => "other_activity_completion",
    }
}
fn activity_value(activity: TechnicalActivity) -> &'static str {
    match activity {
        TechnicalActivity::Budget => "budget",
        TechnicalActivity::Design => "design",
        TechnicalActivity::Execution => "execution",
        TechnicalActivity::Inspection => "inspection",
        TechnicalActivity::Other => "other",
    }
}
pub fn page(
    ctx: ObligationsContext,
    selection: Option<&str>,
    replacement: Option<&str>,
    history: Vec<ObligationRecord>,
    draft: BTreeMap<String, String>,
    message: Option<String>,
    key: String,
) -> String {
    if context_changed(&draft, ctx.context.as_ref()) {
        return preserved_context_page("Obrigações técnicas", &ctx.organization_name, draft);
    }
    let selected = selection
        .and_then(|id| ctx.obligations.iter().find(|o| o.id == id))
        .cloned();
    let replaces = replacement
        .and_then(|id| ctx.obligations.iter().find(|o| o.id == id))
        .cloned();
    let base = selected.clone().or_else(|| replaces.clone());
    let spec = base.as_ref().map(|o| o.spec.clone());
    let list = ctx.obligations.clone();
    let heading = view! {<section class="record-heading"><p class="eyebrow">{ctx.organization_name.clone()}</p><h1>"Responsabilidades e documentos técnicos"</h1><p>"Confira o que falta para cada atividade, quem responde por ela e quando o documento precisa ser substituído."</p><div class="record-actions"><a class="secondary-button" href="/obrigacoes">"Nova obrigação"</a><a href="/documentos">"Enviar anexo"</a><a href="/cadastros">"Cadastrar profissional"</a></div></section>
        {message.clone().map(|text|view!{<div class="record-notice" role="alert" tabindex="-1"><h2>"Confira antes de continuar"</h2><p>{text}</p><p>"O preenchimento enviado foi mantido. Nenhuma confirmação é presumida."</p></div>})}
        <section class="foundation-card"><h2>"Conferência interna"</h2><p>"O registro organiza a documentação e a responsabilidade. Ele não autentica ART ou RRT no conselho. Um cadastro interno também não substitui a inscrição no CNO."</p><p>"ART de orçamento não comprova automaticamente responsabilidade pela execução. Uma dispensa de campo no SICOP não equivale a dispensa legal."</p></section>
        {ctx.context.is_none().then(||view!{<section class="foundation-card"><h2>"Selecione uma empresa"</h2><a href="/organizacoes">"Escolher empresa"</a></section>})}
    };
    let listing = view! {<section class="foundation-card"><h2>"Obrigações disponíveis"</h2>{list.is_empty().then(||view!{<p>"Nenhuma obrigação disponível para suas permissões."</p>})}{list.into_iter().map(|o|{
        let scope=ctx.scopes.iter().find(|s|s.id==o.scope_id).map(|s|s.name.clone()).unwrap_or_default();
        let status=if o.replaced_by.is_some(){"Substituído"}else if o.spec.valid_until.is_some_and(|date|date.to_string()<ctx.today){"Vencido"}else if !o.confirmed{"Pendente de conferência"}else if o.spec.exception.is_some(){"Exceção aprovada internamente"}else{"Documento conferido internamente"};
        let missing=o.spec.validate(true).err().map(|e|e.to_string()).unwrap_or_else(||"Falta a conferência por uma pessoa com permissão para esta atividade.".into());
        view!{<article class="record-summary"><h3><a href=format!("/obrigacoes?obligation={}",o.id)>{format!("{} · {}",o.spec.kind.label(),o.spec.discipline)}</a></h3><p>{scope}" · "{o.spec.activity.label()}</p><p class="record-status">{status}</p><p>{o.spec.description.clone()}</p><p>{format!("Responsável: {}",o.professional_name.clone().unwrap_or_else(||"a definir conforme a obrigação".into()))}</p><p>{format!("Vigência: {} até {}",display_date(o.spec.valid_from),o.spec.valid_until.map(display_date).unwrap_or_else(||"sem término informado".into()))}</p>{(!o.confirmed).then(||view!{<p class="record-small">{missing}" A pessoa com permissão de gerir obrigações pode corrigir o cadastro."</p>})}</article>}
    }).collect_view()}</section>};
    let details = selected.clone().map(|o| detail(&ctx, o, &key, history));
    let allowed = selected
        .as_ref()
        .is_none_or(|o| o.can_edit && !o.confirmed && o.replaced_by.is_none())
        && ctx.scopes.iter().any(|s| s.can_edit);
    let editor=ctx.context.clone().filter(|_|allowed).map(|scope|{
        let field_value=|key:&str,fallback:String|value(&draft,key,fallback);
        let scope_id=field_value("scope_id",base.as_ref().map(|o|o.scope_id.clone()).unwrap_or_default());
        let kind=field_value("kind",spec.as_ref().map(|s|kind_value(s.kind).into()).unwrap_or_else(||"art".into()));
        let activity=field_value("activity",spec.as_ref().map(|s|activity_value(s.activity).into()).unwrap_or_else(||"execution".into()));
        let selected_doc=field_value("document",if replaces.is_some(){String::new()}else{spec.as_ref().and_then(|s|Some(format!("{}:{}",s.document_id?,s.document_version?))).unwrap_or_default()});
        let source=spec.as_ref().and_then(|s|s.exception.clone());
        let rrt_framework=field_value("rrt_framework",spec.as_ref().and_then(|s|s.rrt_timing.as_ref().map(|t|framework_value(t.framework).to_owned())).unwrap_or_default());
        let rrt_event=field_value("rrt_event",spec.as_ref().and_then(|s|s.rrt_timing.as_ref().map(|t|event_value(t.event).to_owned())).unwrap_or_default());
        view!{<section class="foundation-card"><h2>{if replaces.is_some(){"Registrar substituição"}else if selected.is_some(){"Corrigir pendência"}else{"Registrar obrigação"}}</h2>{replaces.as_ref().map(|r|view!{<p>{format!("O registro {} e seu responsável continuarão preservados. Vincule a nova via documental.",r.spec.number)}</p>})}
            <form method="post" action="/obrigacoes/acao" class="record-form">{hidden(&scope,&ctx.csrf,&key)}<input type="hidden" name="action" value="save"/><input type="hidden" name="obligation_id" value=selected.as_ref().map(|o|o.id.clone()).unwrap_or_default()/><input type="hidden" name="expected_version" value=field_value("expected_version",selected.as_ref().map(|o|o.version.to_string()).unwrap_or_default())/><input type="hidden" name="replaces_id" value=field_value("replaces_id",replaces.as_ref().map(|o|o.id.clone()).or_else(||spec.as_ref().and_then(|s|s.replaces_id.map(|id|id.to_string()))).unwrap_or_default())/>
                <label class="record-field"><span>"Obra, projeto, contrato ou orçamento"</span><select name="scope_id" required><option value="">"Selecione o escopo"</option>{ctx.scopes.iter().filter(|s|s.can_edit).map(|s|view!{<option value=s.id.clone() selected={s.id==scope_id}>{s.name.clone()}</option>}).collect_view()}</select></label>
                <div class="record-layout"><label class="record-field"><span>"Natureza do documento"</span><select name="kind">{[ObligationKind::Art,ObligationKind::Rrt,ObligationKind::Designation,ObligationKind::License,ObligationKind::Insurance,ObligationKind::Guarantee,ObligationKind::Cno].into_iter().map(|k|view!{<option value=kind_value(k) selected={kind_value(k)==kind}>{k.label()}</option>}).collect_view()}</select></label><label class="record-field"><span>"Atividade coberta"</span><select name="activity">{[TechnicalActivity::Budget,TechnicalActivity::Design,TechnicalActivity::Execution,TechnicalActivity::Inspection,TechnicalActivity::Other].into_iter().map(|a|view!{<option value=activity_value(a) selected={activity_value(a)==activity}>{a.label()}</option>}).collect_view()}</select></label></div>
                {text_field("Disciplina","discipline",field_value("discipline",spec.as_ref().map(|s|s.discipline.clone()).unwrap_or_default()),"text",true,80)}
                {area("Escopo da obrigação","description",field_value("description",spec.as_ref().map(|s|s.description.clone()).unwrap_or_default()),true,2000)}
                <label class="record-field"><span>"Profissional responsável (ART, RRT e designação)"</span><select name="professional_id">{option_list(&ctx.professionals,&field_value("professional_id",spec.as_ref().and_then(|s|s.professional_id.map(|id|id.to_string())).unwrap_or_default()),"Selecionar responsável")}</select></label>
                {text_field("Número conforme o documento","number",field_value("number",if replaces.is_some(){String::new()}else{spec.as_ref().map(|s|s.number.clone()).unwrap_or_default()}),"text",false,80)}
                {text_field("Conselho ou entidade emissora","issuer",field_value("issuer",spec.as_ref().map(|s|s.issuer.clone()).unwrap_or_default()),"text",false,160)}
                <div class="record-layout">{text_field("Início da vigência","valid_from",field_value("valid_from",spec.as_ref().map(|s|s.valid_from.to_string()).unwrap_or_else(||ctx.today.clone())),"date",true,10)}{text_field("Fim da vigência (opcional)","valid_until",field_value("valid_until",spec.as_ref().and_then(|s|s.valid_until.map(|d|d.to_string())).unwrap_or_default()),"date",false,10)}</div>
                <div class="record-layout">{text_field("Data da atividade ou compromisso","activity_on",field_value("activity_on",spec.as_ref().map(|s|s.activity_on.to_string()).unwrap_or_else(||ctx.today.clone())),"date",true,10)}{text_field("Data do registro no conselho","registered_on",field_value("registered_on",if replaces.is_some(){String::new()}else{spec.as_ref().and_then(|s|s.registered_on.map(|d|d.to_string())).unwrap_or_default()}),"date",false,10)}</div>
                <details><summary>"Prazo específico do RRT fora da execução"</summary><p>"Para os grupos do art. 2º, II, informe o primeiro evento aplicável entre término, entrega final, protocolo ou divulgação. Para as demais atividades do inciso III, informe o término; também será conferido o limite de 30 dias após o início. A data da atividade acima representa o início. Modalidade Múltiplo Mensal e outras hipóteses especiais exigem fundamento próprio, sujeito a aprovação."</p>
                    <label class="record-field"><span>"Enquadramento conforme o documento e a Resolução CAU 21/2012"</span><select name="rrt_framework"><option value="">"Selecionar o grupo ou atividade normativa"</option>{[RrtFramework::Project,RrtFramework::EnvironmentPlanning,RrtFramework::ProjectCoordination,RrtFramework::SafetySystemDesign,RrtFramework::FireProtectionDesign,RrtFramework::OtherActivities].into_iter().map(|framework|view!{<option value=framework_value(framework) selected={framework_value(framework)==rrt_framework}>{framework.label()}</option>}).collect_view()}</select></label>
                    <label class="record-field"><span>"Evento que limita o prazo do RRT"</span><select name="rrt_event"><option value="">"Não se aplica ou ainda não informado"</option>{[RrtEvent::ProjectCompletion,RrtEvent::FinalDelivery,RrtEvent::Submission,RrtEvent::Publication,RrtEvent::OtherActivityCompletion].into_iter().map(|event|view!{<option value=event_value(event) selected={event_value(event)==rrt_event}>{event.label()}</option>}).collect_view()}</select></label>
                    {text_field("Data do primeiro evento aplicável","rrt_event_on",field_value("rrt_event_on",spec.as_ref().and_then(|s|s.rrt_timing.as_ref().map(|t|t.event_on.to_string())).unwrap_or_default()),"date",false,10)}
                </details>
                <p class="record-small">"ART deve anteceder a atividade. Para RRT de execução, o registro deve anteceder o início. RRT de projeto e demais grupos têm condições próprias ligadas à entrega, protocolo, divulgação ou prazo normativo. Confira os eventos do seu caso; a data civil, sozinha, não prova a ordem dos fatos no mesmo dia."</p>
                <label class="record-field"><span>"Versão do arquivo anexado"</span><select name="document"><option value="">"Anexo ainda não enviado"</option>{ctx.documents.iter().map(|d|{let val=format!("{}:{}",d.document_id,d.version);let name=ctx.scopes.iter().find(|s|s.id==d.scope_id).map(|s|s.name.as_str()).unwrap_or("");view!{<option value=val.clone() selected={val==selected_doc}>{format!("{} · {}",name,d.label)}</option>}}).collect_view()}</select></label><p><a href="/documentos">"Enviar uma nova via documental"</a></p>
                <details open=source.is_some()><summary>"Exceção fundamentada"</summary><p>"Use somente quando houver fundamento aplicável ao caso. Salve para que outra pessoa, com permissão de aprovar exceções, confira a mesma redação e o anexo."</p>
                    {text_field("Norma, artigo ou instrumento aplicável","source_reference",field_value("source_reference",source.as_ref().map(|s|s.source_reference.clone()).unwrap_or_default()),"text",false,240)}
                    {text_field("Endereço público da fonte","source_url",field_value("source_url",source.as_ref().map(|s|s.source_url.clone()).unwrap_or_default()),"url",false,500)}
                    {area("Fundamento específico da exceção","reasoning",field_value("reasoning",source.as_ref().map(|s|s.reasoning.clone()).unwrap_or_default()),false,2000)}
                </details>
                {text_field("Motivo deste registro","reason",field_value("reason",if selected.is_some(){String::new()}else{"Cadastro da obrigação técnica".into()}),"text",true,500)}
                <div class="record-actions"><button type="submit" class="secondary-button" name="confirm" value="false">"Salvar pendência"</button><button type="submit" class="primary-button" name="confirm" value="true">"Conferir documento e registrar"</button>{(message.is_some()&&selected.is_some()).then(||view!{<button type="submit" class="secondary-button" name="reapply" value="true">"Conferi a versão atual: reaplicar pendência"</button>})}</div>
            </form></section>}
    });
    shell(
        "Obrigações técnicas",
        view! {{heading}<div class="record-layout"><div>{listing}{details}</div><div>{editor}</div></div>},
    )
}
fn detail(
    ctx: &ObligationsContext,
    o: ObligationRecord,
    key: &str,
    history: Vec<ObligationRecord>,
) -> impl IntoView + use<> {
    let id = o.id.clone();
    let source = o.spec.kind.source();
    let source_link = source.0.to_owned();
    let source_text = source.1.to_owned();
    let approval=ctx.context.as_ref().filter(|_|o.can_exception&&!o.confirmed&&o.spec.exception.is_some()).map(|scope|view!{<form method="post" action="/obrigacoes/acao" class="record-form">{hidden(scope,&ctx.csrf,key)}<input type="hidden" name="action" value="approve_exception"/><input type="hidden" name="obligation_id" value=o.id.clone()/><input type="hidden" name="scope_id" value=o.scope_id.clone()/><input type="hidden" name="expected_version" value=o.version.to_string()/>{text_field("Motivo da aprovação da exceção","reason",String::new(),"text",true,500)}<button type="submit" class="primary-button">"Aprovar a exceção apresentada"</button></form>});
    view! {<section class="foundation-card"><p class="eyebrow">{format!("Versão salva {}",o.version)}</p><h2>{format!("{} · {}",o.spec.kind.label(),o.spec.number)}</h2><p>{o.spec.description}</p><dl><dt>"Atividade"</dt><dd>{o.spec.activity.label()}</dd><dt>"Disciplina"</dt><dd>{o.spec.discipline}</dd><dt>"Responsável registrado"</dt><dd>{o.professional_name.unwrap_or_else(||"Não informado".into())}</dd><dt>"Emissor"</dt><dd>{o.spec.issuer}</dd><dt>"Vigência"</dt><dd>{format!("{} até {}",display_date(o.spec.valid_from),o.spec.valid_until.map(display_date).unwrap_or_else(||"sem término informado".into()))}</dd><dt>"Data da atividade"</dt><dd>{display_date(o.spec.activity_on)}</dd><dt>"Versão do anexo"</dt><dd>{o.spec.document_version.map(|n|n.to_string()).unwrap_or_else(||"Pendente".into())}</dd></dl>
        {o.spec.rrt_timing.map(|timing|view!{<p>{format!("Enquadramento do RRT: {}. Evento: {} em {}",timing.framework.label(),timing.event.label(),display_date(timing.event_on))}</p>})}
        {o.spec.exception.map(|ex|view!{<section class="record-notice"><h3>"Fundamento apresentado"</h3><p>{ex.source_reference}</p><p class="record-prewrap">{ex.reasoning}</p><a href=ex.source_url rel="noreferrer noopener" target="_blank">"Consultar fonte informada"</a><p>"O aceite desta exceção é uma decisão interna documentada; não autentica o anexo nem dispensa obrigações legais."</p></section>})}
        {approval}
        {(o.can_edit&&o.confirmed&&o.replaced_by.is_none()).then(||view!{<a class="secondary-button" href=format!("/obrigacoes?replace={}",id)>"Registrar substituição preservando esta via"</a>})}
        {o.replaced_by.map(|next|view!{<p>"Substituído por "<a href=format!("/obrigacoes?obligation={next}")>"nova via documental"</a>". A vigência e o responsável desta versão continuam preservados."</p>})}
        <details><summary>"Fonte e histórico"</summary><p>{source_text}</p>{(!source_link.is_empty()).then(||view!{<a href=source_link rel="noreferrer noopener" target="_blank">"Consultar fonte oficial"</a>})}<p><a href=format!("/obrigacoes?obligation={}&history=true",id)>"Carregar revisões preservadas"</a></p>{history.into_iter().map(|h|view!{<article class="record-summary"><h3>{format!("Versão {}",h.version)}</h3><p>{h.reason}</p><p>{display_instant(&h.recorded_at)}</p><p>{format!("{} · {} · responsável: {}",h.spec.kind.label(),h.spec.number,h.professional_name.unwrap_or_else(||"não informado".into()))}</p><p>{format!("Vigência: {} até {}",display_date(h.spec.valid_from),h.spec.valid_until.map(display_date).unwrap_or_else(||"sem término".into()))}</p></article>}).collect_view()}</details>
    </section>}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn obligations_show_nature_limits_and_private_empty_state_without_edit_form() {
        let ctx = ObligationsContext {
            csrf: String::new(),
            context: None,
            organization_name: String::new(),
            today: "2026-11-19".into(),
            scopes: vec![],
            obligations: vec![],
            professionals: vec![],
            documents: vec![],
        };
        let rendered = page(ctx, None, None, vec![], BTreeMap::new(), None, "key".into());
        assert!(rendered.contains("Ele não autentica ART ou RRT"));
        assert!(rendered.contains("não equivale a dispensa legal"));
        assert!(rendered.contains("Selecione uma empresa"));
        assert!(!rendered.contains("action=\"/obrigacoes/acao\""));
    }
}
