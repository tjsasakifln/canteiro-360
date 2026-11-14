use super::{
    model::{Organizations, end_of_day, role},
    network, select_code,
};
use canteiro_contracts::organizations::*;
use leptos::prelude::*;

fn use_organizations() -> Organizations {
    expect_context::<Organizations>()
}

fn instant(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%d/%m/%Y às %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| "Data indisponível".into())
}
fn civil_date(value: Option<&str>) -> String {
    value
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|date| {
            date.with_timezone(&chrono_tz::America::Sao_Paulo)
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_default()
}
fn role_description(value: OrganizationRole) -> &'static str {
    match value {
        OrganizationRole::Administrator => {
            "Administra a empresa, os convites e a equipe. Exige confirmação em duas etapas."
        }
        OrganizationRole::Member => {
            "Participa da empresa. O acesso a obras e contratos depende de permissões específicas."
        }
    }
}
fn status_label(value: &str) -> &'static str {
    match value {
        "pending" => "Pendente",
        "accepted" => "Aceito",
        "revoked" => "Revogado",
        "expired" => "Expirado",
        "approved" => "Aprovado",
        "denied" => "Recusado",
        _ => "Situação indisponível",
    }
}

#[component]
fn Field(
    id: &'static str,
    label: &'static str,
    value: RwSignal<String>,
    #[prop(default = "text")] kind: &'static str,
    #[prop(default = false)] required: bool,
) -> impl IntoView {
    let org = use_organizations();
    view! { <div class="field"><label for=id>{label}</label>
        <input id=id type=kind required=required maxlength="320" autocomplete="off" disabled=move || org.busy.get()
            prop:value=move || value.get() on:input=move |event| value.set(event_target_value(&event))/>
    </div> }.into_any()
}

#[component]
fn RoleField(id: &'static str, value: RwSignal<OrganizationRole>) -> impl IntoView {
    let org = use_organizations();
    view! { <div class="field"><label for=id>"Participação concedida"</label>
        <select id=id disabled=move || org.busy.get() prop:value=move || value.get().as_str()
            on:change=move |event| value.set(role(&event_target_value(&event)))>
            <option value="member">"Membro"</option><option value="administrator">"Administrador"</option>
        </select><p class="org-role-note">{move || role_description(value.get())}</p>
    </div> }.into_any()
}

#[component]
pub(super) fn SelectionPanel() -> impl IntoView {
    let org = use_organizations();
    let selected = RwSignal::new(String::new());
    Effect::new(move |_| {
        selected.set(
            org.state
                .get()
                .active
                .map(|a| a.organization.id)
                .unwrap_or_default(),
        )
    });
    view! {
        <div class="org-context" aria-labelledby="org-active-name">
            <div><p class="eyebrow">"Empresa ativa"</p>
                <h2 id="org-active-name">{move || org.state.get().active.map(|a| a.organization.name).unwrap_or_else(|| "Escolha uma empresa".into())}</h2>
                <p class="org-context-role">{move || org.state.get().active.map(|a| format!("Sua participação: {}", a.organization.role.label())).unwrap_or_else(|| "Nenhuma empresa selecionada nesta sessão.".into())}</p>
            </div>
            <div class="org-selector"><label for="org-selection">"Trabalhar na empresa"</label>
                <div class="org-selector-controls">
                    <select id="org-selection" disabled=move || org.blocked() prop:value=move || selected.get()
                        on:change=move |event| selected.set(event_target_value(&event))>
                        <option value="">"Nenhuma empresa selecionada"</option>
                        <For each=move || org.state.get().organizations key=|a| (a.id.clone(), a.name.clone()) children=move |a| view! { <option value=a.id>{a.name}</option> }/>
                    </select>
                    <button id="org-select-submit" class="primary-button" type="button"
                        disabled=move || org.blocked() || selected.get() == org.state.get().active.map(|a| a.organization.id).unwrap_or_default()
                        on:click=move |_| { let value=selected.get_untracked(); org.select(if value.is_empty() { None } else { Some(value) }); }>"Usar empresa"</button>
                </div>
                <p class="field-hint">"Somente empresas às quais sua conta tem acesso aparecem aqui."</p>
            </div>
        </div>
        <Show when=move || org.state.get().organizations.is_empty()>
            <p class="org-empty">"Sua conta ainda não participa de uma empresa. Aceite um convite, peça acesso com o código recebido ou crie sua empresa abaixo."</p>
        </Show>
    }.into_any()
}

