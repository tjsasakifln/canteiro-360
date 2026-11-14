use super::{
    format,
    model::{ACTIONS, Access, DECISIONS, Draft, ROLES},
    trap_tab,
};
use canteiro_contracts::access::*;
use leptos::prelude::*;

fn access() -> Access {
    expect_context::<Access>()
}
fn action(value: &str) -> AccessAction {
    ACTIONS
        .into_iter()
        .find(|a| a.as_str() == value)
        .unwrap_or(AccessAction::View)
}
fn role(value: &str) -> AccessRole {
    ROLES
        .into_iter()
        .find(|r| r.as_str() == value)
        .unwrap_or(AccessRole::Viewer)
}
fn status(value: &str) -> &'static str {
    match value {
        "pending" => "Aguardando decisão",
        "approved" => "Autorizado",
        "rejected" | "denied" => "Recusado",
        _ => "Confira a situação",
    }
}
fn scope_label(scope: &ScopeView) -> String {
    format!("{}: {}", scope.kind.label(), scope.name)
}
fn proposer_action(value: AccessAction) -> AccessAction {
    match value {
        AccessAction::ApproveBudget => AccessAction::EditBudget,
        AccessAction::ApproveChange => AccessAction::ProposeChange,
        AccessAction::AuthorizePayment => AccessAction::RecordPayment,
        AccessAction::TechnicalAccept => AccessAction::ReviewMeasurement,
        _ => AccessAction::View,
    }
}
fn may_propose(a: Access) -> bool {
    let d = a.draft.get();
    let needed = proposer_action(d.request_action);
    a.state.with(|s| {
        s.as_ref().is_some_and(|s| {
            s.grants.iter().any(|g| {
                g.scope_id == d.selected_scope
                    && g.user_id == s.current_user_id
                    && g.effective
                    && g.actions.contains(&needed)
            })
        })
    })
}

#[component]
fn Field(
    id: &'static str,
    label: &'static str,
    get: fn(&Draft) -> &String,
    set: fn(&mut Draft, String),
    #[prop(default = "text")] kind: &'static str,
    #[prop(default = false)] required: bool,
    #[prop(default = "")] hint: &'static str,
) -> impl IntoView {
    let a = access();
    let hint_id = format!("{id}-hint");
    let described = hint_id.clone();
    view!{<div class="field"><label for=id>{label}</label><input id=id type=kind required=required maxlength="320" autocomplete="off" disabled=move||a.busy.get() aria-describedby=described prop:value=move||a.draft.with(|d|get(d).clone()) on:input=move|e|a.edit(|d|set(d,event_target_value(&e)))/><p id=hint_id class="permission-hint">{hint}</p></div>}.into_any()
}

#[component]
pub(super) fn ContextPanel() -> impl IntoView {
    let a = access();
    view!{<div class="permission-context"><div><p class="eyebrow">"Empresa ativa"</p><h2 id="perm-company-name">{move||a.state.get().and_then(|s|s.organization).map(|o|o.name).unwrap_or_else(||"Nenhuma empresa selecionada".into())}</h2><p>{move||if a.can_manage(){"Você pode configurar os acessos desta empresa."}else{"Você verá somente obras e contratos concedidos à sua conta."}}</p></div><a class="secondary-button" href="/organizacoes">"Escolher empresa"</a></div><Show when=move||a.state.with(|s|s.as_ref().is_some_and(|s|!s.mfa_verified&&s.organization.as_ref().is_some_and(|o|o.role==canteiro_contracts::organizations::OrganizationRole::Administrator)))><div class="permission-notice"><p>"Para configurar os acessos desta empresa, ative e confirme a verificação em duas etapas da sua conta."</p><a class="text-link" href="/acesso" target="_blank" rel="noopener">"Confirmar proteção da conta ↗"</a></div></Show>}.into_any()
}

