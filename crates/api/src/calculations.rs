use axum::{
    Json,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use canteiro_contracts::{
    calculations::{AllocationRequest, CalendarRequest, preview_allocation, preview_calendar},
    forms::FieldError,
};

fn malformed() -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, Json(FieldError {
        code: "input".into(),
        message: "Confira os campos e a versão da regra. Valores decimais devem ser textos com ponto, sem separador de milhar.".into(),
    })).into_response()
}

pub async fn allocation(body: Result<Json<AllocationRequest>, JsonRejection>) -> Response {
    let Ok(Json(input)) = body else {
        return malformed();
    };
    match preview_allocation(&input) {
        Ok(result) => Json(result).into_response(),
        Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, Json(error)).into_response(),
    }
}

pub async fn calendar(body: Result<Json<CalendarRequest>, JsonRejection>) -> Response {
    let Ok(Json(input)) = body else {
        return malformed();
    };
    match preview_calendar(&input) {
        Ok(result) => Json(result).into_response(),
        Err(error) => (StatusCode::UNPROCESSABLE_ENTITY, Json(error)).into_response(),
    }
}
