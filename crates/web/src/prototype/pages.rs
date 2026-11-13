use super::{DemoContext, components::*, model::*};
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_query_map};

#[component]
pub(super) fn Dashboard() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let company = context.company;
    view! {
        <Heading title="O que precisa da sua atenção" description="Encontre a próxima decisão e acompanhe cada etapa da obra."/>
        <div class="two-column">
            <section class="section-card" aria-labelledby="next-action-title">
                <Badge tone="neutral">{move || if context.data().approved {"Simulação de decisão concluída"} else {"1 decisão aguardando revisão"}}</Badge>
                <h2 id="next-action-title">{move || if context.data().approved {"Consultar a decisão demonstrativa"} else {"Conferir a medição 02"}}</h2>
                <p>{move || if context.data().approved {"A simulação da medição 02 foi concluída nesta aba. Consulte o resultado ou avance no rascunho da próxima medição. Nenhum ato real foi gravado."} else {"A conferência técnica terminou. Revise os valores, os documentos e a responsabilidade antes de decidir."}}</p>
                <p class="metric-value">{move || money(company.get().net_cents())}</p>
                <p>{move || if context.data().approved {"Valor líquido do exemplo. A simulação não representa pagamento."} else {"Valor líquido em análise. Ainda não representa pagamento."}}</p>
                <a class="primary-button" href=move || company.get().url(Task::Approvals)>{move || if context.data().approved {"Consultar simulação concluída"} else {"Revisar aprovação"}}</a>
            </section>
            <section class="section-card" aria-labelledby="pending-title">
                <h2 id="pending-title">"Pendências da obra"</h2>
                <ul class="action-list">
                    <li><strong>"Equipe de medição"</strong><p>"Falta uma comissão válida para concluir a próxima medição."</p><a class="text-link" href=move || company.get().url(Task::Measurement)>"Ver impedimento da medição"</a></li>
                    <li><strong>"Cadastro da obra"</strong><p>"Dados complementares pendentes. A consulta de documentos já está disponível."</p><a class="text-link" href=move || company.get().url(Task::Documents)>"Consultar documentos"</a></li>
                </ul>
            </section>
        </div>
        <section aria-labelledby="financial-title">
            <h2 id="financial-title">"Valores em etapas diferentes"</h2>
            <p>"Cada valor mostra um estágio do mesmo contrato. Eles não devem ser somados entre si."</p>
            <div class="metric-grid">
                {move || view! {
                    <Metric label="Contratado" value=money(company.get().total_cents()) help="Valor vigente do contrato."/>
                    <Metric label="Medido" value=money(company.get().measured_cents()) help="Execução medida até outubro, incluindo a medição em análise."/>
                    <Metric label="Faturado" value=money(company.get().previous_cents()) help="Documentos de cobrança emitidos."/>
                    <Metric label="Pago pelo contratante" value=money(company.get().paid_cents()) help="Pagamentos registrados no exemplo."/>
                }}
            </div>
        </section>
        <section class="section-card" aria-labelledby="steps-title">
            <h2 id="steps-title">"Etapas e pendências"</h2>
            <p>"Abra a tarefa de que precisa. A ordem abaixo não obriga a percorrer todos os menus."</p>
            <ol class="step-list">
                <li><span class="step-link"><strong>"1. Cadastro"</strong><Badge tone="warning">"Pendente"</Badge><span>"Complementação pela equipe responsável"</span></span></li>
                <li><a class="step-link" href=move || company.get().url(Task::Budget)><strong>"2. Orçamento"</strong><Badge tone="success">"Revisão disponível"</Badge></a></li>
                <li><a class="step-link" href=move || company.get().url(Task::Measurement)><strong>"3. Medição"</strong><Badge tone="warning">"Comissão pendente"</Badge></a></li>
                <li><a class="step-link" href=move || company.get().url(Task::Approvals)><strong>"4. Aprovação"</strong><Badge tone="neutral">{move || if context.data().approved {"Simulação concluída"} else {"1 em análise"}}</Badge></a></li>
                <li><a class="step-link" href=move || company.get().url(Task::Documents)><strong>"5. Documentos"</strong><Badge tone="success">"Consulta disponível"</Badge></a></li>
            </ol>
        </section>
    }
}