#[component]
pub(super) fn JoinPanels() -> impl IntoView {
    let org = use_organizations();
    view! {
        <section aria-labelledby="org-join-heading" class="org-workspace">
            <h2 id="org-join-heading">"Participar de uma empresa"</h2>
            <div class="org-columns">
                <section class="org-card" aria-labelledby="org-invitation-heading">
                    <h3 id="org-invitation-heading">"Recebeu um convite?"</h3>
                    <p>"Cole o código enviado para o seu e-mail. Você verá a empresa e a participação antes de aceitar."</p>
                    <form method="post" action="/organizacoes" on:submit=move |event| {
                        event.prevent_default();
                        let token=org.invitation_token.get_untracked();
                        if token.trim().is_empty() { org.local_error("Cole o código do convite.", "org-invitation-token"); return; }
                        org.submit(OrganizationCommand::PreviewInvitation {token});
                    }>
                        <div class="field"><label for="org-invitation-token">"Código do convite"</label>
                            <input id="org-invitation-token" type="text" autocomplete="off" spellcheck="false" required maxlength="100" disabled=move || org.busy.get()
                                prop:value=move || org.invitation_token.get() on:input=move |event| { org.invitation_token.set(event_target_value(&event)); org.invitation_preview.set(None); }/>
                        </div>
                        <button id="org-preview-invitation" class="secondary-button" disabled=move || org.blocked() type="submit">"Conferir convite"</button>
                    </form>
                    <Show when=move || org.invitation_preview.get().is_some()>
                        <div class="org-invitation-preview" aria-labelledby="org-preview-title">
                            <h3 id="org-preview-title">{move || org.invitation_preview.get().map(|p| p.organization_name).unwrap_or_default()}</h3>
                            <p>{move || org.invitation_preview.get().map(|p| format!("Participação: {}", p.role.label())).unwrap_or_default()}</p>
                            <p class="org-role-note">{move || org.invitation_preview.get().map(|p| role_description(p.role)).unwrap_or_default()}</p>
                            <p>{move || org.invitation_preview.get().map(|p| format!("Aceite até {} (Brasília).", instant(&p.expires_at))).unwrap_or_default()}</p>
                            <p>{move || org.invitation_preview.get().and_then(|p| p.membership_valid_until).map(|v| format!("O acesso termina em {} (Brasília).", instant(&v))).unwrap_or_else(|| "Participação sem data de término definida.".into())}</p>
                            <button id="org-accept-invitation" type="button" class="primary-button" disabled=move || org.blocked()
                                on:click=move |_| org.submit(OrganizationCommand::AcceptInvitation {token: org.invitation_token.get_untracked()})>"Aceitar este acesso"</button>
                        </div>
                    </Show>
                </section>
                <section class="org-card" aria-labelledby="org-request-heading">
                    <h3 id="org-request-heading">"Pedir acesso"</h3><p>"Use o código de acesso informado pela empresa. Um administrador analisará seu pedido."</p>
                    <form method="post" action="/organizacoes" on:submit=move |event| {
                        event.prevent_default(); let code=org.request_code.get_untracked();
                        if code.trim().is_empty() { org.local_error("Informe o código de acesso da empresa.", "org-request-code"); return; }
                        org.submit(OrganizationCommand::RequestAccess {code, requested_role:org.request_role.get_untracked()});
                    }>
                        <Field id="org-request-code" label="Código de acesso da empresa" value=org.request_code required=true/>
                        <div class="field"><label for="org-request-role">"Participação solicitada"</label>
                            <select id="org-request-role" disabled=move || org.busy.get() prop:value=move || org.request_role.get().as_str() on:change=move |event| org.request_role.set(role(&event_target_value(&event)))>
                                <option value="member">"Membro"</option><option value="administrator">"Administrador"</option>
                            </select><p class="field-hint">"Enviar o pedido ainda não concede acesso."</p>
                        </div>
                        <button id="org-request-submit" class="primary-button" type="submit" disabled=move || org.blocked()>"Enviar pedido"</button>
                    </form>
                </section>
            </div>
            <section class="org-card" aria-labelledby="org-create-heading">
                <h3 id="org-create-heading">"Criar uma empresa"</h3><p>"Sua conta será a primeira administradora. Confirme a proteção em duas etapas antes de criar."</p>
                <Show when=move || !org.state.get().mfa_verified><p class="notice warning"><a id="org-protect-account" href="/acesso" target="_blank" rel="noopener">"Ativar ou confirmar proteção da conta em outra aba ↗"</a>". Ao retornar, atualize o acesso."</p></Show>
                <form method="post" action="/organizacoes" on:submit=move |event| {
                    event.prevent_default(); let name=org.create_name.get_untracked();
                    if name.trim().is_empty() { org.local_error("Informe o nome da empresa.", "org-create-name"); return; }
                    org.submit(OrganizationCommand::Create {name});
                }>
                    <Field id="org-create-name" label="Nome da empresa" value=org.create_name required=true/>
                    <button id="org-create-submit" class="primary-button" type="submit" disabled=move || org.blocked() || !org.state.get().mfa_verified>"Criar empresa"</button>
                </form>
            </section>
            <AccessCode/>
        </section>
    }.into_any()
}

