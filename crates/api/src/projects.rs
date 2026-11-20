use crate::{
    auth::{AuthHttp, application_failure, failure, private_headers, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Form, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use canteiro_contracts::{projects::*, reliability::ReliabilityScope};
use canteiro_domain::EntityId;
use serde::Deserialize;
use std::collections::BTreeMap;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/projects/context", get(context))
        .route("/api/projects/command", post(command))
        .route("/api/projects/{project}/history", get(history))
        .route("/projetos", get(page))
        .route("/projetos/acao", post(form))
        .layer(DefaultBodyLimit::max(600_000))
}

pub(crate) fn authenticated(
    auth: Option<Extension<AuthHttp>>,
    headers: &HeaderMap,
    mutation: bool,
) -> Result<(AuthHttp, String), Box<Response>> {
    let Some(Extension(auth)) = auth else {
        return Err(Box::new(unavailable()));
    };
    let token =
        authorize_http(&auth, headers, mutation).map_err(|r| Box::new(r.into_response()))?;
    Ok((auth, token))
}
pub(crate) fn form_headers(
    mut headers: HeaderMap,
    data: &BTreeMap<String, String>,
) -> Result<HeaderMap, Box<Response>> {
    let Some(token) = data.get("csrf").and_then(|s| HeaderValue::from_str(s).ok()) else {
        return Err(Box::new(failure(
            StatusCode::FORBIDDEN,
            "csrf_invalid",
            "Atualize a página antes de enviar o formulário.",
        )));
    };
    headers.insert("x-csrf-token", token);
    Ok(headers)
}
pub(crate) fn html_page(content: String, status: StatusCode) -> Response {
    let mut r = Html(content).into_response();
    *r.status_mut() = status;
    private_headers(&mut r);
    r
}
pub(crate) fn bad_input() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira os campos. Nenhuma alteração foi gravada.",
    )
}
pub(crate) fn field<'a>(form: &'a BTreeMap<String, String>, key: &str) -> &'a str {
    form.get(key).map(String::as_str).unwrap_or("")
}
pub(crate) fn form_context(form: &BTreeMap<String, String>) -> Result<ReliabilityScope, String> {
    Ok(ReliabilityScope {
        organization_id: field(form, "organization_id").to_owned(),
        context_version: field(form, "context_version")
            .parse()
            .map_err(|_| "Atualize a empresa ativa antes de salvar.".to_owned())?,
    })
}
pub(crate) fn optional_id(
    form: &BTreeMap<String, String>,
    name: &str,
) -> Result<Option<EntityId>, String> {
    let value = field(form, name);
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| "Selecione um vínculo válido da lista.".into())
    }
}
pub(crate) fn date(
    form: &BTreeMap<String, String>,
    name: &str,
) -> Result<Option<canteiro_domain::CivilDate>, String> {
    let value = field(form, name);
    if value.is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| "Informe uma data válida no campo indicado.".into())
    }
}

