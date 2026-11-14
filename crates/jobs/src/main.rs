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
        let result = database.consume_contract_notification().await;
        if result.is_ok() {
            last_connection_warning = None;
        }
        match result {
            Ok(NotificationStep::Delivered) => tokio::task::yield_now().await,
            Ok(NotificationStep::Deferred) => {
                tracing::warn!("Uma notificação interna foi adiada para nova tentativa.");
                tokio::task::yield_now().await;
            }
            Ok(NotificationStep::Idle) => tokio::time::sleep(Duration::from_secs(1)).await,
            Err(_) => {
                let now = Instant::now();
                if last_connection_warning
                    .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(60))
                {
                    tracing::warn!("Não foi possível consultar a fila de notificações internas.");
                    last_connection_warning = Some(now);
                }
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