#[component]
fn AccessCode() -> impl IntoView {
    let org = use_organizations();
    view! {
        <Show when=move || org.access_code.get().is_some()>
            <section class="access-secret org-code" aria-labelledby="org-access-code-heading">
                <h3 id="org-access-code-heading">"Código para pedir acesso"</h3>
                <p>{move || org.access_code.get().and_then(|code| code.split_once('.').map(|(id,_)| id.to_owned())).and_then(|id| org.state.get().organizations.into_iter().find(|a| a.id==id)).map(|a| format!("Empresa: {}.",a.name)).unwrap_or_default()}</p>
                <p>"Compartilhe somente com quem deve pedir acesso. Cada pedido dependerá da aprovação de um administrador. Este código aparece apenas agora."</p>
                <label for="org-access-code">"Código completo"</label>
                <textarea id="org-access-code" readonly rows="3" autocomplete="off" spellcheck="false" prop:value=move || org.access_code.get().unwrap_or_default()/>
                <div class="org-row-actions"><button id="org-copy-access-code" type="button" class="secondary-button" on:click=move |_| select_code("org-access-code")>"Selecionar para copiar"</button>
                    <button id="org-hide-access-code" type="button" class="secondary-button" on:click=move |_| org.access_code.set(None)>"Ocultar código"</button></div>
            </section>
        </Show>
    }.into_any()
}

#[component]
pub(super) fn ActiveWorkspace() -> impl IntoView {
    let org = use_organizations();
    view! {
        <section class="org-workspace" aria-labelledby="org-team-heading">
            <div class="org-tools"><h2 id="org-team-heading">"Equipe e participação"</h2>
                <button id="org-refresh" class="secondary-button" type="button" disabled=move || org.busy.get() on:click=move |_| org.load(true)>"Atualizar empresa"</button>
            </div>
            <Show when=move || org.state.get().active.is_some_and(|a| a.organization.role==OrganizationRole::Administrator) && !org.state.get().mfa_verified>
                <p class="notice warning">"Para administrar a equipe, confirme a proteção em duas etapas na sua conta e atualize esta página."</p>
            </Show>
            <MembersPanel/>
            <Show when=move || org.administrator()>
                <div class="org-columns"><InvitationsPanel/><RequestsPanel/></div>
                <OrganizationSettings/>
            </Show>
        </section>
    }.into_any()
}

