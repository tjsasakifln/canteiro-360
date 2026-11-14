//! HTTP do histórico contratual. As consultas também validam sessão e escopo.
use crate::{
    auth::{AuthHttp, application_failure, failure, private_headers, unavailable},
    organizations::{authorize_http, private_json},
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, Query, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use canteiro_contracts::reliability::{
    HistoryEntry, ReliabilityCommand, ReliabilityReply, ReliabilityScope,
};
use serde::Deserialize;

pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new()
        .route("/api/reliability/context", get(context))
        .route("/api/reliability/command", post(command))
        .route("/api/reliability/contracts/{contract}", get(detail))
        .route(
            "/api/reliability/contracts/{contract}/history.csv",
            get(history),
        )
        .route(
            "/api/reliability/contracts/{contract}/operations/{key}",
            get(operation),
        )
        .layer(DefaultBodyLimit::max(4_096))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailQuery {
    organization_id: String,
    context_version: i64,
    after: Option<i64>,
}

fn authenticated(
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

fn invalid_input() -> Response {
    failure(
        StatusCode::BAD_REQUEST,
        "invalid_input",
        "Confira os campos e a referência do contrato.",
    )
}

async fn context(auth: Option<Extension<AuthHttp>>, headers: HeaderMap) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match auth.service().reliability_context(&token).await {
        Ok(value) => private_json(value),
        Err(error) => application_failure(error),
    }
}

async fn detail(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(contract): Path<String>,
    query: Result<Query<DetailQuery>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(DetailQuery {
        organization_id,
        context_version,
        after,
    })) = query
    else {
        return invalid_input();
    };
    let scope = ReliabilityScope {
        organization_id,
        context_version,
    };
    match auth
        .service()
        .reliability_detail(&token, &scope, &contract, after)
        .await
    {
        Ok(value) => private_json(value),
        Err(error) => application_failure(error),
    }
}

async fn operation(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path((contract, key)): Path<(String, String)>,
    query: Result<Query<ReliabilityScope>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(scope)) = query else {
        return invalid_input();
    };
    match auth
        .service()
        .reliability_operation(&token, &scope, &contract, &key)
        .await
    {
        Ok(value) => private_json(value),
        Err(error) => application_failure(error),
    }
}

async fn command(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    input: Result<Json<ReliabilityCommand>, JsonRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, true) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Json(command)) = input else {
        return invalid_input();
    };
    match auth.service().reliability_execute(&token, command).await {
        Ok(reply @ ReliabilityReply::VersionConflict { .. }) => {
            let mut response = private_json(reply);
            *response.status_mut() = StatusCode::CONFLICT;
            response
        }
        Ok(reply) => private_json(reply),
        Err(error) => application_failure(error),
    }
}

async fn history(
    auth: Option<Extension<AuthHttp>>,
    headers: HeaderMap,
    Path(contract): Path<String>,
    query: Result<Query<ReliabilityScope>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let (auth, token) = match authenticated(auth, &headers, false) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Ok(Query(scope)) = query else {
        return invalid_input();
    };
    match auth
        .service()
        .reliability_history(&token, &scope, &contract)
        .await
    {
        Ok(rows) => {
            let mut response = history_report(&rows).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/csv; charset=utf-8"),
            );
            response.headers_mut().insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=historico-do-contrato.csv"),
            );
            private_headers(&mut response);
            response
        }
        Err(error) => application_failure(error),
    }
}

fn history_report(rows: &[HistoryEntry]) -> String {
    let mut csv = String::from(
        "\u{feff}Versão;Evento;Identificação do autor;Autor;Origem;Ato;Título anterior;Título salvo;Motivo;Ocorrido em (UTC);Registrado em (UTC);Registro posterior ao fato\r\n",
    );
    for row in rows {
        let version = row.version.to_string();
        let action = match row.action.as_str() {
            "contract_reference_created" => "Cadastro da referência",
            "contract_reference_renamed" => "Alteração de título",
            other => other,
        };
        let fields = [
            version.as_str(),
            &row.event_id,
            &row.actor_id,
            &row.actor_label,
            &row.origin,
            action,
            row.before.as_deref().unwrap_or(""),
            &row.after,
            &row.reason,
            &row.occurred_at,
            &row.recorded_at,
            if row.late { "Sim" } else { "Não" },
        ];
        for (index, value) in fields.iter().enumerate() {
            if index > 0 {
                csv.push(';');
            }
            csv.push('"');
            // Aspas isolam separadores; o apóstrofo impede execução de fórmula
            // quando o arquivo for aberto por uma planilha.
            if value
                .trim_start_matches(|c: char| {
                    c.is_whitespace() || c.is_control() || c == '\u{feff}'
                })
                .starts_with(['=', '+', '-', '@', '＝', '＋', '－', '＠'])
            {
                csv.push('\'');
            }
            for c in value.chars() {
                if c == '"' {
                    csv.push('"');
                }
                csv.push(c);
            }
            csv.push('"');
        }
        csv.push_str("\r\n");
    }
    csv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapes_cells_and_disables_spreadsheet_formulas() {
        let report = history_report(&[HistoryEntry {
            sequence: 2,
            event_id: "evento".into(),
            actor_id: "autor".into(),
            actor_label: "contato@example.test".into(),
            version: 2,
            origin: "web".into(),
            action: "contract_reference_renamed".into(),
            before: Some("Antigo; \"certo\"".into()),
            after: "   =SUM(1;2)".into(),
            reason: "＠SOMA(A1:A2)".into(),
            occurred_at: "2026-11-07T13:00:00+00:00".into(),
            recorded_at: "2026-11-14T13:00:00+00:00".into(),
            late: true,
        }]);
        assert!(report.contains("\"Antigo; \"\"certo\"\"\";\"'   =SUM(1;2)\";\"'＠SOMA(A1:A2)\""));
        assert!(
            report.contains(
                "\"2026-11-07T13:00:00+00:00\";\"2026-11-14T13:00:00+00:00\";\"Sim\"\r\n"
            )
        );
    }
}
