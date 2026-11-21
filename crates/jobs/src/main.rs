//! Consumidor independente das notificações internas da organização.
use canteiro_persistence::{
    Database, DatabaseEnvironment, DatabaseSettings, TransactionLimits,
    reliability_outbox::NotificationStep,
};
use std::{
    error::Error,
    time::{Duration, Instant},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--check"] {
        println!("Canteiro 360: executável de processamento disponível.");
        return Ok(());
    }
    if !args.is_empty() {
        return Err("Use canteiro-jobs ou canteiro-jobs --check".into());
    }
    let environment: DatabaseEnvironment = std::env::var("CANTEIRO_ENV")
        .map_err(|_| "Identifique o ambiente de processamento em CANTEIRO_ENV.")?
        .parse()?;
    let settings = DatabaseSettings::from_external_url(
        &std::env::var("CANTEIRO_DATABASE_URL")
            .map_err(|_| "Injete a configuração externa do PostgreSQL para o processamento.")?,
    )?;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let database = Database::connect(&settings, environment, TransactionLimits::default()).await?;
    tokio::select! {
        _ = run(&database) => {},
        _ = shutdown() => {},
    }
    database.close().await;
    Ok(())
}

async fn run(database: &Database) {
    let mut last_connection_warning = None;
    loop {
        // Fila de contratos e caixa de entrada avançam de forma independente.
        // A fila de e-mails permanece fora deste processador: sem provedor,
        // nenhuma mensagem externa pode ser dada como entregue.
        let contract = database.consume_contract_notification().await;
        let notice = database.consume_user_notice_delivery().await;
        let failed = contract.is_err() || notice.is_err();
        let delivered = matches!(contract, Ok(NotificationStep::Delivered))
            || matches!(notice, Ok(NotificationStep::Delivered));
        let deferred = matches!(contract, Ok(NotificationStep::Deferred))
            || matches!(notice, Ok(NotificationStep::Deferred));

        if failed {
            let now = Instant::now();
            if last_connection_warning
                .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(60))
            {
                tracing::warn!("Não foi possível consultar uma das filas de comunicação.");
                last_connection_warning = Some(now);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        } else {
            last_connection_warning = None;
            if deferred {
                tracing::warn!("Uma comunicação foi adiada para nova tentativa.");
            }
            if delivered || deferred {
                tokio::task::yield_now().await;
            } else {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
