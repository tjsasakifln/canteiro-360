use crate::{
    http::{RequestFailure, RequestOptions, request},
    organizations::network::query_component,
};
use canteiro_contracts::{
    auth::AuthFailure,
    reliability::{
        ContractDetail, ReliabilityCommand, ReliabilityContext, ReliabilityReply, ReliabilityScope,
        SaveReceipt,
    },
};

const ROOT: &str = "/api/reliability";

pub(super) fn unavailable() -> AuthFailure {
    AuthFailure {
        code: "network_unavailable".into(),
        message: "A resposta do servidor não chegou. Seus campos continuam nesta aba.".into(),
    }
}

pub(super) async fn context() -> Result<ReliabilityContext, RequestFailure> {
    request(
        &format!("{ROOT}/context"),
        None,
        RequestOptions::default(),
        unavailable,
    )
    .await
}

fn contract_url(scope: &ReliabilityScope, id: &str, suffix: &str) -> String {
    format!(
        "{ROOT}/contracts/{}{suffix}?organization_id={}&context_version={}",
        query_component(id),
        query_component(&scope.organization_id),
        scope.context_version,
    )
}

pub(super) fn history_url(scope: &ReliabilityScope, id: &str) -> String {
    contract_url(scope, id, "/history.csv")
}

pub(super) async fn detail(
    scope: &ReliabilityScope,
    id: &str,
    after: Option<i64>,
) -> Result<ContractDetail, RequestFailure> {
    let mut path = contract_url(scope, id, "");
    if let Some(after) = after {
        use std::fmt::Write;
        let _ = write!(path, "&after={after}");
    }
    request(&path, None, RequestOptions::default(), unavailable).await
}

pub(super) async fn operation(
    scope: &ReliabilityScope,
    id: &str,
    key: &str,
) -> Result<Option<SaveReceipt>, RequestFailure> {
    let path = contract_url(scope, id, &format!("/operations/{}", query_component(key)));
    request(&path, None, RequestOptions::default(), unavailable).await
}

pub(super) async fn command(
    command: &ReliabilityCommand,
    csrf: &str,
) -> Result<ReliabilityReply, RequestFailure> {
    let body = serde_json::to_string(command).map_err(|_| RequestFailure {
        status: None,
        details: unavailable(),
    })?;
    request(
        &format!("{ROOT}/command"),
        Some(body),
        RequestOptions {
            csrf: Some(csrf),
            accept_typed_conflict: true,
            ..RequestOptions::default()
        },
        unavailable,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_links_cannot_change_company_or_contract() {
        let scope = ReliabilityScope {
            organization_id: "outra&context_version=99".into(),
            context_version: 7,
        };
        assert_eq!(
            history_url(&scope, "x/a?b=1"),
            "/api/reliability/contracts/x%2Fa%3Fb%3D1/history.csv?organization_id=outra%26context_version%3D99&context_version=7"
        );
    }
}
