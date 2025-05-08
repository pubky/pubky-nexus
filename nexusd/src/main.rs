use clap::Parser;
use nexus_api::mock::MockDb;
use nexus_api::NexusApi;
use nexus_watcher::NexusWatcher;
use nexusd::cli::{ApiArgs, Cli, DbCommands, MigrationCommands, NexusCommands, WatcherArgs};
use nexusd::migrations::{import_migrations, MigrationBuilder, MigrationManager};
use nexusd::DaemonLauncher;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();

    let command = Cli::receive_command(cli);

    match command {
        NexusCommands::Db(db_command) => match db_command {
            DbCommands::Clear => MockDb::clear_database().await,
            DbCommands::Mock(args) => MockDb::run(args.mock_type).await,
            DbCommands::Migration(migration_command) => match migration_command {
                MigrationCommands::New(args) => MigrationManager::new_migration(args.name).await?,
                MigrationCommands::Run => {
                    let builder = MigrationBuilder::default().await;
                    let mut mm = builder.init_stack().await;
                    import_migrations(&mut mm);
                    mm.run(&builder.migrations_backfill_ready()).await?;
                }
            },
        },
        NexusCommands::Api(ApiArgs { config }) => NexusApi::start_from_daemon(config).await?,
        NexusCommands::Watcher(WatcherArgs { config }) => {
            NexusWatcher::start_from_daemon(config).await?
        }
        NexusCommands::Run { config } => DaemonLauncher::start(config).await?,
    }
    Ok(())
}