#[component]
fn InvitationsPanel() -> impl IntoView {
    let org = use_organizations();
    view! {
        <section class="org-card" aria-labelledby="org-invite-heading">
            <h3 id="org-invite-heading">"Convidar uma pessoa"</h3><p>"O convite fica vinculado ao e-mail informado. Copie o código e envie à pessoa após a confirmação."</p>
            <form method="post" action="/organizacoes" on:submit=move |event| {
                event.prevent_default(); let Some(scope)=org.scope() else {return;};
                let email=org.invite_email.get_untracked();
                if email.trim().is_empty() { org.local_error("Informe o e-mail da pessoa convidada.", "org-invite-email"); return; }
                let Ok(valid_for_hours)=org.invite_hours.get_untracked().parse::<u32>() else { org.local_error("Informe a validade entre 1 e 168 horas.", "org-invite-hours"); return; };
                if !(1..=168).contains(&valid_for_hours) { org.local_error("Informe a validade entre 1 e 168 horas.", "org-invite-hours"); return; }
                let role=org.invite_role.get_untracked();
                let membership_valid_until=if role==OrganizationRole::Administrator {None} else {end_of_day(&org.invite_until.get_untracked())};
                org.submit(OrganizationCommand::Invite {scope,email,role,valid_for_hours,membership_valid_until});
            }>
                <Field id="org-invite-email" label="E-mail da pessoa convidada" kind="email" value=org.invite_email required=true/>
                <RoleField id="org-invite-role" value=org.invite_role/>
                <div class="field"><label for="org-invite-hours">"Prazo para aceitar o convite, em horas"</label>
                    <input id="org-invite-hours" type="number" min="1" max="168" required disabled=move || org.busy.get()
                        prop:value=move || org.invite_hours.get() on:input=move |event| org.invite_hours.set(event_target_value(&event))/>
                    <p class="field-hint">"De 1 hora a 7 dias. O prazo começa quando o convite é criado."</p>
                </div>
                <Show when=move || org.invite_role.get()==OrganizationRole::Member>
                    <Field id="org-invite-until" label="Último dia de acesso, opcional (Brasília)" kind="date" value=org.invite_until/>
                </Show>
                <button id="org-invite-submit" class="primary-button" type="submit" disabled=move || org.blocked()>"Gerar convite"</button>
            </form>
            <Show when=move || org.invitation_code.get().is_some()>
                <div class="access-secret org-code">
                    <h3>"Convite pronto para compartilhar"</h3><p>"Copie o código e envie ao e-mail convidado para que a pessoa confira e aceite a participação."</p>
                    <label for="org-invitation-code">"Código completo do convite"</label>
                    <textarea id="org-invitation-code" readonly rows="3" autocomplete="off" spellcheck="false" prop:value=move || org.invitation_code.get().unwrap_or_default()/>
                    <div class="org-row-actions"><button id="org-copy-invitation-code" type="button" class="secondary-button" on:click=move |_| select_code("org-invitation-code")>"Selecionar para copiar"</button>
                        <button id="org-hide-invitation-code" type="button" class="secondary-button" on:click=move |_| org.invitation_code.set(None)>"Ocultar código"</button></div>
                </div>
            </Show>
            <details id="org-invitation-history"><summary>"Convites desta empresa"</summary>
                <Show when=move || org.state.get().active.is_none_or(|a| a.invitations.is_empty())><p class="org-empty">"Nenhum convite registrado nesta empresa."</p></Show>
                <ul class="org-list"><For each=move || org.state.get().active.map_or_else(Vec::new, |a| a.invitations) key=|v| (v.id.clone(),v.version,v.status.clone()) children=move |invitation| view! {<InvitationRow invitation/>}/></ul>
            </details>
        </section>
    }.into_any()
}

#[component]
fn InvitationRow(invitation: InvitationSummary) -> impl IntoView {
    let org = use_organizations();
    let id = invitation.id.clone();
    let version = invitation.version;
    let pending = invitation.status == "pending";
    let email = invitation.email.clone();
    let button_id = format!("org-revoke-invite-{id}");
    let focus_id = button_id.clone();
    view! { <li><div class="org-row-heading"><strong>{invitation.email}</strong><span class="status-badge">{status_label(&invitation.status)}</span></div>
        <p class="org-row-meta">{format!("{} · Aceite até {} (Brasília)",invitation.role.label(),instant(&invitation.expires_at))}</p>
        <Show when=move || pending><div class="org-row-actions"><button id=button_id.clone() type="button" class="secondary-button" disabled=move || org.blocked()
            on:click={let id=id.clone(); let email=email.clone(); let focus_id=focus_id.clone(); move |_| if let Some(scope)=org.scope() { org.confirm(OrganizationCommand::RevokeInvite {scope,invitation_id:id.clone(),invitation_version:version}, "Revogar este convite?", &format!("O código destinado a {email} deixará de conceder acesso."), &focus_id); }}>"Revogar convite"</button></div></Show>
    </li> }.into_any()
}

