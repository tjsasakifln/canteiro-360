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
    let editable_scopes = ctx.scopes.clone();
    let composition_csrf = ctx.csrf.clone();
    let compositions = ctx
        .compositions
        .clone()
        .into_iter()
        .map(|entry| {
            let memory = selected.as_ref().map(|context| {
                view! {
                    <form action="/orcamentos/composicoes/memoria" method="get">
                        <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                        <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                        <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                        <input type="hidden" name="code" value=entry.code.clone()/>
                        <button type="submit" class="secondary-button">"Conferir memória de cálculo"</button>
                    </form>
                }
            });
            let editing = selected
                .as_ref()
                .filter(|_| {
                    editable_scopes
                        .iter()
                        .any(|scope| scope.id == entry.scope_id && scope.can_edit)
                })
                .map(|context| {
                    let has_lines = !entry.composition.lines.is_empty();
                    let line_options = entry
                        .composition
                        .lines
                        .clone()
                        .into_iter()
                        .map(|line| {
                            view! {
                                <option value=line.id.clone()>
                                    {format!("{} · coeficiente atual {}", line.id, line.coefficient)}
                                </option>
                            }
                        })
                        .collect_view();
                    let copy_line_options = entry
                        .composition
                        .lines
                        .clone()
                        .into_iter()
                        .map(|line| {
                            view! {
                                <option value=line.id.clone()>
                                    {format!("{} · fonte e unidade preservadas", line.id)}
                                </option>
                            }
                        })
                        .collect_view();
                    let remove_line_options = entry
                        .composition
                        .lines
                        .clone()
                        .into_iter()
                        .map(|line| {
                            view! {
                                <option value=line.id.clone()>{line.id.clone()}</option>
                            }
                        })
                        .collect_view();
                    let has_incidences = !entry.composition.incidences.is_empty();
                    let incidence_options = entry
                        .composition
                        .incidences
                        .clone()
                        .into_iter()
                        .map(|incidence| {
                            view! {
                                <option value=incidence.id.clone()>
                                    {format!("{} · {}% (base: {})",
                                        incidence.id, incidence.percent, incidence.base_lines.join(", "))}
                                </option>
                            }
                        })
                        .collect_view();
                    let incidence_base_options = entry
                        .composition
                        .incidences
                        .clone()
                        .into_iter()
                        .map(|incidence| {
                            view! {
                                <option value=incidence.id.clone()>
                                    {format!("{} · base atual: {}", incidence.id, incidence.base_lines.join(", "))}
                                </option>
                            }
                        })
                        .collect_view();
                    let informed_cost = match &entry.composition.cost_mode {
                        canteiro_domain::compositions::CostMode::Informed { cost, .. } => {
                            Some(cost.to_string())
                        }
                        canteiro_domain::compositions::CostMode::Formula => None,
                    };
                    view! {
                        <details>
                            <summary>"Editar composição, insumos, índices e incidências"</summary>
                        <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                            <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                            <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                            <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                            <input type="hidden" name="code" value=entry.code.clone()/>
                            <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                            <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                            <label class="record-field">
                                <span>"Descrição técnica da composição"</span>
                                <textarea name="description" maxlength="800" rows="3" required=true>
                                    {entry.composition.description.clone()}
                                </textarea>
                            </label>
                            <label class="record-field">
                                <span>"Motivo da alteração"</span>
                                <input type="text" name="reason" maxlength="240" required=true/>
                            </label>
                            <button type="submit" class="secondary-button">"Salvar nova revisão da descrição"</button>
                        </form>
                        <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                            <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                            <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                            <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                            <input type="hidden" name="code" value=entry.code.clone()/>
                            <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                            <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                            <input type="hidden" name="change_kind" value="costs"/>
                            <p>"Informe valores percentuais em formato decimal com ponto."</p>
                            <label class="record-field">
                                <span>"BDI (%)"</span>
                                <input type="text" name="bdi" inputmode="decimal" maxlength="40"
                                    value=entry.composition.bdi.to_string() required=true/>
                            </label>
                            <label class="record-field">
                                <span>"Desconto (%)"</span>
                                <input type="text" name="discount" inputmode="decimal" maxlength="40"
                                    value=entry.composition.discount.to_string() required=true/>
                            </label>
                            <label class="record-field">
                                <span>"Motivo da alteração dos índices"</span>
                                <input type="text" name="reason" maxlength="240" required=true/>
                            </label>
                            <button type="submit" class="secondary-button">"Salvar índices em nova revisão"</button>
                        </form>
                        {has_lines.then(|| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="coefficient"/>
                                <label class="record-field">
                                    <span>"Linha de insumo a alterar"</span>
                                    <select name="line_id" required=true>{line_options}</select>
                                </label>
                                <label class="record-field">
                                    <span>"Novo coeficiente decimal (use ponto)"</span>
                                    <input type="text" name="coefficient" inputmode="decimal"
                                        maxlength="40" required=true/>
                                </label>
                                <label class="record-field">
                                    <span>"Motivo da alteração do coeficiente"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Salvar coeficiente em nova revisão"</button>
                            </form>
                        })}
                        {has_lines.then(|| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="duplicate_line"/>
                                <label class="record-field">
                                    <span>"Linha de origem (fonte e unidade preservadas)"</span>
                                    <select name="line_id" required=true>{copy_line_options}</select>
                                </label>
                                <label class="record-field">
                                    <span>"Identificador da nova linha"</span>
                                    <input type="text" name="new_line_id" maxlength="80" required=true/>
                                </label>
                                <label class="record-field">
                                    <span>"Coeficiente da nova linha (use ponto)"</span>
                                    <input type="text" name="coefficient" inputmode="decimal" maxlength="40" required=true/>
                                </label>
                                <label class="record-field">
                                    <span>"Justificativa da inclusão"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Duplicar insumo em nova revisão"</button>
                            </form>
                        })}
                        {has_lines.then(|| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="remove_line"/>
                                <label class="record-field">
                                    <span>"Linha de insumo a excluir"</span>
                                    <select name="line_id" required=true>{remove_line_options}</select>
                                </label>
                                <p>"Linhas utilizadas como base de incidência não podem ser excluídas."</p>
                                <label class="record-field">
                                    <span>"Justificativa da exclusão"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Excluir linha em nova revisão"</button>
                            </form>
                        })}
                        {has_incidences.then(|| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="incidence"/>
                                <label class="record-field">
                                    <span>"Incidência a alterar (bases preservadas)"</span>
                                    <select name="incidence_id" required=true>{incidence_options}</select>
                                </label>
                                <label class="record-field">
                                    <span>"Novo percentual da incidência (use ponto)"</span>
                                    <input type="text" name="incidence_percent" inputmode="decimal"
                                        maxlength="40" required=true/>
                                </label>
                                <label class="record-field">
                                    <span>"Motivo da alteração da incidência"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Salvar incidência em nova revisão"</button>
                            </form>
                        })}
                        {has_incidences.then(|| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="incidence_base"/>
                                <label class="record-field">
                                    <span>"Incidência com base a revisar"</span>
                                    <select name="incidence_id" required=true>{incidence_base_options}</select>
                                </label>
                                <label class="record-field">
                                    <span>"Nova base, com IDs existentes separados por vírgula"</span>
                                    <input type="text" name="base_lines" maxlength="2000"
                                        placeholder="insumo-01, insumo-02" required=true/>
                                </label>
                                <p>"Cada identificador deve existir na composição; não repita linhas."</p>
                                <label class="record-field">
                                    <span>"Motivo da alteração da base de incidência"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Salvar base da incidência em nova revisão"</button>
                            </form>
                        })}
                        {informed_cost.map(|amount| view! {
                            <form action="/orcamentos/composicoes/editar-descricao" method="post" class="record-form">
                                <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                                <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                                <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                                <input type="hidden" name="code" value=entry.code.clone()/>
                                <input type="hidden" name="expected_version" value=entry.version.to_string()/>
                                <input type="hidden" name="csrf" value=composition_csrf.clone()/>
                                <input type="hidden" name="change_kind" value="informed_cost"/>
                                <label class="record-field">
                                    <span>"Novo custo unitário informado (R$, use ponto)"</span>
                                    <input type="text" name="informed_cost" inputmode="decimal"
                                        maxlength="40" value=amount required=true/>
                                </label>
                                <label class="record-field">
                                    <span>"Justificativa da revisão de custo"</span>
                                    <input type="text" name="reason" maxlength="240" required=true/>
                                </label>
                                <button type="submit" class="secondary-button">"Salvar custo em nova revisão"</button>
                            </form>
                        })}
                        </details>
                    }
                });
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · revisão {}",entry.code,entry.version)}</h3>
                    <p>{entry.composition.description}</p>
                    <p>{format!("Revisado em {}",entry.recorded_at)}</p>
                    {memory}
                    {editing}
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
        let edit = selected
            .as_ref()
            .filter(|_| {
                scope.is_some_and(|scope| scope.can_edit)
                    && entry.snapshot.status != BudgetState::Approved
            })
            .map(|context| view! {
                <form action="/orcamentos/servicos/editar" method="get" class="record-form">
                    <input type="hidden" name="organization_id" value=context.organization_id.clone()/>
                    <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=entry.scope_id.clone()/>
                    <label class="record-field">
                        <span>"Identificador do serviço a editar"</span>
                        <input type="text" name="item_id" maxlength="120" required=true/>
                    </label>
                    <button type="submit" class="secondary-button">"Localizar serviço no orçamento"</button>
                </form>
            });
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
                {edit}
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
/// Memória autorizada, calculada novamente contra as edições imutáveis das fontes.
pub fn composition_memory(
    ctx: EstimatingContext,
    record: CompositionRecord,
    memory: CostMemory,
) -> String {
    use canteiro_domain::{
        catalogs::CostCategory,
        compositions::{ComponentSource, CostMode},
    };
    fn category(category: CostCategory) -> &'static str {
        match category {
            CostCategory::Material => "Materiais",
            CostCategory::Labor => "Mão de obra",
            CostCategory::Equipment => "Equipamentos",
            CostCategory::Service => "Serviços e composições vinculadas",
            CostCategory::Transport => "Transportes",
        }
    }
    let mode = match &record.composition.cost_mode {
        CostMode::Formula => "Custo obtido dos insumos e incidências".to_owned(),
        CostMode::Informed { reason, .. } => format!("Custo informado: {reason}"),
    };
    let categories = memory
        .category_totals
        .into_iter()
        .map(|entry| {
            view! {
                <p>{format!("{}: R$ {}",category(entry.category),entry.amount)}</p>
            }
        })
        .collect_view();
    let lines = memory
        .line_details
        .into_iter()
        .map(|entry| {
            let source = match &entry.source {
                ComponentSource::Catalog { catalog_id, edition, code } =>
                    format!("Fonte {catalog_id}, edição {edition}, item {code}"),
                ComponentSource::Composition { code } =>
                    format!("Composição vinculada {code}"),
            };
            view! {
                <article class="record-summary">
                    <h3>{format!("{} · {}",entry.id,category(entry.category))}</h3>
                    <p>{source}</p>
                    <p>{format!("Coeficiente {} {} · preço unitário convertido R$ {} · parcela R$ {}",
                        entry.coefficient,entry.unit,entry.unit_cost,entry.amount)}</p>
                </article>
            }
        })
        .collect_view();
    let incidences = memory
        .incidence_details
        .into_iter()
        .map(|entry| {
            view! {
                <article class="record-summary">
                    <h3>{format!("Incidência {}",entry.id)}</h3>
                    <p>{format!("Base: {} · R$ {}",entry.base_lines.join(", "),entry.basis)}</p>
                    <p>{format!("Alíquota {}% · parcela R$ {}",entry.percent,entry.amount)}</p>
                </article>
            }
        })
        .collect_view();
    shell(
        "Memória da composição",
        view! {
            <section class="record-heading">
                <h1>{format!("Memória de cálculo · {}",record.code)}</h1>
                <p>{format!("{} · revisão {} · registrada em {}",
                    record.composition.description,record.version,record.recorded_at)}</p>
                <p>{mode}</p>
                <a href="/orcamentos" class="secondary-button">"Voltar aos orçamentos"</a>
            </section>
            <section class="foundation-card">
                <h2>"Resumo financeiro por unidade da composição"</h2>
                <p>{format!("Custo dos insumos: R$ {}",memory.base_cost)}</p>
                <p>{format!("Incidências: R$ {}",memory.incidence_cost)}</p>
                <p>{format!("Custo direto: R$ {}",memory.direct_cost)}</p>
                <p>{format!("Preço de venda, após BDI e desconto: R$ {}",memory.selling_price)}</p>
                <p>{format!("BDI {}% · desconto {}%",
                    record.composition.bdi,record.composition.discount)}</p>
                {memory.free_item.then(|| view! { <p class="record-status">"Item sem cobrança"</p> })}
                <h3>"Custos por categoria, antes das incidências"</h3>
                {categories}
            </section>
            <section class="foundation-card">
                <h2>"Insumos e referências"</h2>
                {lines}
                <h2>"Incidências e bases utilizadas"</h2>
                {incidences}
                <p>"Valores unitários são arredondados para exibição após cálculo decimal exato. Composições vinculadas aparecem como serviço nesta memória."</p>
            </section>
            <section class="foundation-card">
                <h2>"Escopo e rastreabilidade"</h2>
                <p>{format!("Empresa: {} · escopo: {} · versão da composição: {}",
                    ctx.organization_name,record.scope_id,record.version)}</p>
                <p>"Esta consulta não altera a composição, a fonte de custos ou o orçamento aprovado."</p>
            </section>
        },
    )
}

