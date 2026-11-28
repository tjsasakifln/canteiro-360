use crate::{
    auth::{AuthHttp, application_failure},
    organizations::private_json,
    projects::{authenticated, bad_input, field, form_context, form_headers, html_page},
};
use axum::{
    Extension, Form, Json, Router,
    extract::{DefaultBodyLimit, Query, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use canteiro_application::auth::AuthErrorKind;
use canteiro_contracts::notifications::*;
use canteiro_web::notifications::NoticePreferencesDraft;
use std::collections::BTreeMap;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/notices/context", get(context))
        .route("/api/notices/preferences", post(preferences))
        .route("/pendencias", get(page))
        .route("/pendencias/preferencias", post(form))
        .layer(DefaultBodyLimit::max(8_192))
}
async fn context(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Query<NoticePage>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(input)) = input else {
        return bad_input();
    };
    let page = input.page.unwrap_or(1);
    match auth.service().notices_page(&token, page).await {
        Ok((value, has_more)) => {
            let mut response = private_json(value);
            if let Some(link) = notice_page_links(page, has_more) {
                response.headers_mut().insert(axum::http::header::LINK, link);
            }
            response
        }
        Err(error) => application_failure(error),
    }
}

/// Mantém o corpo JSON existente e publica a navegação no cabeçalho Link.
fn notice_page_links(page: u32, has_more: bool) -> Option<axum::http::HeaderValue> {
    let mut links = Vec::new();
    if let Some(previous) = page.checked_sub(1).filter(|value| *value > 0) {
        links.push(format!(
            "</api/notices/context?page={previous}>; rel=\"prev\""
        ));
    }
    if let Some(next) = page.checked_add(1).filter(|_| has_more) {
        links.push(format!("</api/notices/context?page={next}>; rel=\"next\""));
    }
    if links.is_empty() {
        return None;
    }
    axum::http::HeaderValue::from_str(&links.join(", ")).ok()
}
async fn preferences(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<NoticePreferencesCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Json(input)) = input else {
        return bad_input();
    };
    match auth.service().notices_set_preferences(&token, input).await {
        Ok(value) => private_json(value),
        Err(error) => application_failure(error),
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NoticePage {
    page: Option<u32>,
}

async fn page(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Query<NoticePage>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(input)) = input else {
        return bad_input();
    };
    let page = input.page.unwrap_or(1);
    match auth.service().notices_page(&token, page).await {
        Ok((value, has_more)) => html_page(
            canteiro_web::notifications::page_with_pagination(
                value,
                None,
                uuid::Uuid::new_v4().to_string(),
                None,
                page,
                has_more,
            ),
            StatusCode::OK,
        ),
        Err(error) => application_failure(error),
    }
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
        Err(e) => return *e,
    };
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let parsed = (|| -> Result<NoticePreferencesCommand, String> {
        let context = form_context(&data)?;
        let expected_version = preference_version(&data)?;
        let text = field(&data, "reminder_days").trim();
        let reminder_days = if text.is_empty() {
            vec![]
        } else {
            text.split(',')
                .map(|part| {
                    part.trim()
                        .parse::<u16>()
                        .map_err(|_| "Informe dias inteiros, separados por vírgula.".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        Ok(NoticePreferencesCommand {
            context,
            expected_version,
            settings: NoticePreferences {
                mute_optional_email: field(&data, "mute_optional_email") == "true",
                group_by_day: field(&data, "group_by_day") == "true",
                reminder_days,
            },
        })
    })();
    let command = match parsed {
        Ok(v) => v,
        Err(message) => {
            return invalid_preferences(&auth, &token, &data, message).await;
        }
    };
    match auth
        .service()
        .notices_set_preferences(&token, command)
        .await
    {
        Ok(_) => Redirect::to("/pendencias").into_response(),
        Err(error) if error.kind() == AuthErrorKind::InvalidInput => {
            invalid_preferences(&auth, &token, &data, error.message().to_owned()).await
        }
        Err(error) => application_failure(error),
    }
}

fn preference_version(data: &BTreeMap<String, String>) -> Result<Option<i64>, String> {
    let raw = field(data, "expected_version");
    if raw.is_empty() {
        Ok(None)
    } else {
        raw.parse()
            .map(Some)
            .map_err(|_| "Confira a versão atual das preferências.".to_owned())
    }
}

async fn invalid_preferences(
    auth: &AuthHttp,
    token: &str,
    data: &BTreeMap<String, String>,
    message: String,
) -> Response {
    let (ctx, has_more) = match auth.service().notices_page(token, 1).await {
        Ok(result) => result,
        Err(error) => return application_failure(error),
    };
    let (draft, message) = if form_context(data).ok().as_ref() == ctx.context.as_ref()
        && ctx.context.is_some()
        && preference_version(data) == Ok(ctx.preferences.version)
    {
        (
            Some(NoticePreferencesDraft {
                mute_optional_email: field(data, "mute_optional_email") == "true",
                group_by_day: field(data, "group_by_day") == "true",
                reminder_days: field(data, "reminder_days").to_owned(),
            }),
            message,
        )
    } else {
        (
            None,
            format!(
                "{message} A empresa ativa ou a versão mudou. Confira as preferências atuais antes de editar."
            ),
        )
    };
    html_page(
        canteiro_web::notifications::page_with_pagination(
            ctx,
            Some(message),
            uuid::Uuid::new_v4().to_string(),
            draft,
            1,
            has_more,
        ),
        StatusCode::BAD_REQUEST,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notice_api_links_preserve_the_existing_json_and_expose_all_pages() {
        assert!(notice_page_links(1, false).is_none());
        assert_eq!(
            notice_page_links(1, true).unwrap(),
            "</api/notices/context?page=2>; rel=\"next\""
        );
        assert_eq!(
            notice_page_links(2, true).unwrap(),
            "</api/notices/context?page=1>; rel=\"prev\", </api/notices/context?page=3>; rel=\"next\""
        );
        assert_eq!(
            notice_page_links(3, false).unwrap(),
            "</api/notices/context?page=2>; rel=\"prev\""
        );
        assert!(
            notice_page_links(u32::MAX, true)
                .unwrap()
                .to_str()
                .unwrap()
                .ends_with("rel=\"prev\"")
        );
    }
}
