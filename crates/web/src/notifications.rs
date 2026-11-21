//! Navegação nativa, sem scripts e sem executar ações ao abrir links.
use crate::projects::{hidden, shell};
use canteiro_contracts::notifications::*;
use leptos::prelude::*;
pub fn page(ctx:NoticesContext,message:Option<String>,key:String)->String{
    let notice_rows=ctx.notices.clone().into_iter().map(|row|{
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
            <article class="record-summary">
                <h3>{kind}</h3>
                <p class="record-status">{state}</p>
                <p>{format!("Prazo: {}",row.due_on.unwrap_or_else(||"não informado".into()))}</p>
                <p>{format!("Tentativas de comunicação: {}",row.attempts)}</p>
                <a href=row.destination>"Consultar o registro"</a>
            </article>
        }
    }).collect_view();
    let prefs=ctx.preferences.settings;
    let version=ctx.preferences.version.map(|v|v.to_string()).unwrap_or_default();
    let days=prefs.reminder_days.into_iter().map(|v|v.to_string())
        .collect::<Vec<_>>().join(",");
    shell("Pendências e notificações",
        view!{
            <section class="record-heading">
                <p class="eyebrow">{ctx.organization_name}</p>
                <h1>"Pendências e notificações"</h1>
                <p>"Acompanhe avisos e abra cada registro para consultar os detalhes. Abrir o link não aprova nem confirma uma decisão."</p>
            </section>
            {message.map(|m|view!{<section class="record-notice" role="alert"><p>{m}</p></section>})}
            <div class="record-layout">
                <section class="foundation-card">
                    <h2>"Caixa de entrada"</h2>
                    {ctx.notices.is_empty().then(||view!{<p>"Nenhuma pendência disponível para seus acessos nesta empresa."</p>})}
                    {notice_rows}
                </section>
                <section class="foundation-card">
                    <h2>"Preferências de comunicação"</h2>
                    <p>"Avisos essenciais ficam ativos. Desativar e-mails opcionais não retira as pendências da caixa de entrada."</p>
                    {ctx.context.map(|scope|view!{
                        <form method="post" action="/pendencias/preferencias" class="record-form">
                            {hidden(&scope,&ctx.csrf,&key)}
                            <input type="hidden" name="expected_version" value=version.clone()/>
                            <label class="record-check">
                                <input type="checkbox" name="mute_optional_email" value="true" checked=prefs.mute_optional_email/>
                                "Não enviar e-mails opcionais"
                            </label>
                            <label class="record-check">
                                <input type="checkbox" name="group_by_day" value="true" checked=prefs.group_by_day/>
                                "Agrupar lembretes por dia"
                            </label>
                            <label class="record-field">
                                <span>"Avisar com quantos dias de antecedência (separados por vírgula)"</span>
                                <input name="reminder_days" value=days maxlength="80"/>
                            </label>
                            <p>"Opções: 0, 1, 2, 3, 5, 7, 10, 14, 21, 30, 45, 60, 90, 180 e 365."</p>
                            <button type="submit" class="primary-button">"Salvar preferências"</button>
                        </form>
                    })}
                </section>
            </div>
        }
    )
}