#[component]
pub(super) fn ScopePanel() -> impl IntoView {
    let a = access();
    let selected = RwSignal::new(String::new());
    Effect::new(move |_| selected.set(a.draft.get().selected_scope));
    view!{
  <section class="permission-section" aria-labelledby="perm-scope-heading"><h2 id="perm-scope-heading">"Onde você vai atuar?"</h2>
   <div class="field"><label for="perm-scope-selection">"Obra, contrato ou orçamento"</label><select id="perm-scope-selection" prop:value=move||selected.get() disabled=move||a.blocked() on:change=move|e|selected.set(event_target_value(&e))><option value="">"Escolha uma obra, contrato ou orçamento"</option><For each=move||a.scopes() key=|s|(s.id.clone(),s.name.clone()) children=move|s|{let label=scope_label(&s);view!{<option value=s.id>{label}</option>}}/></select><p class="permission-hint">"Cada obra, contrato e orçamento precisa de concessão própria. A permissão no nível anterior não libera seus registros vinculados."</p></div>
   <div class="form-actions"><button id="perm-scope-open" type="button" class="primary-button" disabled=move||a.blocked()||selected.get().is_empty()||selected.get()==a.draft.get().selected_scope on:click=move |_|a.request_scope(selected.get_untracked())>"Abrir permissões"</button></div>
   <Show when=move||a.scopes().is_empty()><p class="permission-empty">{move||if a.can_manage(){"Cadastre a primeira obra abaixo para organizar os acessos."}else{"Sua conta ainda não tem acesso a uma obra ou contrato. Peça o vínculo à administração da sua empresa."}}</p></Show>
  </section>
  <Show when=move||a.can_manage()><details class="permission-section" id="perm-create-panel"><summary>"Adicionar obra, contrato ou orçamento"</summary><p>"Identifique onde a equipe terá acesso. Estas referências organizam permissões; o cadastro da obra, o contrato formal e os valores do orçamento são registros separados."</p>
   <form method="post" action="/permissoes" on:submit=move|e|{e.prevent_default();a.create_scope();}><div class="permission-grid">
    <div class="field"><label for="perm-scope-kind">"Tipo"</label><select id="perm-scope-kind" disabled=move||a.busy.get() prop:value=move||a.draft.get().scope_kind.as_str() on:change=move|e|a.edit(|d|{d.scope_kind=match event_target_value(&e).as_str(){"contract"=>ScopeKind::Contract,"budget"=>ScopeKind::Budget,_=>ScopeKind::Work};d.scope_parent.clear();})><option value="work">"Obra"</option><option value="contract">"Contrato"</option><option value="budget">"Orçamento"</option></select></div>
    <Field id="perm-scope-name" label="Nome da obra, contrato ou orçamento" get=|d|&d.scope_name set=|d,v|d.scope_name=v required=true/>
    <Show when=move||a.draft.get().scope_kind!=ScopeKind::Work><div class="field wide"><label for="perm-scope-parent">{move||if a.draft.get().scope_kind==ScopeKind::Contract{"Obra do contrato"}else{"Contrato do orçamento"}}</label><select id="perm-scope-parent" required disabled=move||a.busy.get() prop:value=move||a.draft.get().scope_parent on:change=move|e|a.edit(|d|d.scope_parent=event_target_value(&e))><option value="">"Selecione o registro vinculado"</option><For each=move||{let parent_kind=if a.draft.get().scope_kind==ScopeKind::Contract{ScopeKind::Work}else{ScopeKind::Contract};a.scopes().into_iter().filter(|s|s.kind==parent_kind).collect::<Vec<_>>()} key=|s|(s.id.clone(),s.name.clone()) children=move|s|view!{<option value=s.id>{s.name}</option>}/></select></div></Show>
   </div><div class="form-actions"><button id="perm-scope-create" type="submit" class="primary-button" disabled=move||a.blocked()>"Adicionar"</button></div></form>
  </details></Show>
 }.into_any()
}

#[component]
pub(super) fn Workspace() -> impl IntoView {
    let a = access();
    view!{<section aria-labelledby="perm-workspace-title"><h2 id="perm-workspace-title">{move||a.scope_name()}</h2><p>"Acessos e decisões se aplicam somente à referência identificada acima."</p></section><GrantList/><Show when=move||a.can_manage()><GrantPanel/></Show><Show when=move||!a.draft.get().source_id.is_empty()><DelegatePanel/></Show><PolicyPanel/><RequestList/><RequestPanel/>}.into_any()
}