#[component]
fn RequestsPanel() -> impl IntoView {
    let org = use_organizations();
    view! { <section class="org-card" aria-labelledby="org-requests-heading">
        <h3 id="org-requests-heading">"Pedidos de acesso"</h3><p>"Confira a pessoa e a participação solicitada antes de aprovar. A decisão fica registrada."</p>
        <Show when=move || org.state.get().active.is_none_or(|a| a.access_requests.is_empty())><p class="org-empty">"Nenhum pedido recebido nesta empresa."</p></Show>
        <ul class="org-list"><For each=move || org.state.get().active.map_or_else(Vec::new, |a| a.access_requests) key=|v| (v.id.clone(),v.version) children=move |request| view! {<RequestRow request/>}/></ul>
    </section> }.into_any()
}

#[component]
fn RequestRow(request: AccessRequestSummary) -> impl IntoView {
    let org = use_organizations();
    let id = request.id.clone();
    let version = request.version;
    let pending = request.status == "pending";
    let role = request.requested_role;
    let email = request.email.clone();
    let approve_id = format!("org-approve-request-{id}");
    let deny_id = format!("org-deny-request-{id}");
    let approve_focus = approve_id.clone();
    let deny_focus = deny_id.clone();
    let deny_request = id.clone();
    view! { <li><div class="org-row-heading"><strong>{request.email}</strong><span class="status-badge">{status_label(&request.status)}</span></div>
        <p class="org-row-meta">{format!("Solicita {} · {} (Brasília)",role.label(),instant(&request.created_at))}</p>
        <p class="org-role-note">{role_description(role)}</p>
        <Show when=move || pending><div class="org-row-actions">
            <button id=approve_id.clone() type="button" class="primary-button" disabled=move || org.blocked()
                on:click={let id=id.clone(); let email=email.clone(); let approve_focus=approve_focus.clone(); move |_| if let Some(scope)=org.scope() { org.confirm(OrganizationCommand::DecideAccess {scope,request_id:id.clone(),request_version:version,approve:true}, "Aprovar esta participação?", &format!("{email} receberá a participação de {} nesta empresa.",role.label()), &approve_focus); }}>"Aprovar"</button>
            <button id=deny_id.clone() type="button" class="secondary-button" disabled=move || org.blocked()
                on:click={let deny_request=deny_request.clone(); let deny_focus=deny_focus.clone(); move |_| if let Some(scope)=org.scope() { org.confirm(OrganizationCommand::DecideAccess {scope,request_id:deny_request.clone(),request_version:version,approve:false}, "Recusar este pedido?", "Este pedido será encerrado sem conceder acesso à empresa.", &deny_focus); }}>"Recusar"</button>
        </div></Show>
    </li> }.into_any()
}

