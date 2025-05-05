use clap::Parser;
use nexus_api::mock::MockDb;
use nexus_api::NexusApiBuilder;
use nexus_common::file::ConfigReader;
use nexus_common::DaemonConfig;
use nexus_watcher::NexusWatcherBuilder;
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
        NexusCommands::Api(ApiArgs { config }) => {
            let config = DaemonConfig::read_config_file(config).await?;
            println!("Starting api service...");
            // Run API WebServer service
            NexusApiBuilder(config.into()).start().await?
        }
        NexusCommands::Watcher(WatcherArgs { config }) => {
            let config = DaemonConfig::read_config_file(config).await?;
            println!("Starting watcher...");
            // Run watcher service
            NexusWatcherBuilder(config.into()).start().await?
        }
        NexusCommands::Run { config } => DaemonLauncher::start(config).await?,
    }
    Ok(())
}
