use canteiro_contracts::{
    calculations::{AllocationRequest, CalendarRequest, preview_allocation, preview_calendar},
    forms::{DecimalKind, FieldError, format_decimal_pt_br, parse_date_pt_br, parse_decimal_pt_br},
};
use canteiro_domain::{
    calendar::{Cadence, CalendarPolicy},
    decimal::CalculationPolicy,
};
use leptos::prelude::*;
use leptos_meta::{Stylesheet, Title};

fn shares(input: &str) -> Result<u32, String> {
    let value = input
        .parse::<u32>()
        .map_err(|_| "Informe um número inteiro de parcelas.".to_owned())?;
    if !(1..=100).contains(&value) {
        return Err("Informe entre 1 e 100 parcelas.".into());
    }
    Ok(value)
}

fn error(message: impl ToString) -> FieldError {
    FieldError {
        code: "input".into(),
        message: message.to_string(),
    }
}

fn date_display(value: impl ToString) -> String {
    let text = value.to_string();
    format!("{}/{}/{}", &text[8..10], &text[5..7], &text[..4])
}

#[component]
pub fn CalculationPage() -> impl IntoView {
    let ready = RwSignal::new(false);
    Effect::new(move |_| ready.set(true));
    view! {
        <Title text="Conferência de valores e prazos | Canteiro 360"/>
        <Stylesheet id="calculation-style" href="/pkg/prototype.css"/>
        <section class="prototype-shell calculation-page" data-calculation-ready=move || ready.get().to_string()>
            <a class="text-link" href="/">"Voltar ao início"</a>
            <p class="eyebrow">"Prévia de planejamento"</p>
            <h1>"Conferência de valores e prazos"</h1>
            <p class="intro-copy">"Confira as parcelas e as datas antes de consolidar uma versão. Os exemplos são fictícios; esta prévia não altera contratos ou pagamentos."</p>
            <div class="interaction-status" hidden=move || ready.get() data-ready=move || ready.get().to_string()>
                <div class="waiting-state" hidden=move || ready.get()>
                    <p role="status">"Preparando os campos para edição…"</p>
                    <div class="delayed-help" role="status">
                        <p>"Os campos ainda não estão disponíveis. Confira sua conexão e recarregue para editar."</p>
                        <a id="calculation-retry" class="retry-link" href="/conferencia">"Recarregar a conferência"</a>
                    </div>
                </div>
            </div>
            <noscript><p class="notice warning">"Os exemplos abaixo já foram calculados no servidor. Ative os scripts para alterar as entradas e recalcular nesta aba."</p></noscript>
            <div class="calculation-columns"><AllocationCard ready=ready/><CalendarCard ready=ready/></div>
        </section>
    }
}