#[component]
pub(super) fn Budget() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let company = context.company;
    let query = use_query_map();
    let filter = Memo::new(move |_| query.read().get("busca").unwrap_or_default());
    let item_code = Memo::new(move |_| query.read().get("item"));
    let input = RwSignal::new(filter.get_untracked());
    Effect::new(move |_| input.set(filter.get()));
    let navigate = use_navigate();
    view! {
        <Heading title="Orçamento da obra" description="Confira serviços, unidades e totais da revisão vigente."/>
        {move || if let Some(code) = item_code.get() {
            if let Some(item) = company.get().items().into_iter().find(|item| item.code == code) {
                view! {
                    <section class="section-card" aria-labelledby="budget-detail-title">
                        <a id="back-to-budget" class="text-link" href=format!("{}?busca={}#budget-item-{}", company.get().url(Task::Budget), encode_query(&filter.get()), item.code)>"Voltar aos serviços filtrados"</a>
                        <Badge tone="neutral">"Serviço do orçamento"</Badge>
                        <h2 id="budget-detail-title">{format!("{} · {}", item.code, item.name)}</h2>
                        <dl class="context-grid">
                            <div class="context-item"><dt>"Quantidade"</dt><dd>{format!("{} {}", item.quantity, item.unit)}</dd></div>
                            <div class="context-item"><dt>"Preço unitário"</dt><dd>{money(item.unit_cents)}</dd></div>
                            <div class="context-item"><dt>"Total do serviço"</dt><dd>{money(item.total_cents())}</dd></div>
                        </dl>
                        <Notice tone="neutral" title="Critério do exemplo"><p>"A quantidade multiplicada pelo preço unitário forma o total. Esta consulta não altera o orçamento."</p></Notice>
                        <details class="version-comparison"><summary>"Comparar versão e histórico"</summary><p>{format!("Revisão {}: serviço conferido e disponível para consulta.", company.get().version())}</p><p>"Histórico fictício: inclusão em 20/10/2026 e conferência técnica em 25/10/2026. Nenhuma alteração foi feita nesta demonstração."</p></details>
                    </section>
                }.into_any()
            } else {
                view! { <section class="empty-state"><h2>"Serviço não encontrado"</h2><p>"O código informado não pertence a este orçamento."</p><a class="primary-button" href=company.get().url(Task::Budget)>"Voltar ao orçamento"</a></section> }.into_any()
            }
        } else {
            view! {
                <section class="section-card" aria-labelledby="budget-list-title">
                    <h2 id="budget-list-title">"Serviços contratados"</h2>
                    <form class="prototype-toolbar" method="get" action=move || company.get().url(Task::Budget)
                        on:submit={let navigate = navigate.clone(); move |event| {
                            event.prevent_default();
                            navigate(&format!("{}?busca={}", company.get_untracked().url(Task::Budget), encode_query(&input.get_untracked())), Default::default());
                        }}>
                        <div class="field"><label for="budget-filter">"Buscar serviço"</label><input id="budget-filter" name="busca" type="search" value=move || filter.get() prop:value=move || input.get() on:input=move |event| input.set(event_target_value(&event)) placeholder="Nome ou código do serviço"/></div>
                        <button id="budget-submit" type="submit" class="secondary-button">"Buscar serviços"</button>
                        <a class="text-link" href=move || company.get().url(Task::Budget)>"Limpar busca"</a>
                    </form>
                    {move || {
                        let needle = filter.get().trim().to_lowercase();
                        let items = company.get().items().into_iter().filter(|item| item.name.to_lowercase().contains(&needle) || item.code.contains(&needle)).collect::<Vec<_>>();
                        if items.is_empty() {
                            view! { <div class="empty-state" role="status"><h3>"Nenhum serviço encontrado"</h3><p>"Tente outro nome ou limpe a busca. O orçamento original foi preservado."</p></div> }.into_any()
                        } else {
                            let subtotal = items.iter().map(|item| item.total_cents()).sum::<u64>();
                            view! {
                                <p class="field-help" id="budget-scroll-help">"No celular, deslize apenas a tabela para conferir as colunas. Pelo teclado, use Tab para entrar na tabela e as setas para rolar."</p>
                                <div class="table-scroll" tabindex="0" role="region" aria-label="Tabela de serviços" aria-describedby="budget-scroll-help">
                                    <table class="data-table">
                                        <caption>{format!("{} serviço(s) no filtro · valores em reais", items.len())}</caption>
                                        <thead><tr><th scope="col">"Serviço"</th><th scope="col">"Unidade"</th><th scope="col" class="numeric">"Quantidade"</th><th scope="col" class="numeric">"Preço unitário"</th><th scope="col" class="numeric">"Total"</th></tr></thead>
                                        <tbody>{items.into_iter().map(|item| view! {
                                            <tr id=format!("budget-item-{}",item.code) tabindex="-1"><th scope="row"><a class="row-link" href=format!("{}?busca={}&item={}",company.get().url(Task::Budget),encode_query(&filter.get()),item.code)>{format!("{} · {}", item.code,item.name)}</a></th><td>{item.unit}</td><td class="numeric">{item.quantity}</td><td class="numeric">{money(item.unit_cents)}</td><td class="numeric">{money(item.total_cents())}</td></tr>
                                        }).collect_view()}</tbody>
                                        <tfoot><tr><th scope="row" colspan="4">"Subtotal dos serviços exibidos"</th><td class="numeric">{money(subtotal)}</td></tr></tfoot>
                                    </table>
                                </div>
                            }.into_any()
                        }
                    }}
                    <div class="totals-panel"><span>"Total do orçamento completo"</span><strong>{move || money(company.get().total_cents())}</strong><p>"O filtro não altera este total."</p></div>
                </section>
            }.into_any()
        }}
    }
}

