//! Páginas de registro renderizadas no servidor, operáveis sem scripts.
use canteiro_contracts::{projects::*, reliability::ReliabilityScope};
use leptos::prelude::*;
use std::collections::BTreeMap;

pub fn shell(title: &str, body: impl IntoView) -> String {
    let title = title.to_owned();
    view!{<!DOCTYPE html><html lang="pt-BR"><head><meta charset="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><title>{format!("{title} | Canteiro 360")}</title><link rel="stylesheet" href="/pkg/canteiro-web.css"/><style>{include_str!("../../../style/projects.css")}</style></head><body>
        <a class="skip-link" href="#conteudo">"Pular para o conteúdo"</a>
        <header class="site-header"><div class="page-width header-inner"><a class="brand" href="/">"Canteiro 360"</a><nav class="record-navigation" aria-label="Áreas"><a href="/carteira">"Obras"</a><a href="/cadastros">"Cadastros"</a><a href="/projetos">"Projetos"</a><a href="/obrigacoes">"Obrigações"</a><a href="/documentos">"Documentos"</a><a href="/orcamentos">"Orçamentos"</a><a href="/pendencias">"Pendências"</a><a href="/organizacoes">"Empresa"</a></nav></div></header>
        <main id="conteudo" class="page-width record-page" tabindex="-1">{body}</main>
        <footer class="page-width site-footer"><p>"Canteiro 360 · Ambiente em desenvolvimento"</p></footer>
    </body></html>}.to_html()
}
pub fn hidden(scope: &ReliabilityScope, csrf: &str, key: &str) -> impl IntoView + use<> {
    view! {<input type="hidden" name="organization_id" value=scope.organization_id.clone()/><input type="hidden" name="context_version" value=scope.context_version.to_string()/><input type="hidden" name="csrf" value=csrf.to_owned()/><input type="hidden" name="operation_key" value=key.to_owned()/>}
}
pub fn text_field(
    label: &str,
    name: &str,
    value: String,
    kind: &str,
    required: bool,
    max: usize,
) -> impl IntoView + use<> {
    view! {<label class="record-field"><span>{label.to_owned()}</span><input type=kind.to_owned() name=name.to_owned() value=value required=required maxlength=max/></label>}
}
pub fn area(
    label: &str,
    name: &str,
    value: String,
    required: bool,
    max: usize,
) -> impl IntoView + use<> {
    view! {<label class="record-field"><span>{label.to_owned()}</span><textarea name=name.to_owned() required=required maxlength=max rows="3">{value}</textarea></label>}
}
pub fn option_list(choices: &[RecordChoice], selected: &str, empty: &str) -> impl IntoView + use<> {
    let selected = selected.to_owned();
    let choices = choices.to_vec();
    view! {<option value="">{empty.to_owned()}</option>{choices.into_iter().map(|choice|{let is_selected=choice.id==selected;view!{<option value=choice.id selected=is_selected>{choice.name}</option>}}).collect_view()}}
}
fn draft_value(draft: &BTreeMap<String, String>, key: &str, fallback: &str) -> String {
    draft
        .get(key)
        .cloned()
        .unwrap_or_else(|| fallback.to_owned())
}
pub fn display_date(value: canteiro_domain::CivilDate) -> String {
    value.as_naive_date().format("%d/%m/%Y").to_string()
}
pub fn display_instant(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%d/%m/%Y às %H:%M:%S (Brasília)")
                .to_string()
        })
        .unwrap_or_else(|_| "Data indisponível".into())
}
pub fn context_changed(
    draft: &BTreeMap<String, String>,
    current: Option<&ReliabilityScope>,
) -> bool {
    let Some(organization) = draft.get("organization_id") else {
        return false;
    };
    current.is_none_or(|scope| {
        scope.organization_id != *organization
            || draft
                .get("context_version")
                .and_then(|v| v.parse::<i64>().ok())
                != Some(scope.context_version)
    })
}
pub fn preserved_context_page(
    title: &str,
    company: &str,
    draft: BTreeMap<String, String>,
) -> String {
    let values=draft.into_iter().filter(|(key,_)|!matches!(key.as_str(),"csrf"|"operation_key"|"context_version"|"organization_id"|"expected_version"|"action"|"reapply"|"confirm")).map(|(key,value)|{
        let label=match key.as_str(){"title"=>"Título","discipline"=>"Disciplina","objective"=>"Objetivo","scope"=>"Escopo","description"=>"Descrição","reason"=>"Motivo","delivery_title"=>"Entrega","acceptance_criteria"=>"Critério de aceite","due_on"=>"Prazo","number"=>"Número","issuer"=>"Emissor","valid_from"=>"Início da vigência","valid_until"=>"Fim da vigência","activity_on"=>"Data da atividade","registered_on"=>"Data do registro","source_reference"=>"Fonte da exceção","source_url"=>"Endereço da fonte","reasoning"=>"Fundamento","external_reference"=>"Referência externa",_=>"Seleção preservada"};
        view!{<div class="record-summary"><h3>{label}</h3><p class="record-prewrap">{value}</p></div>}
    }).collect_view();
    shell(
        title,
        view! {<section class="record-heading"><h1>"A empresa ativa mudou"</h1><p role="alert">"Nenhuma alteração foi gravada. Os dados abaixo continuam ligados ao formulário anterior e não serão transferidos para a empresa atual."</p><p>{format!("Empresa ativa agora: {company}")}</p><p>"Confira a empresa em outra página. Depois, abra o cadastro correto e copie o preenchimento que deseja manter."</p><a class="primary-button" href="/organizacoes">"Conferir empresa ativa"</a></section><section class="foundation-card"><h2>"Preenchimento preservado para consulta"</h2>{values}</section>},
    )
}