#[component]
fn ActionChecks(#[prop(default = false)] delegating: bool) -> impl IntoView {
    let a = access();
    let prefix = if delegating {
        "perm-delegate-action"
    } else {
        "perm-grant-action"
    };
    view!{<fieldset class="permission-checks" id=if delegating{"perm-delegate-actions"}else{"perm-grant-actions"} tabindex="-1"><legend>"Tarefas permitidas"</legend>
  {ACTIONS.into_iter().map(move|item|{let id=format!("{prefix}-{}",item.as_str());let label_id=id.clone();view!{<Show when=move||if delegating{a.state.with(|s|s.as_ref().is_some_and(|s|s.grants.iter().find(|g|g.id==a.draft.get().source_id).is_some_and(|g|g.actions.contains(&item))))}else{a.draft.get().grant_role.permits(item)}><div class="permission-checkbox"><input id=id.clone() type="checkbox" disabled=move||a.busy.get()||item==AccessAction::View prop:checked=move||a.draft.with(|d|if delegating{d.delegate_actions.contains(&item)}else{d.grant_actions.contains(&item)}) on:change=move|e|{let checked=event_target_checked(&e);a.edit(|d|{let list=if delegating{&mut d.delegate_actions}else{&mut d.grant_actions};list.retain(|a|*a!=item);if checked{list.push(item);}});}/><label for=label_id.clone()>{item.label()}</label></div></Show>}}).collect_view()}
 </fieldset>}.into_any()
}

#[component]
fn GrantPanel() -> impl IntoView {
    let a = access();
    view!{<details class="permission-section" id="perm-grant-panel"><summary>"Conceder acesso a uma pessoa"</summary><p>"A pessoa deve participar desta empresa. Defina as tarefas, o valor máximo por ato e a validade."</p>
  <form method="post" action="/permissoes" on:submit=move|e|{e.prevent_default();a.grant();}><div class="permission-grid">
   <Field id="perm-grant-email" label="E-mail da pessoa" kind="email" get=|d|&d.grant_email set=|d,v|d.grant_email=v required=true/>
   <div class="field"><label for="perm-grant-role">"Função"</label><select id="perm-grant-role" prop:value=move||a.draft.get().grant_role.as_str() disabled=move||a.busy.get() on:change=move|e|{let value=role(&event_target_value(&e));a.edit(|d|{d.grant_role=value;d.grant_actions=ACTIONS.into_iter().filter(|a|value.permits(*a)).collect();if !matches!(value,AccessRole::Manager|AccessRole::TechnicalResponsible|AccessRole::Approver){d.grant_required=false;}});}>{ROLES.into_iter().map(|r|view!{<option value=r.as_str()>{r.label()}</option>}).collect_view()}</select></div>
   <Field id="perm-grant-limit" label="Valor máximo por ato (R$)" get=|d|&d.grant_limit set=|d,v|d.grant_limit=v required=true hint="Use vírgula para centavos. Zero não concede aprovação de valor positivo."/>
   <Field id="perm-grant-from" label="Início, horário de Brasília" kind="datetime-local" get=|d|&d.grant_from set=|d,v|d.grant_from=v required=true/>
   <Field id="perm-grant-until" label="Término, horário de Brasília" kind="datetime-local" get=|d|&d.grant_until set=|d,v|d.grant_until=v hint="Opcional, exceto para suporte. A autorização encerra no instante informado."/>
  </div><ActionChecks/>
  <Show when=move||matches!(a.draft.get().grant_role,AccessRole::Manager|AccessRole::TechnicalResponsible|AccessRole::Approver)><div class="permission-checkbox"><input id="perm-grant-required" type="checkbox" disabled=move||a.busy.get() prop:checked=move||a.draft.get().grant_required on:change=move|e|a.edit(|d|d.grant_required=event_target_checked(&e))/><label for="perm-grant-required">"Manter responsável ativo, sem vencimento, para estas tarefas e este valor: exigir sucessor antes de encerrar o último vínculo."</label></div></Show>
  <div class="form-actions"><button id="perm-grant-submit" type="submit" class="primary-button" disabled=move||a.blocked()>"Conferir concessão"</button></div></form>
 </details>}.into_any()
}

