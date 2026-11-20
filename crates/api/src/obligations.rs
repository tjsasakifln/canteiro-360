use crate::{
    auth::{AuthHttp, application_failure, failure, private_headers},
    organizations::private_json,
    projects::{
        authenticated, bad_input, date, field, form_context, form_headers, html_page, optional_id,
    },
};
use axum::{
    Extension, Form, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use canteiro_contracts::{obligations::*, reliability::ReliabilityScope};
use serde::Deserialize;
use std::collections::BTreeMap;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/obligations/context", get(context))
        .route("/api/obligations/command", post(command))
        .route("/api/obligations/{obligation}/history", get(history))
        .route("/obrigacoes", get(page))
        .route("/obrigacoes/acao", post(form))
        .layer(DefaultBodyLimit::max(40_000))
}
async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    match auth.service().obligations_context(&token).await {
        Ok(ctx) => private_json(ctx),
        Err(e) => application_failure(e),
    }
}
async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<ObligationCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth.service().obligations_execute(&token, input).await {
        Ok(r) => private_json(r),
        Err(e) => application_failure(e),
    }
}
async fn history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(obligation): Path<String>,
    query: Result<Query<ReliabilityScope>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(scope)) = query else {
        return bad_input();
    };
    match auth
        .service()
        .obligations_history(&token, &scope, &obligation)
        .await
    {
        Ok(r) => private_json(r),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    obligation: Option<String>,
    replace: Option<String>,
    history: Option<bool>,
}
async fn page(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    query: Result<Query<PageQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Query(q)) = query else {
        return bad_input();
    };
    let ctx = match auth.service().obligations_context(&token).await {
        Ok(v) => v,
        Err(e) => return application_failure(e),
    };
    if q.obligation
        .iter()
        .chain(q.replace.iter())
        .any(|id| !ctx.obligations.iter().any(|o| &o.id == id))
    {
        return failure(
            StatusCode::NOT_FOUND,
            "record_unavailable",
            "Esta obrigação não está disponível para seu acesso.",
        );
    }
    let history = if q.history == Some(true) {
        if let (Some(scope), Some(id)) = (&ctx.context, &q.obligation) {
            match auth.service().obligations_history(&token, scope, id).await {
                Ok(v) => v,
                Err(e) => return application_failure(e),
            }
        } else {
            vec![]
        }
    } else {
        vec![]
    };
    html_page(
        canteiro_web::obligations::page(
            ctx,
            q.obligation.as_deref(),
            q.replace.as_deref(),
            history,
            BTreeMap::new(),
            None,
            uuid::Uuid::new_v4().to_string(),
        ),
        StatusCode::OK,
    )
}
fn rrt_timing(data: &BTreeMap<String, String>) -> Result<Option<RrtTiming>, String> {
    let event = match field(data, "rrt_event") {
        "" => return Ok(None),
        "project_completion" => RrtEvent::ProjectCompletion,
        "final_delivery" => RrtEvent::FinalDelivery,
        "submission" => RrtEvent::Submission,
        "publication" => RrtEvent::Publication,
        "other_activity_completion" => RrtEvent::OtherActivityCompletion,
        _ => return Err("Selecione o evento aplicável ao prazo do RRT.".into()),
    };
    let framework = match field(data, "rrt_framework") {
        "project" => RrtFramework::Project,
        "environment_planning" => RrtFramework::EnvironmentPlanning,
        "project_coordination" => RrtFramework::ProjectCoordination,
        "safety_system_design" => RrtFramework::SafetySystemDesign,
        "fire_protection_design" => RrtFramework::FireProtectionDesign,
        "other_activities" => RrtFramework::OtherActivities,
        _ => {
            return Err(
                "Informe o grupo ou atividade conforme o RRT e a Resolução CAU 21/2012.".into(),
            );
        }
    };
    Ok(Some(RrtTiming {
        framework,
        event,
        event_on: date(data, "rrt_event_on")?
            .ok_or("Informe a data do evento aplicável ao RRT.")?,
    }))
}
fn form_command(
    data: &BTreeMap<String, String>,
    ctx: &ObligationsContext,
) -> Result<ObligationCommand, String> {
    let record_id = field(data, "obligation_id");
    let current = if record_id.is_empty() {
        None
    } else {
        Some(
            ctx.obligations
                .iter()
                .find(|o| o.id == record_id)
                .ok_or("Esta obrigação não está disponível para seu acesso.")?,
        )
    };
    let reapply = field(data, "reapply") == "true";
    let expected = if reapply {
        current.map(|o| o.version)
    } else {
        let raw = field(data, "expected_version");
        if raw.is_empty() {
            None
        } else {
            Some(
                raw.parse()
                    .map_err(|_| "Confira a versão antes de salvar.")?,
            )
        }
    };
    let approve = field(data, "action") == "approve_exception";
    let spec = if approve {
        current
            .ok_or("Selecione a exceção apresentada.")?
            .spec
            .clone()
    } else {
        let kind = match field(data, "kind") {
            "art" => ObligationKind::Art,
            "rrt" => ObligationKind::Rrt,
            "designation" => ObligationKind::Designation,
            "license" => ObligationKind::License,
            "insurance" => ObligationKind::Insurance,
            "guarantee" => ObligationKind::Guarantee,
            "cno" => ObligationKind::Cno,
            _ => return Err("Selecione a natureza do documento.".into()),
        };
        let activity = match field(data, "activity") {
            "budget" => TechnicalActivity::Budget,
            "design" => TechnicalActivity::Design,
            "execution" => TechnicalActivity::Execution,
            "inspection" => TechnicalActivity::Inspection,
            "other" => TechnicalActivity::Other,
            _ => return Err("Selecione a atividade coberta pelo documento.".into()),
        };
        let (document_id, document_version) = if field(data, "document").is_empty() {
            (None, None)
        } else {
            let (raw, version) = field(data, "document")
                .split_once(':')
                .ok_or("Selecione uma versão de arquivo disponível.")?;
            (
                Some(raw.parse().map_err(|_| "Selecione um documento válido.")?),
                Some(
                    version
                        .parse()
                        .map_err(|_| "Selecione uma versão válida.")?,
                ),
            )
        };
        let exception = if ["source_reference", "source_url", "reasoning"]
            .iter()
            .any(|key| !field(data, key).trim().is_empty())
        {
            Some(TechnicalException {
                source_reference: field(data, "source_reference").into(),
                source_url: field(data, "source_url").into(),
                reasoning: field(data, "reasoning").into(),
            })
        } else {
            None
        };
        ObligationSpec {
            kind,
            activity,
            discipline: field(data, "discipline").into(),
            description: field(data, "description").into(),
            professional_id: optional_id(data, "professional_id")?,
            number: field(data, "number").into(),
            issuer: field(data, "issuer").into(),
            valid_from: date(data, "valid_from")?.ok_or("Informe o início da vigência.")?,
            valid_until: date(data, "valid_until")?,
            activity_on: date(data, "activity_on")?.ok_or("Informe a data da atividade.")?,
            registered_on: date(data, "registered_on")?,
            rrt_timing: rrt_timing(data)?,
            document_id,
            document_version,
            exception,
            replaces_id: optional_id(data, "replaces_id")?,
        }
    };
    Ok(ObligationCommand {
        context: form_context(data)?,
        operation_key: if reapply {
            uuid::Uuid::new_v4().to_string()
        } else {
            field(data, "operation_key").into()
        },
        scope_id: field(data, "scope_id").into(),
        obligation_id: (!record_id.is_empty()).then(|| record_id.to_owned()),
        expected_version: expected,
        reason: field(data, "reason").into(),
        spec,
        confirm: approve || (!reapply && field(data, "confirm") == "true"),
    })
}
async fn form(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Form<BTreeMap<String, String>>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Ok(Form(data)) = input else {
        return bad_input();
    };
    let headers = match form_headers(headers, &data) {
        Ok(v) => v,
        Err(r) => return *r,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let ctx = match auth.service().obligations_context(&token).await {
        Ok(v) => v,
        Err(e) => return application_failure(e),
    };
    let result = match form_command(&data, &ctx) {
        Ok(command) => auth
            .service()
            .obligations_execute(&token, command)
            .await
            .map_err(|e| e.message().to_owned()),
        Err(e) => Err(e),
    };
    match result {
        Ok(reply) => {
            let mut response =
                Redirect::to(&format!("/obrigacoes?obligation={}", reply.obligation.id))
                    .into_response();
            private_headers(&mut response);
            response
        }
        Err(message) => {
            let latest = auth
                .service()
                .obligations_context(&token)
                .await
                .unwrap_or(ctx);
            let selected = (!field(&data, "obligation_id").is_empty())
                .then(|| field(&data, "obligation_id").to_owned());
            let replacement = (!field(&data, "replaces_id").is_empty())
                .then(|| field(&data, "replaces_id").to_owned());
            let key = field(&data, "operation_key").to_owned();
            html_page(
                canteiro_web::obligations::page(
                    latest,
                    selected.as_deref(),
                    replacement.as_deref(),
                    vec![],
                    data,
                    Some(message),
                    key,
                ),
                StatusCode::UNPROCESSABLE_ENTITY,
            )
        }
    }
}