pub fn page(
    ctx: ProjectsContext,
    selection: Option<&str>,
    history: Vec<ProjectRecord>,
    draft: BTreeMap<String, String>,
    message: Option<String>,
    key: String,
) -> String {
    if context_changed(&draft, ctx.context.as_ref()) {
        return preserved_context_page("Projetos e entregas", &ctx.organization_name, draft);
    }
    let selected = selection
        .and_then(|id| ctx.projects.iter().find(|p| p.id == id))
        .cloned();
    let list = ctx.projects.clone();
    let has_context = ctx.context.is_some();
    let top = view! {<section class="record-heading"><p class="eyebrow">{ctx.organization_name.clone()}</p><h1>"Projetos e entregas"</h1><p>"Organize o escopo, os responsáveis e os próximos compromissos. O projeto pode começar antes de existir obra ou contrato."</p><div class="record-actions"><a class="secondary-button" href="/projetos">"Novo projeto"</a><a href="/permissoes">"Gerir permissões"</a></div></section>
        {message.clone().map(|text|view!{<div class="record-notice" role="alert" tabindex="-1"><h2>"Confira antes de continuar"</h2><p>{text}</p><p>"Os campos enviados permanecem abaixo. Compare com os dados salvos antes de reaplicar."</p></div>})}
        {(!has_context).then(||view!{<section class="foundation-card"><h2>"Selecione uma empresa"</h2><p>"Entre na sua conta e escolha a empresa para consultar projetos e permissões."</p><a href="/organizacoes">"Escolher empresa"</a></section>})}
    };
    let listing = view! {<section class="foundation-card"><h2>"Projetos disponíveis"</h2>{list.is_empty().then(||view!{<p>"Nenhum projeto disponível para suas permissões nesta empresa."</p>})}<div class="record-cards">{list.into_iter().map(|p|view!{<article class="record-summary"><h3><a href=format!("/projetos?project={}",p.id)>{p.snapshot.spec.title.clone()}</a></h3><p>{p.snapshot.spec.discipline.clone()}</p><p class="record-status">{p.snapshot.status.label()}" · "{format!("versão {}",p.version)}</p><p>{format!("{} entregas, {} aceitas",p.snapshot.spec.deliveries.len(),p.snapshot.accepted_deliveries.len())}</p></article>}).collect_view()}</div></section>};
    let details = selected
        .clone()
        .map(|record| project_detail(&ctx, record, &key, &history));
    let can_edit = selected.as_ref().map_or(ctx.can_create, |p| {
        p.can_edit && p.snapshot.status != ProjectStatus::Closed
    });
    let editor=ctx.context.clone().filter(|_|can_edit).map(|scope|{
        let spec=selected.as_ref().map(|p|p.snapshot.spec.clone());
        let draft_is_delivery=draft.get("action").is_some_and(|a|a=="delivery");
        let editing_draft=if draft_is_delivery{BTreeMap::new()}else{draft.clone()};
        let val=|key:&str,base:String|draft_value(&editing_draft,key,&base);
        let id=selected.as_ref().map(|p|p.id.clone()).unwrap_or_default();
        let expected=selected.as_ref().map(|p|p.version.to_string()).unwrap_or_default();
        let expected=val("expected_version",expected);
        let work=val("work_id",spec.as_ref().and_then(|s|s.work_id).map(|w|w.to_string()).unwrap_or_default());
        let stakeholders=if editing_draft.get("action").is_some_and(|action|action=="save") {editing_draft.iter().filter(|(key,_)|key.starts_with("stakeholder_")).map(|(_,value)|value.clone()).collect::<Vec<_>>().join(",")}else{spec.as_ref().map(|s|s.stakeholder_ids.iter().map(ToString::to_string).collect::<Vec<_>>().join(",")).unwrap_or_default()};
        view!{<section class="foundation-card"><h2>{if selected.is_some(){"Revisar escopo"}else{"Cadastrar projeto"}}</h2><p>"Alterar um escopo aprovado abre nova revisão em elaboração e mantém a anterior no histórico."</p>
            <form method="post" action="/projetos/acao" class="record-form">{hidden(&scope,&ctx.csrf,&key)}<input type="hidden" name="action" value="save"/><input type="hidden" name="project_id" value=id/><input type="hidden" name="expected_version" value=expected/>
                {text_field("Título do projeto","title",val("title",spec.as_ref().map(|s|s.title.clone()).unwrap_or_default()),"text",true,160)}
                {text_field("Disciplina","discipline",val("discipline",spec.as_ref().map(|s|s.discipline.clone()).unwrap_or_default()),"text",true,80)}
                {area("Objetivo","objective",val("objective",spec.as_ref().map(|s|s.objective.clone()).unwrap_or_default()),true,1000)}
                {area("Escopo e limites do trabalho","scope",val("scope",spec.as_ref().map(|s|s.scope.clone()).unwrap_or_default()),true,8000)}
                <details><summary>"Vínculos e identificação externa"</summary><label class="record-field"><span>"Obra vinculada (opcional)"</span><select name="work_id">{option_list(&ctx.works,&work,"Projeto independente de obra")}</select></label>
                    {text_field("Referência externa (opcional)","external_reference",val("external_reference",spec.as_ref().map(|s|s.external_reference.clone()).unwrap_or_default()),"text",false,120)}
                    <fieldset><legend>"Partes interessadas"</legend><p>"Informe as pessoas e empresas participantes. O vínculo não concede acesso ao sistema."</p>{ctx.parties.iter().map(|p|{let checked=stakeholders.split(',').any(|id|id==p.id);view!{<label class="record-check"><input type="checkbox" name=format!("stakeholder_{}",p.id) value=p.id.clone() checked=checked/>{p.name.clone()}</label>}}).collect_view()}</fieldset>
                </details>
                {text_field("Motivo da revisão","reason",val("reason",if selected.is_some(){String::new()}else{"Cadastro inicial".into()}),"text",true,500)}
                <div class="record-actions"><button type="submit" class="primary-button">"Salvar em elaboração"</button>{(message.is_some()&&selected.is_some()).then(||view!{<button type="submit" class="secondary-button" name="reapply" value="true">"Conferi a versão atual e quero reaplicar"</button>})}</div>
            </form></section>}
    });
    let delivery=selected.clone().filter(|p|p.can_edit&&p.snapshot.status!=ProjectStatus::Closed).and_then(|record|ctx.context.clone().map(|scope|{
        let delivery_draft=if draft.get("action").is_some_and(|a|a=="delivery"){draft.clone()}else{BTreeMap::new()};
        let value=|key:&str|draft_value(&delivery_draft,key,"");
        view!{<section class="foundation-card"><h2>"Adicionar ou revisar entrega"</h2><p>"Responsável, prazo e critério de aceite serão exigidos na aprovação do escopo."</p><form method="post" action="/projetos/acao" class="record-form">{hidden(&scope,&ctx.csrf,&key)}<input type="hidden" name="action" value="delivery"/><input type="hidden" name="project_id" value=record.id.clone()/><input type="hidden" name="expected_version" value=draft_value(&delivery_draft,"expected_version",&record.version.to_string())/>
            <label class="record-field"><span>"Entrega"</span><select name="delivery_id"><option value="">"Criar nova entrega"</option>{record.snapshot.spec.deliveries.iter().map(|d|{let id=d.id.to_string();let selected=value("delivery_id")==id;view!{<option value=id selected=selected>{d.title.clone()}</option>}}).collect_view()}</select></label>
            {text_field("Título da entrega","delivery_title",value("delivery_title"),"text",true,160)}
            <label class="record-field"><span>"Profissional responsável"</span><select name="responsible_id">{option_list(&ctx.professionals,&value("responsible_id"),"Selecionar antes da aprovação")}</select></label>
            {text_field("Prazo da entrega","due_on",value("due_on"),"date",false,10)}
            {area("Como a entrega será aceita","acceptance_criteria",value("acceptance_criteria"),false,2000)}
            <label class="record-check"><input type="checkbox" name="milestone" value="true" checked={value("milestone")=="true"}/>"Esta entrega é um marco do projeto"</label>
            {text_field("Motivo","reason",draft_value(&delivery_draft,"reason","Definição de entrega"),"text",true,500)}
            <div class="record-actions"><button type="submit" class="primary-button">"Salvar entrega em nova revisão"</button>{message.is_some().then(||view!{<button type="submit" class="secondary-button" name="reapply" value="true">"Conferi a versão atual e quero reaplicar"</button>})}</div>
        </form></section>}
    }));
    shell(
        "Projetos e entregas",
        view! { {top}<div class="record-layout"><div>{listing}{details}</div><div>{editor}{delivery}</div></div> },
    )
}
fn project_detail(
    ctx: &ProjectsContext,
    record: ProjectRecord,
    key: &str,
    history: &[ProjectRecord],
) -> impl IntoView + use<> {
    let scope = ctx.context.clone();
    let csrf = ctx.csrf.clone();
    let key = key.to_owned();
    let history = history.to_vec();
    let id = record.id.clone();
    let version = record.version;
    let state = record.snapshot.status;
    let controls=scope.as_ref().map(|scope|{
        let mut actions=vec![];
        if record.can_approve&&state==ProjectStatus::Draft{actions.push(("approve","Aprovar escopo"));}
        if record.can_edit&&state==ProjectStatus::Approved{actions.push(("start","Iniciar execução"));}
        if record.can_edit&&matches!(state,ProjectStatus::Approved|ProjectStatus::InProgress){actions.push(("close","Encerrar projeto"));}
        actions.into_iter().map(|(action,label)|view!{<form method="post" action="/projetos/acao" class="record-inline-form">{hidden(scope,&csrf,&key)}<input type="hidden" name="action" value=action/><input type="hidden" name="project_id" value=id.clone()/><input type="hidden" name="expected_version" value=version.to_string()/>{text_field("Motivo da decisão","reason",String::new(),"text",true,500)}<button type="submit" class="secondary-button">{label}</button></form>}).collect_view()
    });
    let deliveries=record.snapshot.spec.deliveries.clone().into_iter().map(|delivery|{
        let accepted=record.snapshot.accepted_deliveries.contains(&delivery.id);let name=delivery.responsible_id.and_then(|id|ctx.professionals.iter().find(|p|p.id==id.to_string()).map(|p|p.name.clone())).unwrap_or_else(||"Responsável ainda não definido ou cadastro inativo".into());
        let actions=scope.as_ref().map(|scope|view!{<div class="record-actions">{(!accepted&&record.can_accept&&matches!(state,ProjectStatus::Approved|ProjectStatus::InProgress)).then(||view!{<form method="post" action="/projetos/acao">{hidden(scope,&csrf,&key)}<input type="hidden" name="action" value="accept"/><input type="hidden" name="delivery_id" value=delivery.id.to_string()/><input type="hidden" name="project_id" value=id.clone()/><input type="hidden" name="expected_version" value=version.to_string()/>{text_field("Evidência e motivo do aceite","reason",String::new(),"text",true,500)}<button type="submit" class="secondary-button">"Registrar aceite técnico"</button></form>})}{(record.can_edit&&state==ProjectStatus::Draft).then(||view!{<form method="post" action="/projetos/acao">{hidden(scope,&csrf,&key)}<input type="hidden" name="action" value="remove"/><input type="hidden" name="delivery_id" value=delivery.id.to_string()/><input type="hidden" name="project_id" value=id.clone()/><input type="hidden" name="expected_version" value=version.to_string()/><input type="hidden" name="reason" value="Retirada da entrega do escopo em elaboração"/><button type="submit" class="text-link">"Retirar desta revisão"</button></form>})}</div>});
        view!{<article class="record-summary"><h3>{delivery.title}</h3><p class="record-status">{if accepted{"Aceita"}else{"Pendente"}}{if delivery.milestone{" · Marco"}else{""}}</p><p>{name}</p><p>{format!("Prazo: {}",delivery.due_on.map(display_date).unwrap_or_else(||"a definir".into()))}</p><p>{delivery.acceptance_criteria}</p>{actions}</article>}
    }).collect_view();
    view! {<section class="foundation-card"><p class="eyebrow">{format!("Versão salva {}",record.version)}</p><h2>{record.snapshot.spec.title}</h2><p class="record-status">{state.label()}</p><h3>"Objetivo"</h3><p class="record-prewrap">{record.snapshot.spec.objective}</p><h3>"Escopo"</h3><p class="record-prewrap">{record.snapshot.spec.scope}</p><p>"O aceite técnico não cria medição, fatura ou transferência de direitos."</p>{controls}<h3>"Entregas e próximos compromissos"</h3>{deliveries}<details><summary>"Histórico de decisões e revisões"</summary><a href=format!("/projetos?project={}&history=true",id)>"Carregar revisões preservadas"</a>{history.into_iter().map(|h|view!{<article class="record-summary"><h4>{format!("Versão {} · {}",h.version,h.snapshot.status.label())}</h4><p>{h.reason}</p><p>{display_instant(&h.recorded_at)}</p><p class="record-prewrap">{h.snapshot.spec.scope}</p><p>{format!("{} entregas; {} aceitas",h.snapshot.spec.deliveries.len(),h.snapshot.accepted_deliveries.len())}</p></article>}).collect_view()}</details></section>}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_native_form_escapes_preserved_input_and_retains_csrf_and_version() {
        let scope = ReliabilityScope {
            organization_id: "00000000-0000-4000-8000-000000000001".into(),
            context_version: 7,
        };
        let ctx = ProjectsContext {
            csrf: "server-csrf".into(),
            context: Some(scope),
            organization_name: "Empresa Fictícia".into(),
            can_create: true,
            projects: vec![],
            professionals: vec![],
            parties: vec![],
            works: vec![],
        };
        let unsafe_text = ["<scr", "ipt>alert(1)</scr", "ipt>"].concat();
        let draft = BTreeMap::from([
            ("title".into(), unsafe_text.clone()),
            ("scope".into(), "Escopo preservado".into()),
            ("action".into(), "save".into()),
        ]);
        let rendered = page(
            ctx,
            None,
            vec![],
            draft,
            Some("Corrija a disciplina".into()),
            "attempt-key".into(),
        );
        assert!(!rendered.contains(&unsafe_text));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        for expected in [
            "lang=\"pt-BR\"",
            "name=\"csrf\"",
            "server-csrf",
            "name=\"context_version\"",
            "Escopo preservado",
            "/projetos/acao",
            "role=\"alert\"",
            "Pular para o conteúdo",
        ] {
            assert!(rendered.contains(expected), "{expected}");
        }
    }
    #[test]
    fn changed_company_preserves_text_without_retargeting_a_form() {
        let ctx = ProjectsContext {
            csrf: "secret".into(),
            context: Some(ReliabilityScope {
                organization_id: "company-b".into(),
                context_version: 8,
            }),
            organization_name: "Empresa B".into(),
            can_create: true,
            projects: vec![],
            professionals: vec![],
            parties: vec![],
            works: vec![],
        };
        let draft = BTreeMap::from([
            ("organization_id".into(), "company-a".into()),
            ("context_version".into(), "7".into()),
            ("title".into(), "Projeto da empresa A".into()),
            ("csrf".into(), "old-secret".into()),
        ]);
        let html = page(
            ctx,
            None,
            vec![],
            draft,
            Some("Contexto alterado".into()),
            "key".into(),
        );
        assert!(html.contains("Projeto da empresa A"));
        assert!(html.contains("A empresa ativa mudou"));
        assert!(!html.contains("<form"));
        assert!(!html.contains("old-secret"));
        assert_eq!(
            display_instant("2026-11-20T01:30:00Z"),
            "19/11/2026 às 22:30:00 (Brasília)"
        );
    }
}