#[component]
fn GrantList() -> impl IntoView {
    let a = access();
    view!{<section class="permission-section" aria-labelledby="perm-grants-heading"><h2 id="perm-grants-heading">"Vínculos de acesso"</h2><p>"Os limites e períodos de cada pessoa permanecem explícitos. A revogação também alcança delegações dependentes."</p>
 <Show when=move||a.grants().is_empty()><p class="permission-empty">"Nenhum vínculo disponível para sua consulta nesta obra, contrato ou orçamento."</p></Show>
 <ul class="permission-list" id="perm-grant-list"><For each=move||a.grants() key=|g|serde_json::to_string(g).unwrap_or_default() children=move|grant|{let reason=StoredValue::new(grant.reason.clone());let revoke=StoredValue::new(grant.clone());let delegate=StoredValue::new(grant.clone());let actions=grant.actions.iter().map(|a|a.label()).collect::<Vec<_>>().join(", ");let validity=match &grant.valid_until{Some(end)=>format!("De {} até {}",format::instant(&grant.valid_from),format::instant(end)),None=>format!("Desde {}, sem término definido",format::instant(&grant.valid_from))};view!{<li><h3>{grant.user_email.clone()}</h3><span class="permission-badge">{grant.role.label()}</span><span class="permission-badge">{if grant.revoked{"Encerrado"}else if grant.effective{"Em vigor"}else{"Fora de vigor"}}</span><Show when=move||grant.source_grant_id.is_some()><span class="permission-badge">{if grant.substitution{"Substituição temporária"}else{"Delegação temporária"}}</span></Show><Show when=move||grant.required><span class="permission-badge">"Responsável necessário"</span></Show><p>{actions}</p><Show when=move||!reason.get_value().is_empty()><p><strong>"Motivo: "</strong>{move||reason.get_value()}</p></Show><p><strong>"Limite por ato: "</strong>{format::money(&grant.max_amount.to_string())}</p><p class="permission-hint">{validity}</p><div class="form-actions"><Show when=move||grant.can_delegate><button id=format!("perm-grant-delegate-{}",grant.id) type="button" class="secondary-button" disabled=move||a.blocked() on:click=move |_|a.begin_delegation(delegate.get_value())>"Delegar ou substituir"</button></Show><Show when=move||grant.can_revoke><button id=format!("perm-grant-revoke-{}",revoke.get_value().id) type="button" class="secondary-button" disabled=move||a.blocked() on:click=move |_|a.revoke(revoke.get_value())>"Encerrar vínculo"</button></Show></div></li>}}/></ul>
 </section>}.into_any()
}

#[component]
fn DelegatePanel() -> impl IntoView {
    let a = access();
    view!{<details open class="permission-section" id="perm-delegate-panel"><summary>"Delegação e substituição temporária"</summary><p>"Você pode compartilhar somente as tarefas e o valor que já recebeu. O término é obrigatório. O vínculo de origem continua sendo conferido a cada decisão."</p>
 <form method="post" action="/permissoes" on:submit=move|e|{e.prevent_default();a.delegate();}><div class="permission-grid">
  <Field id="perm-delegate-email" label="E-mail de quem receberá o acesso" kind="email" get=|d|&d.delegate_email set=|d,v|d.delegate_email=v required=true/>
  <Field id="perm-delegate-limit" label="Valor máximo delegado por ato (R$)" get=|d|&d.delegate_limit set=|d,v|d.delegate_limit=v required=true/>
  <Field id="perm-delegate-from" label="Início, horário de Brasília" kind="datetime-local" get=|d|&d.delegate_from set=|d,v|d.delegate_from=v required=true/>
  <Field id="perm-delegate-until" label="Término, horário de Brasília" kind="datetime-local" get=|d|&d.delegate_until set=|d,v|d.delegate_until=v required=true/><div class="field wide"><label for="perm-delegate-reason">"Motivo da delegação ou substituição"</label><textarea id="perm-delegate-reason" rows="3" required minlength="2" maxlength="200" disabled=move||a.busy.get() prop:value=move||a.draft.get().delegate_reason on:input=move|e|a.edit(|d|d.delegate_reason=event_target_value(&e))/><p class="permission-hint">"Explique por que o acesso temporário é necessário. O motivo será registrado com o vínculo."</p></div>
 </div><ActionChecks delegating=true/>
 <div class="permission-checkbox"><input id="perm-delegate-substitution" type="checkbox" prop:checked=move||a.draft.get().substitution disabled=move||a.busy.get() on:change=move|e|a.edit(|d|d.substitution=event_target_checked(&e))/><label for="perm-delegate-substitution">"Registrar como substituição durante minha ausência."</label></div>
 <p class="permission-hint">"O titular mantém sua permissão. A substituição registra a cobertura temporária e não cria uma segunda autorização independente para dupla aprovação."</p>
 <div class="form-actions"><button id="perm-delegate-submit" type="submit" class="primary-button" disabled=move||a.blocked()>"Conferir acesso temporário"</button><button type="button" class="secondary-button" disabled=move||a.busy.get() on:click=move |_|a.draft.update(|d|{d.source_id.clear();d.delegate_email.clear();d.delegate_reason.clear();d.dirty_forms&=!4;d.changed=d.dirty_forms!=0;})>"Cancelar"</button></div></form>
 </details>}.into_any()
}