#[component]
fn AllocationCard(ready: RwSignal<bool>) -> impl IntoView {
    let total = RwSignal::new("100,00".to_owned());
    let count = RwSignal::new("3".to_owned());
    let parsed_total = Memo::new(move |_| parse_decimal_pt_br(&total.get(), DecimalKind::Money));
    let parsed_count = Memo::new(move |_| shares(&count.get()));
    let preview = Memo::new(move |_| {
        let total = parsed_total.get()?.canonical.parse().map_err(error)?;
        preview_allocation(&AllocationRequest {
            policy: CalculationPolicy::default(),
            total,
            shares: parsed_count.get().map_err(error)?,
        })
    });
    view! {
        <section class="section-card" aria-labelledby="allocation-title">
            <p class="eyebrow">"Parcelas com pesos iguais"</p>
            <h2 id="allocation-title">"Distribuição do valor"</h2>
            <p>"O total é preservado até o último centavo. A ordem das parcelas define quem recebe o centavo de diferença."</p>
            <div class="form-grid">
                <div class="field"><label for="allocation-total">"Valor total (R$)"</label>
                    <input id="allocation-total" inputmode="decimal" maxlength="128" disabled=move || !ready.get() value=move || total.get() prop:value=move || total.get()
                        aria-describedby="allocation-total-help allocation-total-error" aria-invalid=move || parsed_total.get().is_err().to_string()
                        on:input=move |event| total.set(event_target_value(&event))/>
                    <p id="allocation-total-help">"Use vírgula para os centavos. Exemplo: 1.234,56."</p>
                    <p id="allocation-total-error" class="field-error" role="status">{move || parsed_total.get().err().map(|e| e.message).unwrap_or_default()}</p>
                </div>
                <div class="field"><label for="allocation-count">"Quantidade de parcelas"</label>
                    <input id="allocation-count" inputmode="numeric" maxlength="3" disabled=move || !ready.get() value=move || count.get() prop:value=move || count.get()
                        aria-describedby="allocation-count-help allocation-count-error" aria-invalid=move || parsed_count.get().is_err().to_string()
                        on:input=move |event| count.set(event_target_value(&event))/>
                    <p id="allocation-count-help">"De 1 a 100 parcelas. A primeira recebe prioridade no desempate."</p>
                    <p id="allocation-count-error" class="field-error" role="status">{move || parsed_count.get().err().unwrap_or_default()}</p>
                </div>
            </div>
            <div id="allocation-output">
                {move || match preview.get() {
                    Ok(result) => view! {
                        <div data-policy="C360-DECIMAL-1">
                            <p class="calculation-total" role="status"><strong>{format!("Total preservado: R$ {} em {} parcelas", format_decimal_pt_br(&result.total), result.shares.len())}</strong></p>
                            <div class="calculation-table" tabindex="0" role="region" aria-label="Parcelas da prévia">
                                <table><caption>"Distribuição monetária e percentual"</caption>
                                    <thead><tr><th scope="col">"Parcela"</th><th scope="col">"Valor (R$)"</th><th scope="col">"Percentual"</th><th scope="col">"Ajuste residual"</th></tr></thead>
                                    <tbody>{result.shares.into_iter().map(|share| view! {
                                        <tr><th scope="row">{share.position}</th><td class="numeric">{format_decimal_pt_br(&share.amount)}</td><td class="numeric">{format!("{}%", format_decimal_pt_br(&share.percent))}</td><td>{match (share.received_cent, share.received_percent_residual) {
                                            (true,true) => "Recebeu 1 centavo e 0,01 ponto percentual",
                                            (true,false) => "Recebeu 1 centavo",
                                            (false,true) => "Recebeu 0,01 ponto percentual",
                                            (false,false) => "Sem ajuste",
                                        }}</td></tr>
                                    }).collect_view()}</tbody>
                                </table>
                            </div>
                            <p class="calculation-policy">"Regra de cálculo versão 1: maiores restos, com desempate pela ordem estável das parcelas. Os percentuais fecham 100,00% e não são somados ao valor."</p>
                        </div>
                    }.into_any(),
                    Err(_) => view! { <p class="notice warning">"Revise os campos indicados para atualizar a prévia. As entradas foram preservadas."</p> }.into_any(),
                }}
            </div>
        </section>
    }
}

