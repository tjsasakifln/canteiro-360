//! Navegação nativa, sem scripts e sem executar ações ao abrir links.
use crate::projects::{hidden, shell};
use canteiro_contracts::notifications::*;
use leptos::prelude::*;
pub struct NoticePreferencesDraft {
    pub mute_optional_email: bool,
    pub group_by_day: bool,
    pub reminder_days: String,
}

pub fn page(ctx: NoticesContext, message: Option<String>, key: String) -> String {
    page_with_draft(ctx, message, key, None)
}

pub fn page_with_draft(
    ctx: NoticesContext,
    message: Option<String>,
    key: String,
    draft: Option<NoticePreferencesDraft>,
) -> String {
    page_with_pagination(ctx, message, key, draft, 1, false)
}

pub fn page_with_pagination(
    ctx: NoticesContext,
    message: Option<String>,
    key: String,
    draft: Option<NoticePreferencesDraft>,
    page: u32,
    has_more: bool,
) -> String {
    let grouped = ctx.preferences.settings.group_by_day;
    let previous = page.checked_sub(1).filter(|value| *value > 0);
    let next = page.checked_add(1).filter(|_| has_more);
    let mut previous_day = None;
    let notice_rows=ctx.notices.clone().into_iter().map(|row|{
        let heading = if grouped {
            day_heading(&mut previous_day, &row)
        } else {
            None
        };
        let kind=match row.kind{
            NoticeKind::TechnicalObligation=>"Documento técnico",
            NoticeKind::Invitation=>"Convite",
            NoticeKind::Approval=>"Aprovação",
            NoticeKind::Deadline=>"Prazo",
            NoticeKind::AssignedTask=>"Atividade",
        };
        let state=match row.state{
            NoticeState::Pending=>"Pendente",
            NoticeState::Communicated=>"Comunicado",
            NoticeState::Failed=>"Entrega falhou",
            NoticeState::Resolved=>"Resolvido",
        };
        view!{
            {heading.map(|heading| view!{<p class="eyebrow" data-notice-day="true">{heading}</p>})}
            <article class="record-summary">
                <h3>{kind}</h3>
                <p class="record-status">{state}</p>
                <p>{format!("Prazo: {}",row.due_on.unwrap_or_else(||"não informado".into()))}</p>
                <p>{format!("Tentativas de comunicação: {}",row.attempts)}</p>
                <a href=row.destination>"Consultar o registro"</a>
            </article>
        }
    }).collect_view();
    let prefs = ctx.preferences.settings;
    let version = ctx
        .preferences
        .version
        .map(|v| v.to_string())
        .unwrap_or_default();
    let draft = draft.unwrap_or_else(|| NoticePreferencesDraft {
        mute_optional_email: prefs.mute_optional_email,
        group_by_day: prefs.group_by_day,
        reminder_days: prefs
            .reminder_days
            .into_iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(","),
    });
    shell(
        "Pendências e notificações",
        view! {
            <section class="record-heading">
                <p class="eyebrow">{ctx.organization_name}</p>
                <h1>"Pendências e notificações"</h1>
                <p>"Acompanhe avisos e abra cada registro para consultar os detalhes. Abrir o link não aprova nem confirma uma decisão."</p>
            </section>
            {message.map(|m|view!{<section class="record-notice" role="alert"><p>{m}</p></section>})}
            <div class="record-layout">
                <section class="foundation-card">
                    <h2>"Caixa de entrada"</h2>
                    {ctx.notices.is_empty().then(||view!{<p>{
                        if page == 1 {
                            "Nenhuma pendência disponível para seus acessos nesta empresa."
                        } else {
                            "Nenhuma pendência nesta página. Volte para consultar os registros atuais."
                        }
                    }</p>})}
                    {notice_rows}
                    {(previous.is_some() || next.is_some()).then(|| view!{
                        <nav aria-label="Navegação das pendências" class="record-form">
                            <p>{format!("Página {page}")}</p>
                            {previous.map(|number| view!{
                                <a href=format!("/pendencias?page={number}") rel="prev" class="secondary-button">
                                    "Página anterior de pendências"
                                </a>
                            })}
                            {next.map(|number| view!{
                                <a href=format!("/pendencias?page={number}") rel="next" class="secondary-button">
                                    "Próxima página de pendências"
                                </a>
                            })}
                        </nav>
                    })}
                </section>
                <section class="foundation-card">
                    <h2>"Preferências de comunicação"</h2>
                    <p>"Avisos essenciais ficam ativos. Desativar e-mails opcionais não retira as pendências da caixa de entrada."</p>
                    {ctx.context.map(|scope|view!{
                        <form method="post" action="/pendencias/preferencias" class="record-form">
                            {hidden(&scope,&ctx.csrf,&key)}
                            <input type="hidden" name="expected_version" value=version.clone()/>
                            <label class="record-check">
                                <input type="checkbox" name="mute_optional_email" value="true" checked=draft.mute_optional_email/>
                                "Não enviar e-mails opcionais"
                            </label>
                            <label class="record-check">
                                <input type="checkbox" name="group_by_day" value="true" checked=draft.group_by_day/>
                                "Agrupar pendências por dia de vencimento"
                            </label>
                            <label class="record-field">
                                <span>"Avisar com quantos dias de antecedência (separados por vírgula)"</span>
                                <input name="reminder_days" value=draft.reminder_days maxlength="80"/>
                            </label>
                            <p>"Opções: 0, 1, 2, 3, 5, 7, 10, 14, 21, 30, 45, 60, 90, 180 e 365."</p>
                            <button type="submit" class="primary-button">"Salvar preferências"</button>
                        </form>
                    })}
                </section>
            </div>
        },
    )
}