/// Edição pontual, sem carregar milhares de serviços em formulários simultâneos.
/// O servidor reconfere sessão, versão, reserva e referências antes de persistir.
pub fn budget_line_editor(ctx: EstimatingContext, record: BudgetRecord, item_id: &str) -> String {
    let line = record
        .snapshot
        .lines
        .iter()
        .find(|line| line.id == item_id)
        .expect("O servidor localiza o serviço antes de renderizar")
        .clone();
    let context = ctx.context.expect("Edição requer empresa selecionada");
    let historical = line.catalog_id.as_ref().zip(line.catalog_edition).map(|(id, edition)| {
        format!("Preço fixado pela fonte histórica {id}, edição {edition}. Edite apenas a quantidade.")
    });
    let copy_context = context.clone();
    let copy_scope = record.scope_id.clone();
    let copy_item = line.id.clone();
    let copy_csrf = ctx.csrf.clone();
    let move_context = context.clone();
    let move_scope = record.scope_id.clone();
    let move_item = line.id.clone();
    let move_csrf = ctx.csrf.clone();
    let delete_context = context.clone();
    let delete_scope = record.scope_id.clone();
    let delete_item = line.id.clone();
    let delete_csrf = ctx.csrf.clone();
    let delete_disabled = line.catalog_id.is_some();
    let move_variation_options = record
        .snapshot
        .variations
        .clone()
        .into_iter()
        .map(|variation| {
            let selected = variation.id == line.variation_id;
            view! {
                <option value=variation.id selected=selected>{variation.scope}</option>
            }
        })
        .collect_view();
    let variation_options = record
        .snapshot
        .variations
        .clone()
        .into_iter()
        .map(|variation| {
            let selected = variation.id == line.variation_id;
            view! {
                <option value=variation.id selected=selected>{variation.scope}</option>
            }
        })
        .collect_view();
    shell(
        "Editar serviço do orçamento",
        view! {
            <section class="record-heading">
                <h1>{format!("Editar serviço · {}",line.code)}</h1>
                <p>{line.description}</p>
                <p>{format!("Orçamento {} · revisão {} · unidade {}",
                    record.scope_id,record.version,line.unit)}</p>
                <p>"A alteração cria nova revisão de rascunho e preserva o cronograma. Outra sessão pode impedir a gravação."</p>
                <a href="/orcamentos" class="secondary-button">"Voltar sem alterar"</a>
            </section>
            <section class="foundation-card">
                <h2>"Quantidade e preço unitário"</h2>
                {historical.map(|note|view!{<p class="record-status">{note}</p>})}
                <form action="/orcamentos/servicos/salvar" method="post" class="record-form">
                    <input type="hidden" name="organization_id" value=context.organization_id/>
                    <input type="hidden" name="context_version" value=context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=record.scope_id/>
                    <input type="hidden" name="expected_version" value=record.version.to_string()/>
                    <input type="hidden" name="item_id" value=line.id/>
                    <input type="hidden" name="csrf" value=ctx.csrf/>
                    <label class="record-field">
                        <span>"Quantidade (separador decimal: ponto)"</span>
                        <input type="text" name="quantity" value=line.quantity.to_string() required=true/>
                    </label>
                    <label class="record-field">
                        <span>"Preço unitário (separador decimal: ponto)"</span>
                        <input type="text" name="price" value=line.price.to_string()
                            readonly=line.catalog_id.is_some() required=true/>
                    </label>
                    <label class="record-field">
                        <span>"Justificativa da alteração"</span>
                        <input type="text" name="reason" maxlength="240" required=true/>
                    </label>
                    <button type="submit" class="primary-button">"Salvar nova revisão do serviço"</button>
                </form>
            </section>
            <section class="foundation-card">
                <h2>"Copiar serviço para outra variação"</h2>
                <p>"O novo serviço mantém unidade, preço e referência de catálogo. A cópia cria outra revisão e preserva o cronograma existente."</p>
                <form action="/orcamentos/servicos/copiar" method="post" class="record-form">
                    <input type="hidden" name="operation" value="copy"/>
                    <input type="hidden" name="organization_id" value=copy_context.organization_id/>
                    <input type="hidden" name="context_version" value=copy_context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=copy_scope/>
                    <input type="hidden" name="expected_version" value=record.version.to_string()/>
                    <input type="hidden" name="item_id" value=copy_item/>
                    <input type="hidden" name="csrf" value=copy_csrf/>
                    <label class="record-field">
                        <span>"Identificador único do serviço copiado"</span>
                        <input type="text" name="new_item_id" maxlength="120" required=true/>
                    </label>
                    <label class="record-field">
                        <span>"Variação de destino"</span>
                        <select name="variation_id" required=true>{variation_options}</select>
                    </label>
                    <label class="record-field">
                        <span>"Justificativa da cópia"</span>
                        <input type="text" name="reason" maxlength="240" required=true/>
                    </label>
                    <button type="submit" class="secondary-button">"Criar cópia e salvar nova revisão"</button>
                </form>
            </section>
            <section class="foundation-card">
                <h2>"Mover serviço entre variações"</h2>
                <p>"O serviço mantém o mesmo identificador, preço, unidade e referência. Confirme a variação de destino; a alteração cria outra revisão sem mudar o cronograma."</p>
                <form action="/orcamentos/servicos/mover" method="post" class="record-form">
                    <input type="hidden" name="operation" value="move"/>
                    <input type="hidden" name="organization_id" value=move_context.organization_id/>
                    <input type="hidden" name="context_version" value=move_context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=move_scope/>
                    <input type="hidden" name="expected_version" value=record.version.to_string()/>
                    <input type="hidden" name="item_id" value=move_item/>
                    <input type="hidden" name="csrf" value=move_csrf/>
                    <label class="record-field">
                        <span>"Variação de destino do serviço"</span>
                        <select name="variation_id" required=true>{move_variation_options}</select>
                    </label>
                    <label class="record-field">
                        <span>"Justificativa da movimentação"</span>
                        <input type="text" name="reason" maxlength="240" required=true/>
                    </label>
                    <button type="submit" class="secondary-button">"Mover serviço e salvar nova revisão"</button>
                </form>
            </section>
            <section class="foundation-card">
                <h2>"Excluir serviço manual"</h2>
                <p>"A exclusão exige confirmação, motivo e versão atual. Serviços vinculados a preços históricos não podem ser excluídos aqui. Revisões anteriores e cronograma permanecem preservados."</p>
                <form action="/orcamentos/servicos/excluir" method="post" class="record-form">
                    <input type="hidden" name="operation" value="delete"/>
                    <input type="hidden" name="organization_id" value=delete_context.organization_id/>
                    <input type="hidden" name="context_version" value=delete_context.context_version.to_string()/>
                    <input type="hidden" name="scope_id" value=delete_scope/>
                    <input type="hidden" name="expected_version" value=record.version.to_string()/>
                    <input type="hidden" name="item_id" value=delete_item/>
                    <input type="hidden" name="csrf" value=delete_csrf/>
                    <label class="record-field">
                        <span>"Motivo da exclusão"</span>
                        <input type="text" name="reason" maxlength="240" required=true/>
                    </label>
                    <label class="record-field">
                        <input type="checkbox" name="confirm_delete" value="yes" required=true disabled=delete_disabled/>
                        <span>"Confirmo a exclusão deste serviço da nova revisão."</span>
                    </label>
                    <button type="submit" class="secondary-button" disabled=delete_disabled>
                        "Excluir serviço e registrar revisão"
                    </button>
                </form>
            </section>
            <section class="foundation-card">
                <h2>"Rastreabilidade"</h2>
                <p>"Preços vinculados a catálogos não são sobrescritos. A revisão aprovada permanece imutável."</p>
            </section>
        },
    )
}

