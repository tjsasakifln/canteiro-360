use canteiro_api::{config::Config, render_shell, router};
use std::{error::Error, io::Write};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !args.is_empty() && args != ["--render-shell"] {
        return Err("Use canteiro-api ou canteiro-api --render-shell".into());
    }
    let config = Config::from_env()?;
    if args == ["--render-shell"] {
        std::io::stdout().write_all(render_shell(&config).await?.as_bytes())?;
        return Ok(());
    }
    if !config.site_root.join("pkg/canteiro-web_bg.wasm").is_file() {
        return Err(
            "Artefatos da interface ausentes. Execute cargo xtask build antes de iniciar.".into(),
        );
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let listener = TcpListener::bind(config.bind_addr).await?;
    tracing::info!(address = %listener.local_addr()?, environment = ?config.environment, "Canteiro 360 iniciado");
    axum::serve(listener, router(&config))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
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