#[component]
fn CalendarCard(ready: RwSignal<bool>) -> impl IntoView {
    let start = RwSignal::new("31/01/2027".to_owned());
    let end = RwSignal::new("31/03/2027".to_owned());
    let civil = RwSignal::new(true);
    let parsed_start = Memo::new(move |_| parse_date_pt_br(&start.get()));
    let parsed_end = Memo::new(move |_| parse_date_pt_br(&end.get()));
    let preview = Memo::new(move |_| {
        preview_calendar(&CalendarRequest {
            policy: CalculationPolicy::default(),
            calendar_policy: if civil.get() {
                CalendarPolicy::Civil
            } else {
                CalendarPolicy::FixedDays
            },
            cadence: Cadence::Monthly,
            anchor: parsed_start.get()?.canonical.parse().map_err(error)?,
            formal_end: parsed_end.get()?.canonical.parse().map_err(error)?,
        })
    });
    view! {
        <section class="section-card" aria-labelledby="calendar-title">
            <p class="eyebrow">"Cronograma mensal"</p>
            <h2 id="calendar-title">"Datas e duração dos períodos"</h2>
            <p>"O calendário civil mantém a data de referência. O último período respeita o término informado."</p>
            <div class="form-grid">
                <div class="field"><label for="calendar-start">"Data de referência"</label>
                    <input id="calendar-start" maxlength="10" disabled=move || !ready.get() value=move || start.get() prop:value=move || start.get() aria-describedby="calendar-start-help calendar-start-error"
                        aria-invalid=move || parsed_start.get().is_err().to_string() on:input=move |event| start.set(event_target_value(&event))/>
                    <p id="calendar-start-help">"Dia/mês/ano. Exemplo: 31/01/2027."</p>
                    <p id="calendar-start-error" class="field-error" role="status">{move || parsed_start.get().err().map(|e| e.message).unwrap_or_default()}</p>
                </div>
                <div class="field"><label for="calendar-end">"Término informado"</label>
                    <input id="calendar-end" maxlength="10" disabled=move || !ready.get() value=move || end.get() prop:value=move || end.get() aria-describedby="calendar-end-help calendar-end-error"
                        aria-invalid=move || parsed_end.get().is_err().to_string() on:input=move |event| end.set(event_target_value(&event))/>
                    <p id="calendar-end-help">"A data final do instrumento é preservada."</p>
                    <p id="calendar-end-error" class="field-error" role="status">{move || parsed_end.get().err().map(|e| e.message).unwrap_or_default()}</p>
                </div>
                <div class="field calculation-span"><label for="calendar-policy">"Regra do cronograma"</label>
                    <select id="calendar-policy" disabled=move || !ready.get() prop:value=move || if civil.get() { "civil" } else { "fixed" }
                        on:change=move |event| civil.set(event_target_value(&event) == "civil")>
                        <option value="civil">"Calendário civil, com referência original"</option>
                        <option value="fixed">"Períodos fixos de 30 dias"</option>
                    </select>
                    <p>"A escolha define a programação das parcelas. Ela não altera o prazo de execução ou a vigência do contrato."</p>
                </div>
            </div>
            <div id="calendar-output">
                {move || match preview.get() {
                    Ok(result) => view! {
                        <div data-policy="C360-DECIMAL-1">
                            <p class="calculation-total" role="status"><strong>{format!("{} dias corridos até {}", result.duration_days, date_display(result.formal_end))}</strong></p>
                            <div class="calculation-table" tabindex="0" role="region" aria-label="Períodos do cronograma">
                                <table><caption>"Períodos contados a partir da referência"</caption><thead><tr><th scope="col">"Período"</th><th scope="col">"Término"</th><th scope="col">"Dias"</th><th scope="col">"Condição"</th></tr></thead>
                                    <tbody>{result.periods.into_iter().enumerate().map(|(index,p)| view! {
                                        <tr><th scope="row">{index+1}</th><td>{date_display(p.end)}</td><td class="numeric">{p.days}</td><td>{if p.residual { "Parcela residual" } else { "Período completo" }}</td></tr>
                                    }).collect_view()}</tbody>
                                </table>
                            </div>
                            <p class="calculation-policy">{if civil.get() { "Regra versão 1: meses civis. Quando o dia não existe, usa-se o último dia daquele mês, mantendo a referência original para o seguinte." } else { "Regra versão 1: períodos fixos de 30 dias. O período residual termina exatamente na data final informada." }}</p>
                        </div>
                    }.into_any(),
                    Err(error) => view! { <p class="notice warning" role="status">{error.message}</p> }.into_any(),
                }}
            </div>
        </section>
    }
}