#[component]
pub(super) fn Measurement() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let company = context.company;
    let error = RwSignal::new(None::<(&'static str, &'static str)>);
    let edit_note = move |text: String| {
        context.update(|state| {
            state.note = text;
            state.checked = false;
        });
        error.set(None);
    };
    view! {
        <Heading title="Preparar medição" description="Organize o período e a descrição da execução antes da conferência."/>
        <Notice tone="warning" title="Conclusão bloqueada: falta uma comissão válida">
            <p id="commission-reason">"Peça ao responsável da obra que cadastre uma comissão com vigência para o período. Você pode preparar o rascunho e consultar documentos enquanto essa pendência é resolvida."</p>
        </Notice>
        <div class="two-column">
            <section class="section-card" aria-labelledby="measurement-title">
                <div class="page-heading"><h2 id="measurement-title">"Rascunho da medição 03"</h2><Badge tone="warning">"Não enviado"</Badge></div>
                <p>"Exemplo fictício. O rascunho fica apenas nesta aba e não é gravado no servidor. Recarregar ou fechar a página descarta o conteúdo."</p>
                <form novalidate on:submit=move |event| {
                    event.prevent_default();
                    if let Some(problem) = context.data().validation() {
                        error.set(Some(problem));
                        focus(problem.0);
                    } else {
                        error.set(None);
                        context.update(|state| state.checked = true);
                    }
                }>
                    <div class="form-grid">
                        <div class="field"><label for="period-start">"Início do período (obrigatório)"</label>
                            <input id="period-start" name="inicio" type="date" required disabled=move || !context.ready.get()
                                value=move || context.data().start prop:value=move || context.data().start
                                aria-invalid=move || error.get().is_some_and(|(field,_)|field == "period-start").to_string()
                                aria-describedby="period-help period-summary measurement-error"
                                on:input=move |event| {context.update(|state|{state.start=event_target_value(&event);state.checked=false;});error.set(None);}/>
                        </div>
                        <div class="field"><label for="period-end">"Fim do período (obrigatório)"</label>
                            <input id="period-end" name="fim" type="date" required disabled=move || !context.ready.get()
                                value=move || context.data().end prop:value=move || context.data().end
                                aria-invalid=move || error.get().is_some_and(|(field,_)|field == "period-end").to_string()
                                aria-describedby="period-help period-summary measurement-error"
                                on:input=move |event| {context.update(|state|{state.end=event_target_value(&event);state.checked=false;});error.set(None);}/>
                        </div>
                    </div>
                    <p id="period-help" class="field-help">"Escolha as datas no calendário ou preencha pelo teclado. O fim não pode anteceder o início."</p>
                    <p id="period-summary" class="field-help" role="status" aria-live="polite">{move || context.data().period_description()}</p>
                    <div class="field"><label for="measurement-note">"Descrição da execução (obrigatório)"</label>
                        <textarea id="measurement-note" name="descricao" rows="4" maxlength="2000" required disabled=move || !context.ready.get()
                            prop:value=move || context.data().note
                            aria-invalid=move || error.get().is_some_and(|(field,_)|field == "measurement-note").to_string()
                            aria-describedby="measurement-help measurement-error"
                            on:input=move |event| edit_note(event_target_value(&event))></textarea>
                        <p id="measurement-help" class="field-help">"Descreva o serviço e o local, com pelo menos 10 caracteres. Use apenas informações fictícias."</p>
                    </div>
                    <p id="measurement-error" class="field-error" role="alert" hidden=move || error.get().is_none()>{move || error.get().map(|(_,message)|message).unwrap_or_default()}</p>
                    <div class="page-actions"><button id="save-draft" type="submit" class="primary-button" disabled=move || !context.ready.get()>"Conferir rascunho"</button><a class="text-link" href=move || company.get().url(Task::Documents)>"Consultar documentos"</a></div>
                    <p id="draft-feedback" role="status" aria-live="polite" hidden=move || !context.data().checked>"Rascunho conferido nesta demonstração. Nenhum dado foi gravado no servidor. A comissão continua pendente."</p>
                </form>
            </section>
            <section class="section-card" aria-labelledby="measurement-summary-title">
                <h2 id="measurement-summary-title">"Prévia ilustrativa de valores"</h2><p>"Valores fixos para conhecer a apresentação. Alterar a descrição ou o período não recalcula esta prévia."</p>
                <dl class="totals-panel">
                    <div><dt>"Serviços no período"</dt><dd>{move || money(company.get().gross_cents())}</dd></div>
                    <div><dt>"Retenções previstas"</dt><dd>{move || money(company.get().retention_cents())}</dd></div>
                    <div><dt>"Líquido da prévia"</dt><dd><strong>{move || money(company.get().net_cents())}</strong></dd></div>
                </dl>
                <p>"Preparar a medição não emite fatura nem registra pagamento."</p>
                <button id="complete-measurement" type="button" class="secondary-button" disabled aria-describedby="commission-reason">"Concluir medição indisponível"</button>
                <details><summary>"Por que não posso concluir?"</summary><p>"A comissão precisa estar válida para conferir a execução. O botão permanece indisponível neste exemplo, mesmo depois de conferir o rascunho."</p></details>
            </section>
        </div>
    }
}

