use canteiro_domain::OperationId;
use serde::{Deserialize, Serialize};

/// Códigos persistentes do protocolo. Não dependem de mensagens do PostgreSQL.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ErrorCode {
    #[serde(rename = "C360_INVALID_INPUT")]
    InvalidInput,
    #[serde(rename = "C360_NOT_FOUND")]
    NotFound,
    #[serde(rename = "C360_CONFLICT")]
    Conflict,
    #[serde(rename = "C360_UNAVAILABLE")]
    Unavailable,
    #[serde(rename = "C360_TIMEOUT")]
    Timeout,
    #[serde(rename = "C360_INTERNAL")]
    Internal,
    #[serde(rename = "C360_OUTCOME_UNKNOWN")]
    OutcomeUnknown,
}

/// Vocabulário de campos controlado pelo servidor; não recebe nomes de restrições SQL.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorField {
    Input,
    TenantId,
    EntityId,
    ExpectedVersion,
    CivilDate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteOutcome {
    /// O servidor sabe que esta operação não confirmou alterações.
    NotCommitted,
    /// A comunicação falhou durante a confirmação e o resultado precisa ser verificado.
    Unknown,
}

/// Resposta construída somente a partir de um catálogo seguro.
///
/// Não aceita mensagens, nomes de tabelas, detalhes técnicos ou erros arbitrários.
/// `OutcomeUnknown` é obrigatório quando a confirmação pode ter ocorrido.
#[derive(Clone, Debug, Serialize)]
pub struct PublicError {
    code: ErrorCode,
    field: Option<ErrorField>,
    message: &'static str,
    operation_id: OperationId,
    write_outcome: WriteOutcome,
    recovery: &'static str,
}

impl PublicError {
    pub fn new(code: ErrorCode, operation_id: OperationId, field: Option<ErrorField>) -> Self {
        let (message, recovery) = match code {
            ErrorCode::InvalidInput => (
                "Confira os dados informados. Nenhuma alteração desta operação foi gravada.",
                "Revise os dados informados e tente novamente, sem apagar os demais campos.",
            ),
            ErrorCode::NotFound => (
                "O registro não está disponível para esta operação. Nenhuma alteração foi gravada.",
                "Confira a empresa e o registro selecionados. Se necessário, peça a revisão do acesso.",
            ),
            ErrorCode::Conflict => (
                "O registro não pôde ser alterado no estado atual. Nenhuma alteração desta operação foi gravada.",
                "Confira a versão atual do registro e revise os dados antes de tentar novamente.",
            ),
            ErrorCode::Unavailable => (
                "O serviço está temporariamente indisponível. Nenhuma alteração desta operação foi gravada.",
                "Mantenha seus dados preenchidos e tente novamente em alguns instantes.",
            ),
            ErrorCode::Timeout => (
                "O tempo de espera foi excedido. Nenhuma alteração desta operação foi gravada.",
                "Mantenha seus dados preenchidos e tente novamente. Se persistir, informe o identificador da operação ao suporte.",
            ),
            ErrorCode::Internal => (
                "Não foi possível concluir a operação. Nenhuma alteração desta operação foi gravada.",
                "Mantenha seus dados preenchidos e informe o identificador da operação ao suporte.",
            ),
            ErrorCode::OutcomeUnknown => (
                "Não foi possível confirmar se a alteração foi gravada.",
                "Não repita a operação agora. Confira o registro e informe o identificador da operação ao suporte para verificar o resultado.",
            ),
        };
        Self {
            code,
            field,
            message,
            operation_id,
            write_outcome: if code == ErrorCode::OutcomeUnknown {
                WriteOutcome::Unknown
            } else {
                WriteOutcome::NotCommitted
            },
            recovery,
        }
    }

    pub fn code(&self) -> ErrorCode {
        self.code
    }

    pub fn field(&self) -> Option<ErrorField> {
        self.field
    }

    pub fn operation_id(&self) -> OperationId {
        self.operation_id
    }

    pub fn write_outcome(&self) -> WriteOutcome {
        self.write_outcome
    }

    pub fn message(&self) -> &'static str {
        self.message
    }

    pub fn recovery(&self) -> &'static str {
        self.recovery
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation() -> OperationId {
        "b2222222-2222-4222-8222-222222222222".parse().unwrap()
    }

    #[test]
    fn public_shape_and_stable_codes_are_explicit() {
        let error = PublicError::new(
            ErrorCode::Conflict,
            operation(),
            Some(ErrorField::ExpectedVersion),
        );
        let value = serde_json::to_value(&error).unwrap();
        assert_eq!(value["code"], "C360_CONFLICT");
        assert_eq!(value["field"], "expected_version");
        assert_eq!(value["operation_id"], operation().to_string());
        assert_eq!(value["write_outcome"], "not_committed");
        assert_eq!(value.as_object().unwrap().len(), 6);
        assert_eq!(error.field(), Some(ErrorField::ExpectedVersion));
    }

    #[test]
    fn every_error_has_a_safe_recovery_without_false_success() {
        for (code, expected) in [
            (ErrorCode::InvalidInput, "C360_INVALID_INPUT"),
            (ErrorCode::NotFound, "C360_NOT_FOUND"),
            (ErrorCode::Conflict, "C360_CONFLICT"),
            (ErrorCode::Unavailable, "C360_UNAVAILABLE"),
            (ErrorCode::Timeout, "C360_TIMEOUT"),
            (ErrorCode::Internal, "C360_INTERNAL"),
            (ErrorCode::OutcomeUnknown, "C360_OUTCOME_UNKNOWN"),
        ] {
            let error = PublicError::new(code, operation(), None);
            let json = serde_json::to_value(&error).unwrap();
            assert_eq!(json["code"], expected);
            assert_eq!(error.code(), code);
            assert_eq!(error.operation_id(), operation());
            assert!(json["field"].is_null());
            assert!(!error.recovery().is_empty());
            let serialized = json.to_string();
            for forbidden in [
                "SQL",
                "postgres",
                "password",
                "constraint",
                "schema",
                "stack",
                "segredo",
            ] {
                assert!(!serialized.contains(forbidden), "{serialized}");
            }
            if code == ErrorCode::OutcomeUnknown {
                assert_eq!(error.write_outcome(), WriteOutcome::Unknown);
                assert!(!error.message().contains("Nenhuma alteração"));
                assert!(error.recovery().starts_with("Não repita"));
                assert!(!error.recovery().contains("tente novamente"));
            } else {
                assert_eq!(error.write_outcome(), WriteOutcome::NotCommitted);
                assert!(error.message().contains("Nenhuma alteração"));
            }
        }
    }
}