/// A consulta entrega pendências primeiro, por vencimento, e resolvidas ao final.
/// O agrupamento mantém essa prioridade, inclusive quando não há prazo definido.
fn day_heading(previous: &mut Option<(bool, Option<String>)>, row: &NoticeEntry) -> Option<String> {
    let resolved = row.state == NoticeState::Resolved;
    let key = (resolved, row.due_on.clone());
    if previous.as_ref() == Some(&key) {
        return None;
    }
    let heading = match (resolved, row.due_on.as_deref()) {
        (false, Some(day)) => format!("Vencimento: {day}"),
        (false, None) => "Pendências sem prazo informado".into(),
        (true, Some(day)) => format!("Resolvidas, vencimento: {day}"),
        (true, None) => "Resolvidas sem prazo informado".into(),
    };
    *previous = Some(key);
    Some(heading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping_keeps_due_dates_and_resolved_notices_separate() {
        let mut previous = None;
        let mut row = NoticeEntry {
            event_id: String::new(),
            kind: NoticeKind::TechnicalObligation,
            scope_id: String::new(),
            due_on: Some("2027-01-01".into()),
            state: NoticeState::Pending,
            attempts: 0,
            created_at: String::new(),
            destination: "/obrigacoes".into(),
        };
        assert_eq!(
            day_heading(&mut previous, &row).as_deref(),
            Some("Vencimento: 2027-01-01")
        );
        row.state = NoticeState::Failed;
        assert_eq!(day_heading(&mut previous, &row), None);
        row.due_on = None;
        assert_eq!(
            day_heading(&mut previous, &row).as_deref(),
            Some("Pendências sem prazo informado")
        );
        row.state = NoticeState::Resolved;
        row.due_on = Some("2027-01-01".into());
        assert_eq!(
            day_heading(&mut previous, &row).as_deref(),
            Some("Resolvidas, vencimento: 2027-01-01")
        );
        row.due_on = None;
        assert_eq!(
            day_heading(&mut previous, &row).as_deref(),
            Some("Resolvidas sem prazo informado")
        );
    }
}