#[component]
fn PolicyPanel() -> impl IntoView {
    let a = access();
    view!{<details class="permission-section" id="perm-policy-panel"><summary>"Regras de aprovação"</summary><p>"O maior valor entre acréscimo e redução define a alçada. Um acréscimo de R$ 100,00 com redução de R$ 100,00 continua contendo acréscimo, mesmo que o saldo seja zero."</p>
 <ul id="perm-policy-summary" class="permission-list"><For each=move||{let id=a.draft.get().selected_scope;a.state.get().map(|s|s.policies.into_iter().filter(|p|p.scope_id==id).collect::<Vec<_>>()).unwrap_or_default()} key=|p|(p.action.as_str(),p.version) children=move|p|view!{<li><h3>{p.action.label()}</h3><p>{format!("{} aprovação(ões) a partir de {}. Abaixo desse valor, uma aprovação. Versão {}.",p.approvals_above_threshold,format::money(&p.threshold.to_string()),p.version)}</p><p>{if p.segregate_proposer{"O proponente e as pessoas com conflito declarado não aprovam o pedido."}else{"A regra permite que o proponente participe, se tiver alçada. Pessoas com conflito declarado continuam impedidas."}}</p></li>}/></ul>
 <Show when=move||a.can_manage()><form method="post" action="/permissoes" on:submit=move|e|{e.prevent_default();a.policy();}><div class="permission-grid">
 <div class="field"><label for="perm-policy-action">"Decisão"</label><select id="perm-policy-action" prop:value=move||a.draft.get().policy_action.as_str() disabled=move||a.busy.get() on:change=move|e|a.change_policy(action(&event_target_value(&e)))>{DECISIONS.into_iter().map(|item|view!{<option value=item.as_str()>{item.label()}</option>}).collect_view()}</select></div>
 <Field id="perm-policy-threshold" label="Valor a partir do qual a regra se aplica (R$)" get=|d|&d.policy_threshold set=|d,v|d.policy_threshold=v required=true/>
 </div><div class="permission-checkbox"><input id="perm-policy-double" type="checkbox" disabled=move||a.busy.get() prop:checked=move||a.draft.get().policy_double on:change=move|e|a.edit(|d|d.policy_double=event_target_checked(&e))/><label for="perm-policy-double">"Exigir duas aprovações independentes a partir desse valor."</label></div>
 <div class="permission-checkbox"><input id="perm-policy-segregate" type="checkbox" disabled=move||a.busy.get() prop:checked=move||a.draft.get().policy_segregate on:change=move|e|a.edit(|d|d.policy_segregate=event_target_checked(&e))/><label for="perm-policy-segregate">"Impedir que o proponente aprove o próprio pedido."</label></div><p class="permission-hint">{move||a.draft.get().policy_version.map(|v|format!("Versão em edição: {v}. Atualizar o acesso preserva esta versão até você escolher carregar a regra atual.")).unwrap_or_else(||"Nova regra. Confira a versão atual antes de substituir uma regra criada por outra pessoa.".into())}</p><div class="form-actions"><button id="perm-policy-submit" type="submit" class="primary-button" disabled=move||a.blocked()>"Conferir regra"</button><button id="perm-policy-reload" type="button" class="secondary-button" disabled=move||a.blocked() on:click=move |_|{a.load_policy();a.draft.update(|d|{d.dirty_forms&=!8;d.changed=d.dirty_forms!=0;});}>"Descartar edição e carregar regra atual"</button></div></form></Show>
 </details>}.into_any()
}

