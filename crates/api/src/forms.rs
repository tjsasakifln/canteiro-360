//! Bounded, stateless demonstration endpoints. No database writes, permissions
//! or financial commands are implied by these publicly fictitious examples.
use axum::{
    Json,
    extract::{
        Path, Query,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use canteiro_contracts::forms::{
    FieldError, MeasurementForm, MeasurementQuery, demo_export, demo_page,
    validate_measurement_form,
};

fn error_response(error: FieldError) -> Response {
    let status = if error.code == "not_found" {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    (status, Json(error)).into_response()
}

fn query_error() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(FieldError {
            code: "query".into(),
            message:
                "Não foi possível interpretar os filtros. Confira a página, a busca e a ordenação."
                    .into(),
        }),
    )
        .into_response()
}

pub async fn page(
    Path(organization): Path<String>,
    query: Result<Query<MeasurementQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return query_error();
    };
    match demo_page(&organization, &query) {
        Ok(page) => Json(page).into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn export(
    Path(organization): Path<String>,
    query: Result<Query<MeasurementQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return query_error();
    };
    match demo_export(&organization, &query) {
        Ok(tsv) => (
            [
                (
                    header::CONTENT_TYPE,
                    "text/tab-separated-values; charset=utf-8",
                ),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=medicao-demonstrativa.tsv",
                ),
            ],
            tsv,
        )
            .into_response(),
        Err(error) => error_response(error),
    }
}

pub async fn preview(
    Path(organization): Path<String>,
    body: Result<Json<MeasurementForm>, JsonRejection>,
) -> Response {
    let Ok(Json(form)) = body else {
        return (StatusCode::UNPROCESSABLE_ENTITY, Json(FieldError {
            code: "form".into(), message: "Não foi possível interpretar o formulário. Confira os campos e tente novamente.".into(),
        })).into_response();
    };
    match validate_measurement_form(&organization, &form) {
        Ok(preview) => {
            let status = if preview.errors.is_empty() {
                StatusCode::OK
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            (status, Json(preview)).into_response()
        }
        Err(error) => error_response(error),
    }
}
