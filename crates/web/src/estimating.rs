//! Visão orçamentária nativa, com fontes históricas visíveis e exportação autorizada.
use crate::projects::shell;
use canteiro_contracts::estimating::*;
use leptos::prelude::*;

pub fn page(ctx: EstimatingContext) -> String {
    let scopes = ctx.scopes.clone();
    let selected = ctx.context.clone();
    let all_catalogs = ctx.catalogs.clone();
    let comparison_context = ctx.context.clone();
    let current_competence = chrono::Utc::now().format("%Y-%m").to_string();
    let catalogs = ctx
        .catalogs
        .clone()
        .into_iter()
        .map(|catalog| {
            let mut earlier = all_catalogs
                .iter()
                .filter(|other| {
                    other.scope_id == catalog.scope_id
                        && other.catalog_id == catalog.catalog_id
                        && other.edition < catalog.edition
                })
                .collect::<Vec<_>>();
            earlier.sort_by_key(|other| std::cmp::Reverse(other.edition));
            let compare = comparison_context.as_ref().and_then(|context| {
                if earlier.is_empty() {
                    return None;
                }
                let options = earlier
                    .into_iter()
                    .map(|prior| {
                        view! {
                            <option value=prior.edition.to_string()>
                                {format!("Edição {} · competência {}", prior.edition, prior.source.competence)}
                            </option>
                        }
                    })
                    .collect_view();
                Some(view! {
                    <form action="/orcamentos/catalogos/comparar" method="get" class="record-form">
                        <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                        <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                        <input type="hidden" name="scope_id" value=catalog.scope_id.clone()/>
                        <input type="hidden" name="catalog_id" value=catalog.catalog_id.clone()/>
                        <input type="hidden" name="after" value=catalog.edition.to_string()/>
                        <label class="record-field">
                            <span>{format!("Comparar a edição {} com uma edição histórica", catalog.edition)}</span>
                            <select name="before" required=true>{options}</select>
                        </label>
                        <button class="secondary-button" type="submit">"Conferir diferenças de preços"</button>
                    </form>
                })
            });
            let source = catalog.source;
            let historical = source.competence < current_competence;
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · edição {}",catalog.catalog_id,catalog.edition)}</h3>
                    <p>{format!("Fonte: {} / {}",source.organization,source.title)}</p>
                    <p>{format!("Competência: {} · Localidade: {} · Regime: {}",
                        source.competence,source.locality,source.tax_regime)}</p>
                    <p>{format!("Licença de uso: {}",source.license)}</p>
                    {historical.then(||view!{<p class="record-status">
                        "Base histórica. Os valores não representam preços da competência atual."
                    </p>})}
                    {compare}
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
    let catalog_selected = selected.clone();
    let catalog_scope_options = ctx
        .scopes
        .clone()
        .into_iter()
        .map(|scope| {
            let disabled = !scope.can_edit;
            view! { <option value=scope.id disabled=disabled>{scope.name}</option> }
        })
        .collect_view();
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
                <h2>"Importar tabela de preços"</h2>
                <p>"Informe a fonte e cole o CSV versionado. A prévia examina todas as linhas sem gravar e compara os valores com a edição histórica anterior. A publicação exige confirmação."</p>
                {catalog_selected.map(|context| view! {
                    <form action="/orcamentos/catalogos/conferir" method="post" class="record-form">
                        <input type="hidden" name="organization_id" value=context.organization_id/>
                        <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                        <input type="hidden" name="csrf" value=ctx.csrf.clone()/>
                        <label class="record-field"><span>"Orçamento vinculado"</span>
                            <select name="scope_id" required=true>{catalog_scope_options}</select></label>
                        <label class="record-field"><span>"Identificador da tabela"</span>
                            <input type="text" name="catalog_id" maxlength="128" required=true/></label>
                        <label class="record-field"><span>"Número da edição"</span>
                            <input type="number" name="edition" min="1" required=true/></label>
                        <label class="record-field"><span>"Data de publicação da fonte"</span>
                            <input type="date" name="published_on" required=true/></label>
                        <label class="record-field"><span>"Título da tabela de origem"</span>
                            <input type="text" name="source_title" maxlength="160" required=true/></label>
                        <label class="record-field"><span>"Organização responsável pela fonte"</span>
                            <input type="text" name="source_organization" maxlength="160" required=true/></label>
                        <label class="record-field"><span>"Condição ou licença autorizando uso dos preços"</span>
                            <input type="text" name="source_license" maxlength="300" required=true/></label>
                        <label class="record-field"><span>"Localidade de referência"</span>
                            <input type="text" name="source_locality" maxlength="120" required=true/></label>
                        <label class="record-field"><span>"Regime tributário da fonte"</span>
                            <input type="text" name="source_tax_regime" maxlength="120" required=true/></label>
                        <label class="record-field"><span>"Competência (AAAA-MM)"</span>
                            <input type="month" name="source_competence" required=true/></label>
                        <label class="record-field"><span>"Justificativa da publicação"</span>
                            <input type="text" name="reason" maxlength="240" required=true/></label>
                        <label class="record-field"><span>"CSV no formato CANTEIRO-CUSTOS-CSV-1"</span>
                            <textarea name="csv" rows="7" maxlength="8000000" required=true
                                placeholder="CANTEIRO-CUSTOS-CSV-1&#10;codigo;descricao;categoria;unidade;preco&#10;A;Concreto;material;m³;350,00"></textarea></label>
                        <p>"Categorias: material, labor, equipment, service, transport. Unidades: un, m, m², m³, kg, t, L, h. No arquivo pt-BR, use vírgula decimal. Evite colar fórmulas."</p>
                        <button class="primary-button" type="submit">"Conferir preços antes de publicar"</button>
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

pub fn catalog_comparison(ctx: EstimatingContext, result: CatalogComparison) -> String {
    let added_count = result.added.len();
    let removed_count = result.removed.len();
    let changed_count = result.changed.len();
    let added = result
        .added
        .into_iter()
        .map(|item| {
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · {}",item.code,item.description)}</h3>
                    <p>{format!("{} · R$ {}",item.unit,item.price)}</p>
                </article>
            }
        })
        .collect_view();
    let removed = result
        .removed
        .into_iter()
        .map(|item| {
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · {}",item.code,item.description)}</h3>
                    <p>{format!("{} · R$ {}",item.unit,item.price)}</p>
                </article>
            }
        })
        .collect_view();
    let changed = result
        .changed
        .into_iter()
        .map(|item| {
            view! {
                <article class="record-summary">
                    <h3>{item.after.code.clone()}</h3>
                    <p>{format!("Antes: {} · {} · R$ {}",
                        item.before.description,item.before.unit,item.before.price)}</p>
                    <p>{format!("Depois: {} · {} · R$ {}",
                        item.after.description,item.after.unit,item.after.price)}</p>
                </article>
            }
        })
        .collect_view();
    let before = result.before_source;
    let after = result.after_source;
    shell(
        "Comparar edições de custos",
        view! {
            <section class="record-heading">
                <h1>{format!("Comparação da tabela {}",result.catalog_id)}</h1>
                <p>"As edições são imutáveis. Esta comparação não atualiza contratos ou orçamentos existentes."</p>
                <a href="/orcamentos" class="secondary-button">"Voltar aos orçamentos"</a>
            </section>
            <section class="foundation-card">
                <h2>{format!("Edição {} · fonte anterior",result.before_edition)}</h2>
                <p>{format!("{} / {} · competência {}",
                    before.organization,before.title,before.competence)}</p>
                <p>{format!("Localidade: {} · Regime: {} · Licença: {}",
                    before.locality,before.tax_regime,before.license)}</p>
            </section>
            <section class="foundation-card">
                <h2>{format!("Edição {} · fonte posterior",result.after_edition)}</h2>
                <p>{format!("{} / {} · competência {}",
                    after.organization,after.title,after.competence)}</p>
                <p>{format!("Localidade: {} · Regime: {} · Licença: {}",
                    after.locality,after.tax_regime,after.license)}</p>
            </section>
            <section class="foundation-card">
                <h2>"Mudanças identificadas"</h2>
                <p>{format!("Incluídos: {} · removidos: {} · alterados: {}",
                    added_count,removed_count,changed_count)}</p>
                <h3>"Valores alterados ou dados corrigidos"</h3>
                {changed_count.eq(&0).then(||view!{<p>"Nenhum item alterado."</p>})}
                {changed}
                <h3>"Itens incluídos"</h3>
                {added_count.eq(&0).then(||view!{<p>"Nenhum item incluído."</p>})}
                {added}
                <h3>"Itens removidos"</h3>
                {removed_count.eq(&0).then(||view!{<p>"Nenhum item removido."</p>})}
                {removed}
            </section>
            <section class="foundation-card">
                <h2>"Empresa consultada"</h2>
                <p>{ctx.organization_name}</p>
            </section>
        },
    )
}


pub fn catalog_csv_preview(
    ctx: EstimatingContext,
    preview: CatalogCsvPreviewReply,
    input: CatalogCsvImportCommand,
) -> String {
    let sample = preview
        .sample
        .into_iter()
        .map(|item| {
            view! {
                <article class="record-summary">
                    <p>{format!("{} · {} · {} · R$ {}",
                        item.code,item.description,item.unit,item.price)}</p>
                </article>
            }
        })
        .collect_view();
    shell(
        "Conferir tabela de custos",
        view! {
            <section class="record-heading">
                <h1>"Conferência da tabela de preços"</h1>
                <p>"Nenhuma linha foi publicada. Confira a fonte, a competência, os valores e a comparação antes de confirmar."</p>
                <a href="/orcamentos">"Voltar sem publicar"</a>
            </section>
            <section class="foundation-card">
                <h2>{format!("{} · edição {}",preview.header.catalog_id,preview.header.edition)}</h2>
                <p>{format!("{} / {}",preview.header.source.organization,preview.header.source.title)}</p>
                <p>{format!("Competência {} · Localidade {} · Regime {}",
                    preview.header.source.competence,preview.header.source.locality,
                    preview.header.source.tax_regime)}</p>
                <p>{format!("Licença: {}",preview.header.source.license)}</p>
                <p>{format!("{} preços examinados",preview.items_total)}</p>
                <p>{format!("Comparação com edição {}: {} incluídos, {} alterados, {} removidos",
                    preview.previous_edition.map_or("nenhuma".into(),|value|value.to_string()),
                    preview.included,preview.changed,preview.removed)}</p>
                <h3>"Amostra dos primeiros 20 itens"</h3>
                {sample}
            </section>
            <section class="foundation-card">
                <h2>"Confirmar publicação imutável"</h2>
                <p>"O servidor revalidará integralmente os dados, a permissão e a inexistência da edição. A publicação será integral ou não ocorrerá."</p>
                <form action="/orcamentos/catalogos/publicar-csv" method="post" class="record-form">
                    <input type="hidden" name="organization_id" value=input.context.organization_id/>
                    <input type="hidden" name="context_version" value=input.context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=input.scope_id/>
                    <input type="hidden" name="catalog_id" value=input.catalog_id/>
                    <input type="hidden" name="edition" value=input.edition.to_string()/>
                    <input type="hidden" name="published_on" value=input.published_on/>
                    <input type="hidden" name="source_title" value=input.source.title/>
                    <input type="hidden" name="source_organization" value=input.source.organization/>
                    <input type="hidden" name="source_license" value=input.source.license/>
                    <input type="hidden" name="source_locality" value=input.source.locality/>
                    <input type="hidden" name="source_tax_regime" value=input.source.tax_regime/>
                    <input type="hidden" name="source_competence" value=input.source.competence/>
                    <input type="hidden" name="reason" value=input.reason/>
                    <input type="hidden" name="csrf" value=ctx.csrf.clone()/>
                    <textarea hidden=true name="csv">{input.csv}</textarea>
                    <button class="primary-button" type="submit">"Publicar edição de preços"</button>
                </form>
            </section>
        },
    )
}

pub fn catalog_csv_published(ctx: EstimatingContext, header: CatalogHeader) -> String {
    shell(
        "Tabela de preços publicada",
        view! {
            <section class="record-heading">
                <h1>"Tabela publicada"</h1>
                <p>{format!("A tabela {} · edição {} foi publicada integralmente.",
                    header.catalog_id,header.edition)}</p>
                <p>{format!("Fonte: {} · competência {} · licença {}",
                    header.source.organization,header.source.competence,header.source.license)}</p>
                <p>{format!("Empresa: {}",ctx.organization_name)}</p>
                <a class="primary-button" href="/orcamentos">"Ver tabelas e comparar edições"</a>
            </section>
        },
    )
}