#[component]
fn RequestPanel() -> impl IntoView {
    let a = access();
    view!{<details class="permission-section" id="perm-request-panel"><summary>{move||if a.draft.get().request_id.is_empty(){"Solicitar uma decisão"}else{"Revisar o pedido"}}</summary><p>"Registre a decisão necessária com os valores separados. Esta autorização não executa serviços nem registra pagamentos."</p>
 <form method="post" action="/permissoes" on:submit=move|e|{e.prevent_default();a.propose();}><div class="permission-grid">
 <div class="field"><label for="perm-request-action">"Decisão necessária"</label><select id="perm-request-action" disabled=move||a.busy.get()||!a.draft.get().request_id.is_empty() prop:value=move||a.draft.get().request_action.as_str() on:change=move|e|a.edit(|d|d.request_action=action(&event_target_value(&e)))>{DECISIONS.into_iter().map(|item|view!{<option value=item.as_str()>{item.label()}</option>}).collect_view()}</select></div>
 <Field id="perm-request-title" label="Descrição do pedido" get=|d|&d.request_title set=|d,v|d.request_title=v required=true/>
 <Field id="perm-request-increase" label="Acréscimo ou valor a autorizar (R$)" get=|d|&d.request_increase set=|d,v|d.request_increase=v required=true hint="Informe o valor positivo por inteiro, antes de qualquer compensação."/>
 <Field id="perm-request-decrease" label="Redução (R$)" get=|d|&d.request_decrease set=|d,v|d.request_decrease=v required=true hint="Informe a redução como valor positivo, em campo separado."/>
 </div><fieldset class="permission-checks"><legend>"Pessoas com conflito de interesse neste pedido"</legend><p class="permission-hint">"Pessoas marcadas não poderão decidir esta versão do pedido."</p><For each=move||{let scope_id=a.draft.get().selected_scope;a.state.get().map(|s|s.members.into_iter().filter(|m|m.active&&s.grants.iter().any(|g|g.scope_id==scope_id&&g.user_id==m.id&&g.effective)).collect::<Vec<_>>()).unwrap_or_default()} key=|m|(m.id.clone(),m.version) children=move|m|{let id=format!("perm-conflict-{}",m.id);let uid=m.id.clone();let uid2=m.id.clone();view!{<div class="permission-checkbox"><input id=id.clone() type="checkbox" disabled=move||a.busy.get() prop:checked=move||a.draft.get().request_conflicts.contains(&uid) on:change=move|e|{let checked=event_target_checked(&e);a.edit(|d|{d.request_conflicts.retain(|x|x!=&uid2);if checked{d.request_conflicts.push(uid2.clone());}});}/><label for=id>{m.email}</label></div>}}/></fieldset>
 <Show when=move||!may_propose(a)><p class="permission-denial">"Seu vínculo não permite propor esta decisão aqui. Peça à administração da empresa um vínculo para a tarefa correspondente, nesta obra, contrato ou orçamento."</p></Show>
 <div class="form-actions"><button id="perm-request-submit" type="submit" class="primary-button" disabled=move||a.blocked()||!may_propose(a)>"Conferir pedido"</button><Show when=move||!a.draft.get().request_id.is_empty()><button id="perm-request-cancel" type="button" class="secondary-button" disabled=move||a.busy.get() on:click=move |_|a.draft.update(|d|{d.request_id.clear();d.request_title.clear();d.request_conflicts.clear();d.dirty_forms&=!16;d.changed=d.dirty_forms!=0;})>"Cancelar revisão"</button></Show></div></form>
 </details>}.into_any()
}

#[component]
fn RequestList() -> impl IntoView {
    let a = access();
    view!{<section class="permission-section" aria-labelledby="perm-requests-heading"><h2 id="perm-requests-heading" tabindex="-1">"Pedidos de decisão"</h2><Show when=move||a.requests().is_empty()><p class="permission-empty">"Nenhum pedido disponível para sua consulta nesta obra, contrato ou orçamento."</p></Show><ul class="permission-list" id="perm-request-list"><For each=move||a.requests() key=|r|serde_json::to_string(r).unwrap_or_default() children=move|request|view!{<RequestCard request=request/>}/></ul></section>}.into_any()
}

