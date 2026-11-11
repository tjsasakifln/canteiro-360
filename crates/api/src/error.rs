//! Adaptação HTTP do contrato seguro, sem aceitar mensagens internas arbitrárias.
use axum::{
    Json,
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use canteiro_contracts::{ErrorCode, ErrorField, PublicError};
use canteiro_persistence::{PersistenceError, PersistenceErrorKind};

#[derive(Debug)]
pub struct ApiError(PublicError);

impl ApiError {
    pub fn new(error: PublicError) -> Self {
        Self(error)
    }
}

impl From<PublicError> for ApiError {
    fn from(error: PublicError) -> Self {
        Self::new(error)
    }
}

impl From<PersistenceError> for ApiError {
    fn from(error: PersistenceError) -> Self {
        let (code, field) = persistence_contract(error.kind());
        Self::new(PublicError::new(code, error.operation_id(), field))
    }
}

fn persistence_contract(kind: PersistenceErrorKind) -> (ErrorCode, Option<ErrorField>) {
    match kind {
        PersistenceErrorKind::InvalidInput => (ErrorCode::InvalidInput, None),
        PersistenceErrorKind::NotFound => (ErrorCode::NotFound, None),
        PersistenceErrorKind::Conflict => (ErrorCode::Conflict, None),
        PersistenceErrorKind::VersionConflict => {
            (ErrorCode::Conflict, Some(ErrorField::ExpectedVersion))
        }
        PersistenceErrorKind::Unavailable => (ErrorCode::Unavailable, None),
        PersistenceErrorKind::Timeout => (ErrorCode::Timeout, None),
        PersistenceErrorKind::Internal => (ErrorCode::Internal, None),
        PersistenceErrorKind::OutcomeUnknown => (ErrorCode::OutcomeUnknown, None),
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0.code() {
            ErrorCode::InvalidInput => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Conflict => StatusCode::CONFLICT,
            ErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Timeout => StatusCode::GATEWAY_TIMEOUT,
            ErrorCode::Internal | ErrorCode::OutcomeUnknown => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let operation = HeaderValue::from_str(&self.0.operation_id().to_string())
            .expect("OperationId sempre usa UUID ASCII válido para cabeçalho");
        let mut response = (status, Json(self.0)).into_response();
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-operation-id"), operation);
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canteiro_domain::OperationId;

    #[test]
    fn only_a_known_stale_version_marks_the_version_field() {
        assert_eq!(
            persistence_contract(PersistenceErrorKind::Conflict),
            (ErrorCode::Conflict, None)
        );
        assert_eq!(
            persistence_contract(PersistenceErrorKind::VersionConflict),
            (ErrorCode::Conflict, Some(ErrorField::ExpectedVersion))
        );
        assert_eq!(
            persistence_contract(PersistenceErrorKind::OutcomeUnknown),
            (ErrorCode::OutcomeUnknown, None)
        );
    }

    #[tokio::test]
    async fn public_errors_have_stable_status_and_matching_operation_reference() {
        let operation: OperationId = "c3333333-3333-4333-8333-333333333333".parse().unwrap();
        for (code, status) in [
            (ErrorCode::InvalidInput, StatusCode::UNPROCESSABLE_ENTITY),
            (ErrorCode::NotFound, StatusCode::NOT_FOUND),
            (ErrorCode::Conflict, StatusCode::CONFLICT),
            (ErrorCode::Unavailable, StatusCode::SERVICE_UNAVAILABLE),
            (ErrorCode::Timeout, StatusCode::GATEWAY_TIMEOUT),
            (ErrorCode::Internal, StatusCode::INTERNAL_SERVER_ERROR),
            (ErrorCode::OutcomeUnknown, StatusCode::INTERNAL_SERVER_ERROR),
        ] {
            let response = ApiError::new(PublicError::new(code, operation, None)).into_response();
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()["x-operation-id"], operation.to_string());
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert!(!response.headers().contains_key(header::RETRY_AFTER));
            let bytes = axum::body::to_bytes(response.into_body(), 4096)
                .await
                .unwrap();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["operation_id"], operation.to_string());
            assert_eq!(
                value["write_outcome"],
                if code == ErrorCode::OutcomeUnknown {
                    "unknown"
                } else {
                    "not_committed"
                }
            );
        }
    }

    #[tokio::test]
    async fn field_error_preserves_the_protocol_without_private_details() {
        let operation: OperationId = "c3333333-3333-4333-8333-333333333333".parse().unwrap();
        let response = ApiError::from(PublicError::new(
            ErrorCode::InvalidInput,
            operation,
            Some(ErrorField::CivilDate),
        ))
        .into_response();
        let bytes = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["code"], "C360_INVALID_INPUT");
        assert_eq!(value["field"], "civil_date");
        assert_eq!(value.as_object().unwrap().len(), 6);
        assert!(value.get("source").is_none());
        assert!(value.get("detail").is_none());
    }
}
