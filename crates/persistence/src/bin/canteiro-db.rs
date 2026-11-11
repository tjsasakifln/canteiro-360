//! Migração administrativa explícita. O servidor HTTP nunca recebe esta credencial.
use canteiro_persistence::{
    Database, DatabaseEnvironment, DatabaseSettings, TransactionLimits, migrate,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["migrate"] {
        return Err("Use canteiro-db migrate, com as configurações externas do ambiente.".into());
    }
    let environment: DatabaseEnvironment = std::env::var("CANTEIRO_ENV")
        .map_err(|_| "CANTEIRO_ENV precisa identificar explicitamente o ambiente de migração.")?
        .parse()?;
    let migrator = DatabaseSettings::from_external_url(
        &std::env::var("CANTEIRO_MIGRATION_DATABASE_URL")
            .map_err(|_| "Injete a configuração externa do usuário de migração.")?,
    )?;
    let runtime = DatabaseSettings::from_external_url(
        &std::env::var("CANTEIRO_DATABASE_URL")
            .map_err(|_| "Injete a configuração externa do usuário de execução.")?,
    )?;
    migrate(&migrator, &runtime, environment).await?;
    let database = Database::connect(&runtime, environment, TransactionLimits::default()).await?;
    database.close().await;
    println!("Migrações verificadas e usuário de execução conferido.");
    Ok(())
}
