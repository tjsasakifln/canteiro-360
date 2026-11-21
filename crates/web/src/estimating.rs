//! Visão orçamentária nativa, com fontes históricas visíveis e exportação autorizada.
use crate::projects::shell;
use canteiro_contracts::estimating::*;
use leptos::prelude::*;

pub fn page(ctx: EstimatingContext) -> String {
    let scopes = ctx.scopes.clone();
    let selected = ctx.context.clone();
    let catalogs = ctx
        .catalogs
        .clone()
        .into_iter()
        .map(|catalog| {
            let source = catalog.source;
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · edição {}",catalog.catalog_id,catalog.edition)}</h3>
                    <p>{format!("Fonte: {} / {}",source.organization,source.title)}</p>
                    <p>{format!("Competência: {} · Localidade: {} · Regime: {}",
                        source.competence,source.locality,source.tax_regime)}</p>
                    <p>{format!("Licença de uso: {}",source.license)}</p>
                </article>
            }
        })
        .collect_view();
    let compositions = ctx
        .compositions
        .clone()
        .into_iter()
        .map(|entry| {
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · revisão {}",entry.code,entry.version)}</h3>
                    <p>{entry.composition.description}</p>
                    <p>{format!("Revisado em {}",entry.recorded_at)}</p>
                </article>
            }
        })
        .collect_view();
    let budgets=ctx.budgets.clone().into_iter().map(|entry|{
        let total=entry.snapshot.summarize().ok();
        let scope=scopes.iter().find(|s|s.id==entry.scope_id);
        let title=scope.map_or("Orçamento".to_owned(),|s|s.name.clone());
        let export=selected.as_ref().map(|s|format!(
            "/api/estimating/budget/{}/export.csv?organization_id={}&context_version={}&locale=pt_br",
            entry.scope_id,s.organization_id,s.context_version));
        let state=match entry.snapshot.status{
            BudgetState::Draft=>"Rascunho",
            BudgetState::Review=>"Em conferência",
            BudgetState::Approved=>"Aprovado",
        };
        view!{
            <article class="record-summary">
                <h3>{title}</h3>
                <p class="record-status">{state}</p>
                <p>{format!("Revisão {} · {} variações · {} serviços",
                    entry.version,entry.snapshot.variations.len(),entry.snapshot.lines.len())}</p>
                <p>{format!("Valor dos serviços: R$ {}",total
                    .map(|v|v.total.to_string()).unwrap_or_else(||"Inconsistente".into()))}</p>
                <p>{format!("Última alteração: {}",entry.recorded_at)}</p>
                {export.map(|url|view!{<a class="secondary-button" href=url>
                    "Exportar CSV editável"</a>})}
            </article>
        }
    }).collect_view();
    let scope_options = scopes
        .into_iter()
        .map(|scope| {
            let disabled = !scope.can_edit;
            view! {<option value=scope.id disabled=disabled>{scope.name}</option>}
        })
        .collect_view();
    shell(
        "Orçamentos e composições",
        view! {
            <section class="record-heading">
                <p class="eyebrow">{ctx.organization_name.clone()}</p>
                <h1>"Orçamentos, preços e composições"</h1>
                <p>"Trabalhe com fontes identificadas por competência, revisão de orçamento e rastreabilidade das alterações."</p>
            </section>
            {ctx.context.is_none().then(||view!{
                <section class="foundation-card"><h2>"Escolha uma empresa"</h2>
                    <a href="/organizacoes">"Selecionar empresa"</a></section>})}
            <section class="foundation-card">
                <h2>"Orçamentos disponíveis"</h2>
                {ctx.budgets.is_empty().then(||view!{<p>
                    "Nenhum orçamento registrado nos escopos a que você tem acesso."</p>})}
                {budgets}
            </section>
            <section class="foundation-card">
                <h2>"Importar orçamento CSV"</h2>
                <p>"Cole o arquivo no formato oficial, escolha uma obra orçamentária e confira a prévia. Nada será gravado durante a conferência."</p>
                {selected.map(|scope|view!{
                    <form action="/orcamentos/conferir-csv" method="post" class="record-form">
                        <input type="hidden" name="organization_id" value=scope.organization_id/>
                        <input type="hidden" name="context_version" value=scope.context_version.to_string()/>
                        <input type="hidden" name="csrf" value=ctx.csrf.clone()/>
                        <label class="record-field"><span>"Orçamento"</span>
                            <select name="scope_id" required=true>{scope_options}</select></label>
                        <label class="record-field"><span>"CSV em português brasileiro"</span>
                            <textarea name="csv" rows="8" maxlength="8500000" required=true></textarea>
                        </label>
                        <button class="primary-button" type="submit">"Conferir arquivo"</button>
                    </form>
                })}
            </section>
            <section class="foundation-card">
                <h2>"Fontes de custos"</h2>
                {ctx.catalogs.is_empty().then(||view!{<p>"Nenhum catálogo publicado."</p>})}
                {catalogs}
            </section>
            <section class="foundation-card">
                <h2>"Composições"</h2>
                {ctx.compositions.is_empty().then(||view!{<p>"Nenhuma composição cadastrada."</p>})}
                {compositions}
            </section>
        },
    )
}
pub fn preview(ctx: EstimatingContext, result: BudgetImportPreviewReply) -> String {
    let total = result.total;
    shell(
        "Prévia da importação",
        view! {
            <section class="record-heading">
                <h1>"Prévia da importação"</h1>
                <p>"Nenhum serviço foi gravado. Confira as diferenças antes de iniciar uma nova revisão."</p>
                <p>{format!("Formato: {} · revisão existente: {}",
                    result.format,result.base_version.map(|v|v.to_string()).unwrap_or_else(||"nova".into()))}</p>
            </section>
            <section class="foundation-card">
                <h2>"Diferenças identificadas"</h2>
                <p>{format!("Incluídos: {} · modificados: {} · removidos: {}",
                    result.difference.included,result.difference.changed,result.difference.removed)}</p>
                <p>{format!("Total calculado: R$ {}",total.total)}</p>
                <p>{format!("Quantidade de serviços: {}",result.snapshot.lines.len())}</p>
                <p>"Para salvar, obtenha a reserva de edição e utilize o comando autenticado de revisão. Esta prévia não equivale a uma aprovação."</p>
                <a href="/orcamentos">"Voltar para os orçamentos"</a>
            </section>
            <section class="foundation-card">
                <h2>"Empresa selecionada"</h2><p>{ctx.organization_name}</p>
            </section>
        },
    )
}