async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    match auth.service().projects_context(&token).await {
        Ok(ctx) => private_json(ctx),
        Err(e) => application_failure(e),
    }
}
async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<ProjectCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth.service().projects_execute(&token, input).await {
        Ok(reply) => private_json(reply),
        Err(e) => application_failure(e),
    }
}
async fn history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(project): Path<String>,
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
        .projects_history(&token, &scope, &project)
        .await
    {
        Ok(rows) => private_json(rows),
        Err(e) => application_failure(e),
    }
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    project: Option<String>,
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
    let ctx = match auth.service().projects_context(&token).await {
        Ok(v) => v,
        Err(e) => return application_failure(e),
    };
    if q.project
        .as_ref()
        .is_some_and(|id| !ctx.projects.iter().any(|p| &p.id == id))
    {
        return failure(
            StatusCode::NOT_FOUND,
            "record_unavailable",
            "Este projeto não está disponível para seu acesso.",
        );
    }
    let history = if q.history == Some(true) {
        if let (Some(scope), Some(id)) = (&ctx.context, &q.project) {
            match auth.service().projects_history(&token, scope, id).await {
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
        canteiro_web::projects::page(
            ctx,
            q.project.as_deref(),
            history,
            BTreeMap::new(),
            None,
            uuid::Uuid::new_v4().to_string(),
        ),
        StatusCode::OK,
    )
}
fn form_command(
    data: &BTreeMap<String, String>,
    ctx: &ProjectsContext,
) -> Result<ProjectCommand, String> {
    let project_id = field(data, "project_id");
    let current = if project_id.is_empty() {
        None
    } else {
        Some(
            ctx.projects
                .iter()
                .find(|p| p.id == project_id)
                .ok_or("Este projeto não está disponível para seu acesso.")?,
        )
    };
    let reapply = field(data, "reapply") == "true";
    let expected = if reapply {
        current.map(|p| p.version)
    } else {
        let raw = field(data, "expected_version");
        if raw.is_empty() {
            None
        } else {
            Some(
                raw.parse()
                    .map_err(|_| "Confira a versão salva antes de reaplicar.")?,
            )
        }
    };
    let key = if reapply {
        uuid::Uuid::new_v4().to_string()
    } else {
        field(data, "operation_key").to_owned()
    };
    let change = match field(data, "action") {
        "save" => {
            let stakeholder_ids = data
                .iter()
                .filter(|(key, _)| key.starts_with("stakeholder_"))
                .map(|(_, value)| {
                    value
                        .parse::<EntityId>()
                        .map_err(|_| "Selecione uma parte interessada válida.".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            ProjectChange::Save {
                spec: ProjectSpec {
                    title: field(data, "title").into(),
                    discipline: field(data, "discipline").into(),
                    objective: field(data, "objective").into(),
                    scope: field(data, "scope").into(),
                    external_reference: field(data, "external_reference").into(),
                    work_id: optional_id(data, "work_id")?,
                    stakeholder_ids,
                    deliveries: current
                        .map(|p| p.snapshot.spec.deliveries.clone())
                        .unwrap_or_default(),
                },
            }
        }
        "delivery" => {
            let delivery_id = if field(data, "delivery_id").is_empty() {
                key.as_str()
            } else {
                field(data, "delivery_id")
            };
            ProjectChange::SetDelivery {
                delivery: ProjectDelivery {
                    id: delivery_id
                        .parse()
                        .map_err(|_| "Atualize a página antes de incluir a entrega.")?,
                    title: field(data, "delivery_title").into(),
                    responsible_id: optional_id(data, "responsible_id")?,
                    acceptance_criteria: field(data, "acceptance_criteria").into(),
                    due_on: date(data, "due_on")?,
                    milestone: field(data, "milestone") == "true",
                },
            }
        }
        "remove" => ProjectChange::RemoveDelivery {
            delivery_id: field(data, "delivery_id").into(),
        },
        "approve" => ProjectChange::Approve,
        "start" => ProjectChange::Start,
        "accept" => ProjectChange::Accept {
            delivery_id: field(data, "delivery_id").into(),
        },
        "close" => ProjectChange::Close,
        _ => return Err("Selecione uma ação disponível.".into()),
    };
    Ok(ProjectCommand {
        context: form_context(data)?,
        operation_key: key,
        project_id: (!project_id.is_empty()).then(|| project_id.to_owned()),
        expected_version: expected,
        reason: field(data, "reason").into(),
        change,
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
        Ok(h) => h,
        Err(r) => return *r,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(v) => v,
        Err(e) => return *e,
    };
    let ctx = match auth.service().projects_context(&token).await {
        Ok(v) => v,
        Err(e) => return application_failure(e),
    };
    let result = match form_command(&data, &ctx) {
        Ok(command) => auth
            .service()
            .projects_execute(&token, command)
            .await
            .map_err(|e| e.message().to_owned()),
        Err(e) => Err(e),
    };
    match result {
        Ok(reply) => {
            let mut r =
                Redirect::to(&format!("/projetos?project={}", reply.project.id)).into_response();
            private_headers(&mut r);
            r
        }
        Err(message) => {
            let latest = auth.service().projects_context(&token).await.unwrap_or(ctx);
            let selection = (!field(&data, "project_id").is_empty())
                .then(|| field(&data, "project_id").to_owned());
            let key = field(&data, "operation_key").to_owned();
            html_page(
                canteiro_web::projects::page(
                    latest,
                    selection.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_delivery_retries_keep_the_same_identity() {
        let ctx = ProjectsContext {
            csrf: String::new(),
            context: None,
            organization_name: String::new(),
            can_create: false,
            projects: vec![],
            professionals: vec![],
            parties: vec![],
            works: vec![],
        };
        let key = uuid::Uuid::new_v4().to_string();
        let data = BTreeMap::from([
            ("operation_key".into(), key.clone()),
            ("context_version".into(), "1".into()),
            ("action".into(), "delivery".into()),
        ]);
        let a = form_command(&data, &ctx).unwrap();
        let b = form_command(&data, &ctx).unwrap();
        match (a.change, b.change) {
            (
                ProjectChange::SetDelivery { delivery: a },
                ProjectChange::SetDelivery { delivery: b },
            ) => assert_eq!(a.id, b.id),
            _ => panic!("entrega esperada"),
        }
    }
}