#[component]
pub(super) fn Approvals() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let company = context.company;
    view! {
        <Heading title="Aprovações da obra" description="Revise o que muda e quem decide antes de confirmar uma ação."/>
        <section class="section-card" aria-labelledby="approval-title">
            <div class="page-heading"><h2 id="approval-title">"Medição 02"</h2><Badge tone="neutral">{move || if context.data().approved {"Simulação concluída"}else{"Aguardando decisão"}}</Badge></div>
            <p>"Período de 16/10/2026 a 31/10/2026 · revisão 01 da medição"</p>
            <div class="metric-grid">
                {move || view! {
                    <Metric label="Serviços medidos" value=money(company.get().gross_cents()) help="Execução submetida à conferência."/>
                    <Metric label="Retenções" value=money(company.get().retention_cents()) help="Valor retido conforme o exemplo contratual."/>
                    <Metric label="Líquido em análise" value=money(company.get().net_cents()) help="A aprovação não representa pagamento."/>
                }}
            </div>
            <dl class="context-grid"><div class="context-item"><dt>"Autor"</dt><dd>"Marina Costa, execução"</dd></div><div class="context-item"><dt>"Conferência"</dt><dd>"Rafael Lima, equipe técnica"</dd></div><div class="context-item"><dt>"Decisão"</dt><dd>"Ana Ribeiro, contratante"</dd></div></dl>
            <p>"Pessoas e vínculos fictícios, usados apenas para mostrar a separação de responsabilidades."</p>
            <details class="version-comparison"><summary>"Conferir saldo, prazo e evidências"</summary>
                <dl class="totals-panel"><div><dt>"Valor contratual vigente"</dt><dd>{move || money(company.get().total_cents())}</dd></div><div><dt>"Medido anteriormente"</dt><dd>{move || money(company.get().previous_cents())}</dd></div><div><dt>"Acréscimo medido neste período"</dt><dd>{move || money(company.get().gross_cents())}</dd></div><div><dt>"Acumulado incluindo esta medição"</dt><dd>{move || money(company.get().measured_cents())}</dd></div><div><dt>"Saldo contratual a medir"</dt><dd>{move || money(company.get().total_cents()-company.get().measured_cents())}</dd></div></dl>
                <p>"Esta aprovação não altera o valor nem o prazo contratual. Depois da decisão, a etapa seguinte é preparar a cobrança, conforme as regras do contrato."</p>
                <a class="text-link" href=move || company.get().url(Task::Documents)>"Consultar evidências e versões"</a>
            </details>
            <div class="page-actions"><button id="review-approval" class="primary-button" type="button" disabled=move || !context.ready.get() || context.data().approved
                on:click=move |_| show_dialog("approval-dialog", "cancel-approval")>"Revisar aprovação da medição"</button></div>
            <div id="approval-feedback" tabindex="-1" role="status" aria-live="polite" class="notice success" hidden=move || !context.data().approved>
                <strong>"Simulação concluída nesta aba"</strong><p>"Nenhuma aprovação, fatura ou pagamento foi gravado. O exemplo serve para conferir o percurso da decisão."</p>
            </div>
        </section>
        <section class="section-card" aria-labelledby="history-title"><h2 id="history-title">"Histórico do exemplo"</h2><ol class="history-list"><li><time datetime="2026-11-02">"02/11/2026"</time><strong>"Execução apresentada"</strong><p>"Marina Costa incluiu a medição e os documentos."</p></li><li><time datetime="2026-11-03">"03/11/2026"</time><strong>"Conferência técnica concluída"</strong><p>"Rafael Lima encaminhou a versão 01 para decisão."</p></li></ol></section>
        <dialog id="approval-dialog" class="prototype-dialog" aria-labelledby="approval-dialog-title" aria-describedby="approval-consequence"
            on:keydown=move |event| trap_dialog_tab(event, "approval-dialog", ["cancel-approval", "confirm-approval"])
            on:close=move |_| focus(if context.data().approved {"approval-feedback"} else {"review-approval"})>
            <h2 id="approval-dialog-title">"Revise antes de simular a aprovação"</h2>
            <dl class="context-grid"><div class="context-item"><dt>"Empresa"</dt><dd>{move || company.get().name()}</dd></div><div class="context-item"><dt>"Obra"</dt><dd>{move || company.get().work()}</dd></div><div class="context-item"><dt>"Contrato"</dt><dd>{move || company.get().contract()}</dd></div><div class="context-item"><dt>"Versão"</dt><dd>"Medição 02, revisão 01"</dd></div></dl>
            <p>"Período: 16/10/2026 a 31/10/2026. Decisão atribuída a Ana Ribeiro no exemplo."</p>
            <dl class="totals-panel"><div><dt>"Bruto"</dt><dd>{move || money(company.get().gross_cents())}</dd></div><div><dt>"Retenções"</dt><dd>{move || money(company.get().retention_cents())}</dd></div><div><dt>"Líquido"</dt><dd><strong>{move || money(company.get().net_cents())}</strong></dd></div></dl>
            <p id="approval-consequence">"Esta ação apenas muda o estado do exemplo nesta aba. No fluxo real, a aprovação autoriza seguir para cobrança; não comprova pagamento. Nenhum dado será enviado ao servidor nesta demonstração."</p>
            <div class="dialog-actions"><button id="cancel-approval" type="button" class="secondary-button" on:click=move |_| close_dialog("approval-dialog")>"Voltar à revisão"</button><button id="confirm-approval" type="button" class="primary-button" disabled=move || context.data().approved
                on:click=move |_| {context.update(|state|state.approved=true);close_dialog("approval-dialog");}>
                {move || format!("Simular aprovação de {}",money(company.get().net_cents()))}
            </button></div>
        </dialog>
    }
}