#[component]
fn MembersPanel() -> impl IntoView {
    let org = use_organizations();
    view! { <section class="org-card" aria-labelledby="org-members-heading">
        <h3 id="org-members-heading">{move || if org.administrator() {"Pessoas da empresa"} else {"Sua participação"}}</h3>
        <Show when=move || org.administrator()>
            <form method="get" action="/organizacoes" class="org-search" on:submit=move |event| {event.prevent_default();org.search(0);}>
                <Field id="org-member-search" label="Buscar por e-mail nesta empresa" value=org.search_query/>
                <button id="org-member-search-submit" class="secondary-button" type="submit" disabled=move || org.blocked() || org.search_busy.get()>"Buscar ou listar"</button>
            </form>
            <p class="org-search-status" role="status" aria-live="polite">{move || if org.search_busy.get() {"Consultando pessoas desta empresa…".into()} else {org.search_result.get().map(|r|format!("{} resultado(s) nesta empresa.",r.total)).unwrap_or_else(||"Use a busca para consultar todas as páginas da equipe.".into())}}</p>
            <div class="org-row-actions"><a id="org-export-members" class="text-link" download="equipe.tsv"
                href=move || org.scope().map(|s|network::members_url(&s.organization_id,s.context_version,&org.search_query.get(),0,true)).unwrap_or_else(||"#".into())>"Exportar esta consulta da equipe"</a></div>
        </Show>
        <ul id="org-member-list" class="org-list" aria-busy=move || org.search_busy.get().to_string()>
            <For each=move || org.member_rows() key=|m| (m.id.clone(),m.version,m.active) children=move |member| view! {<MemberRow member/>}/>
        </ul>
        <Show when=move || org.member_rows().is_empty() && !org.search_busy.get()><p class="org-empty">"Nenhuma participação encontrada nesta consulta."</p></Show>
        <Show when=move || org.search_result.get().is_some_and(|r|r.total>50)>
            <div class="org-pagination"><button id="org-member-previous" type="button" class="secondary-button" disabled=move || org.blocked() || org.search_busy.get() || org.search_offset.get()==0 on:click=move |_| org.search((org.search_offset.get_untracked()-50).max(0))>"Página anterior"</button>
                <span>{move ||format!("Página {}",org.search_offset.get()/50+1)}</span>
                <button id="org-member-next" type="button" class="secondary-button" disabled=move || org.blocked() || org.search_busy.get() || org.search_result.get().is_none_or(|r|org.search_offset.get()+50>=r.total) on:click=move |_|org.search(org.search_offset.get_untracked()+50)>"Próxima página"</button>
            </div>
        </Show>
    </section> }.into_any()
}

#[component]
fn MemberRow(member: MemberSummary) -> impl IntoView {
    let org = use_organizations();
    let user_id = member.id.clone();
    let member_version = member.version;
    let member_email = member.email.clone();
    let tenant = org
        .state
        .get_untracked()
        .active
        .map(|a| a.organization.id)
        .unwrap_or_default();
    let draft_key = StoredValue::new((tenant, user_id.clone(), member_version));
    let draft = org
        .member_drafts
        .with_untracked(|rows| rows.get(&draft_key.get_value()).cloned());
    let value = RwSignal::new(draft.as_ref().map_or(member.role, |d| d.0));
    let until = RwSignal::new(
        draft
            .map(|d| d.1)
            .unwrap_or_else(|| civil_date(member.valid_until.as_deref())),
    );
    let active = member.active;
    let role_id = format!("org-member-role-{user_id}");
    let until_id = format!("org-member-until-{user_id}");
    let save_id = format!("org-member-save-{user_id}");
    let remove_id = format!("org-member-remove-{user_id}");
    let save_focus = save_id.clone();
    let remove_focus = remove_id.clone();
    let remove_user = user_id.clone();
    let own_id = user_id.clone();
    view! { <li><div class="org-row-heading"><strong>{member.email}<Show when=move ||org.state.get().current_user_id==own_id><span>" (você)"</span></Show></strong><span class="status-badge">{if active {member.role.label()} else {"Participação encerrada"}}</span></div>
        <p class="org-row-meta">{member.valid_until.as_ref().map(|v|format!("Válido até {} (Brasília).",instant(v))).unwrap_or_else(||"Sem data de término definida.".into())}</p>
        <Show when=move ||active && org.administrator()>
            <div class="org-member-edit">
                <div><label for=role_id.clone()>"Participação"</label><select id=role_id.clone() disabled=move ||org.busy.get() prop:value=move ||value.get().as_str()
                    on:change=move |event| {value.set(role(&event_target_value(&event)));if value.get_untracked()==OrganizationRole::Administrator{until.set(String::new());}org.member_edits.set(true);org.member_drafts.update(|rows|{rows.insert(draft_key.get_value(),(value.get_untracked(),until.get_untracked()));});}>
                    <option value="member">"Membro"</option><option value="administrator">"Administrador"</option>
                </select></div>
                <div><label for=until_id.clone()>"Último dia, opcional (Brasília)"</label><input id=until_id.clone() type="date" disabled=move ||org.busy.get() ||value.get()==OrganizationRole::Administrator prop:value=move ||until.get()
                    on:input=move |event|{until.set(event_target_value(&event));org.member_edits.set(true);org.member_drafts.update(|rows|{rows.insert(draft_key.get_value(),(value.get_untracked(),until.get_untracked()));});}/></div>
                <div class="org-row-actions"><button id=save_id.clone() type="button" class="secondary-button" disabled=move ||org.blocked()
                    on:click={let user_id=user_id.clone();let member_email=member_email.clone();let save_focus=save_focus.clone();move |_|if let Some(scope)=org.scope(){let chosen=value.get_untracked();org.confirm(OrganizationCommand::ChangeMember{scope,user_id:user_id.clone(),member_version,role:chosen,valid_until:if chosen==OrganizationRole::Administrator{None}else{end_of_day(&until.get_untracked())}},"Alterar esta participação?",&format!("{member_email} passará a {}. Confira também a validade informada.",chosen.label()),&save_focus);}}>"Salvar participação"</button></div>
            </div>
        </Show>
        <Show when=move ||active>
            <div class="org-row-actions"><button id=remove_id.clone() type="button" class="secondary-button" disabled=move ||org.blocked()
                on:click={let remove_user=remove_user.clone();let remove_focus=remove_focus.clone();move |_|if let Some(scope)=org.scope(){org.confirm(OrganizationCommand::RemoveMember{scope,user_id:remove_user.clone(),member_version},"Encerrar esta participação?","O acesso à empresa será encerrado. A empresa precisa conservar pelo menos um administrador.",&remove_focus);}}>"Encerrar participação"</button></div>
        </Show>
    </li> }.into_any()
}