#[component]
fn RequestCard(request: RequestView) -> impl IntoView {
    let a = access();
    let selected = RwSignal::new(
        request
            .allowed_grant_ids
            .first()
            .cloned()
            .unwrap_or_default(),
    );
    let approve = StoredValue::new(request.clone());
    let reject = StoredValue::new(request.clone());
    let revise = StoredValue::new(request.clone());
    let denial = request.denial.clone();
    let grant_ids = request.allowed_grant_ids.clone();
    let grants = a
        .state
        .get_untracked()
        .map(|s| {
            s.grants
                .into_iter()
                .filter(|g| grant_ids.contains(&g.id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let has_grants = !grants.is_empty();
    let selector_id = format!("perm-decision-grant-{}", request.id);
    let heading_id = format!("perm-request-{}", request.id);
    let policy_requires_revision = request.policy_changed && request.status == "pending";
    view!{<li aria-labelledby=heading_id.clone()><h3 id=heading_id.clone()>{request.title.clone()}</h3><span class="permission-badge">{request.action.label()}</span><span class="permission-badge">{status(&request.status)}</span><p>{format!("Proposto por {} · Versão {}",request.proposer_email,request.version)}</p>
 <dl class="permission-values"><div><dt>"Acréscimo ou autorização"</dt><dd>{format::money(&request.increase.to_string())}</dd></div><div><dt>"Redução"</dt><dd>{format::money(&request.decrease.to_string())}</dd></div><div><dt>"Valor usado na alçada"</dt><dd>{format::money(&request.authority_amount.to_string())}</dd></div></dl>
 <p>{format!("{} de {} aprovações válidas. Regra na versão {}.",request.valid_approvals,request.required_approvals,request.policy_version)}</p>
 <Show when=move||request.contains_increase><p class="permission-hint">"Este pedido contém acréscimo. Uma redução simultânea não elimina a exigência de alçada."</p></Show>
 <Show when=move||policy_requires_revision><p class="permission-denial">"A regra de aprovação mudou. O proponente precisa revisar o pedido antes de uma nova decisão."</p></Show>
 {denial.map(|d|view!{<div class="permission-denial"><p>{d.message}</p><p>{d.next_step}</p>{(!d.resolvers.is_empty()).then(||view!{<p>{format!("Quem pode ajudar: {}",d.resolvers.join(", "))}</p>})}</div>})}
 <Show when=move||has_grants><div class="field"><label for=selector_id.clone()>"Autorização que você usará"</label><select id=selector_id.clone() disabled=move||a.busy.get() prop:value=move||selected.get() on:change=move|e|selected.set(event_target_value(&e))>{grants.iter().map(|g|view!{<option value=g.id.clone()>{format!("{} · até {}",g.role.label(),format::money(&g.max_amount.to_string()))}</option>}).collect_view()}</select></div><div class="form-actions"><button id=format!("perm-approve-{}",request.id) type="button" class="primary-button" disabled=move||a.blocked()||selected.get().is_empty() on:click=move |_|a.decide(approve.get_value(),selected.get_untracked(),true)>"Aprovar versão"</button><button id=format!("perm-reject-{}",reject.get_value().id) type="button" class="secondary-button" disabled=move||a.blocked()||selected.get().is_empty() on:click=move |_|a.decide(reject.get_value(),selected.get_untracked(),false)>"Recusar versão"</button></div></Show>
 <Show when=move||request.can_revise><div class="form-actions"><button id=format!("perm-revise-{}",revise.get_value().id) type="button" class="secondary-button" disabled=move||a.blocked() on:click=move |_|a.begin_revision(revise.get_value())>"Revisar pedido"</button></div></Show>
 <details><summary>"Histórico de decisões"</summary><ul>{request.approvals.iter().map(|p|view!{<li>{format!("{}: {} a versão {} em {}",p.user_email,if p.approved{"aprovou"}else{"recusou"},p.request_version,format::instant(&p.decided_at))}</li>}).collect_view()}</ul></details>
 </li>}.into_any()
}

#[component]
pub(super) fn ConfirmationDialog() -> impl IntoView {
    let a = access();
    view!{<dialog id="perm-confirm-dialog" aria-labelledby="perm-confirm-title" aria-describedby="perm-confirm-description" on:cancel=move|e:leptos::ev::Event|{e.prevent_default();a.dismiss();} on:keydown=trap_tab><h2 id="perm-confirm-title">{move||a.pending.get().map(|p|p.title).unwrap_or_default()}</h2><p id="perm-confirm-description">{move||a.pending.get().map(|p|p.description).unwrap_or_default()}</p><div class="form-actions"><button id="perm-confirm-cancel" class="secondary-button" type="button" on:click=move |_|a.dismiss()>"Voltar e conferir"</button><button id="perm-confirm-submit" class="primary-button" type="button" disabled=move||a.blocked() on:click=move |_|a.confirm_pending()>"Confirmar"</button></div></dialog>}.into_any()
}