#[component]
pub(super) fn Documents() -> impl IntoView {
    let context = expect_context::<DemoContext>();
    let company = context.company;
    view! {
        <Heading title="Documentos e versões" description="Consulte a referência disponível sem depender da ordem dos menus."/>
        <Notice tone="neutral" title="Consulta disponível com cadastro pendente"><p>"A pendência de cadastro não impede abrir estes documentos. Atos que dependem de uma comissão válida continuam bloqueados na medição."</p></Notice>
        <section class="section-card" aria-labelledby="documents-title">
            <h2 id="documents-title">"Referências da obra"</h2>
            <article class="document-card"><div><Badge tone="success">"Versão vigente do exemplo"</Badge><h3>"Memorial de execução"</h3><p>{move || format!("{} · {} · revisão {}",company.get().work(),company.get().contract(),company.get().version())}</p><p>"Disponibilizado em 25/10/2026 pela equipe técnica fictícia."</p></div>
                <details id="document-preview"><summary>"Abrir prévia do memorial"</summary><div class="document-preview"><h3>"Memorial demonstrativo"</h3><p>{move || format!("Objeto: serviços contratados para {}.",company.get().work())}</p><p>"A execução deve corresponder aos serviços e unidades da revisão de orçamento indicada. Esta prévia é ilustrativa e não constitui documento técnico para execução de obra."</p><p>"Conteúdo disponível em texto para leitura, seleção e consulta por teclado."</p></div></details>
            </article>
            <article class="document-card"><div><Badge tone="neutral">"Evidência da medição 02"</Badge><h3>"Registro da conferência técnica"</h3><p>"Revisão 01 · 03/11/2026 · Rafael Lima, conferente fictício"</p></div>
                <details><summary>"Abrir registro de conferência"</summary><p>"Conferência demonstrativa dos serviços medidos entre 16/10/2026 e 31/10/2026. Retenções e valor líquido estão detalhados na página de aprovação."</p><a class="text-link" href=move || company.get().url(Task::Approvals)>"Revisar valores da medição"</a></details>
            </article>
            <details class="version-comparison"><summary>"Como identificar a versão de referência"</summary><p>"Use a indicação textual de versão vigente e confira empresa, obra e contrato. Uma versão anterior não deve ser confundida com a referência atual."</p><p>"Este protótipo não recebe arquivos, não aprova projetos e não altera documentos."</p></details>
        </section>
    }
}