#[component]
fn OrganizationSettings() -> impl IntoView {
    let org = use_organizations();
    view! { <section class="org-card" aria-labelledby="org-settings-heading">
        <h3 id="org-settings-heading">"Dados e acesso da empresa"</h3>
        <details id="org-rename-details"><summary>"Alterar o nome da empresa"</summary>
            <form method="post" action="/organizacoes" on:submit=move |event|{event.prevent_default();let Some(scope)=org.scope()else{return;};let name=org.rename_name.get_untracked();if name.trim().is_empty(){org.local_error("Informe o novo nome da empresa.","org-rename-name");return;}org.submit(OrganizationCommand::Rename{scope,name});}>
                <Field id="org-rename-name" label="Novo nome da empresa" value=org.rename_name required=true/>
                <button id="org-rename-submit" class="secondary-button" type="submit" disabled=move ||org.blocked()>"Salvar nome"</button>
            </form>
        </details>
        <details id="org-code-details"><summary>"Gerar novo código para pedidos de acesso"</summary>
            <p>"O código anterior deixará de receber novos pedidos. As solicitações já recebidas continuam para análise."</p>
            <div class="org-row-actions"><button id="org-rotate-access-code" type="button" class="secondary-button" disabled=move ||org.blocked()
                on:click=move |_|if let Some(scope)=org.scope(){org.confirm(OrganizationCommand::RotateAccessCode{scope},"Substituir o código de acesso?","O código anterior deixará de aceitar novos pedidos. Compartilhe o novo código somente com as pessoas desejadas.","org-rotate-access-code");}>"Gerar novo código"</button></div>
        </details>
    </section> }.into_any()
}

#[component]
pub(super) fn ConfirmationDialog() -> impl IntoView {
    let org = use_organizations();
    view! { <dialog id="org-confirm-dialog" class="org-confirm-dialog" aria-labelledby="org-confirm-title" aria-describedby="org-confirm-description" on:keydown=super::trap_dialog_tab on:cancel=move |event: leptos::ev::Event|{event.prevent_default();org.dismiss();}>
        <h2 id="org-confirm-title">{move ||org.pending.get().map(|p|p.title).unwrap_or_default()}</h2>
        <p id="org-confirm-description">{move ||org.pending.get().map(|p|p.description).unwrap_or_default()}</p>
        <div class="form-actions"><button id="org-confirm-cancel" class="secondary-button" type="button" on:click=move |_|org.dismiss()>"Voltar sem alterar"</button>
            <button id="org-confirm-submit" class="primary-button" type="button" on:click=move |_|org.confirm_pending()>"Confirmar"</button></div>
    </dialog> }.into_any()
}