pub fn preview(ctx: EstimatingContext, result: BudgetImportPreviewReply) -> String {
    let total = result.total;
    let detail_count = result.change_details.records.len();
    let omitted_count = result.change_details.omitted;
    let detail_rows = result
        .change_details
        .records
        .into_iter()
        .map(|change| {
            let kind = match change.kind {
                "included" => "Incluído",
                "removed" => "Removido",
                _ => "Alterado",
            };
            let fields = if change.fields.is_empty() {
                "Registro completo".to_owned()
            } else {
                format!("Campos: {}", change.fields.join(", "))
            };
            let display = |line: BudgetLine| {
                format!(
                    "{} · {} · {} {} × R$ {}",
                    line.code, line.description, line.quantity, line.unit, line.price
                )
            };
            let before = change.before.map(display);
            let after = change.after.map(display);
            view! {
                <li class="record-summary">
                    <strong>{format!("{} · serviço {}", kind, change.id)}</strong>
                    <p>{fields}</p>
                    {before.map(|text| view! { <p>{format!("Antes: {text}")}</p> })}
                    {after.map(|text| view! { <p>{format!("Depois: {text}")}</p> })}
                </li>
            }
        })
        .collect_view();
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
                <details>
                    <summary>{format!("Conferir {} diferenças por serviço",detail_count)}</summary>
                    <p>"Mostramos as primeiras diferenças por identificador. Valores anteriores permanecem na revisão histórica."</p>
                    <ol>{detail_rows}</ol>
                    {(omitted_count > 0).then(||view!{<p>{format!("Outros {omitted_count} serviços não exibidos nesta amostra. Os totais acima consideram todos.")}</p>})}
                </details>
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
