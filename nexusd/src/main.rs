use clap::Parser;
use nexus_common::types::DynError;
use nexus_common::StackManager;
use nexus_watcher::service::NexusWatcher;
use nexus_webapi::mock::MockDb;
use nexus_webapi::NexusApi;
use nexusd::cli::{ApiArgs, Cli, DbCommands, MigrationCommands, NexusCommands, WatcherArgs};
use nexusd::migrations::{import_migrations, MigrationBuilder, MigrationManager};
use nexusd::DaemonLauncher;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cli = Cli::parse();
    let command = Cli::receive_command(cli);
    match command {
        NexusCommands::Db(db_command) => match db_command {
            DbCommands::Clear => MockDb::clear_database().await,
            DbCommands::Mock(args) => MockDb::run(args.mock_type).await,
            DbCommands::Reindex(args) => {
                // The daemon's own config file, not the migrations one: a
                // missing migrations config would silently auto-create itself
                // with default credentials.
                let dc =
                    nexus_common::DaemonConfig::read_or_create_config_file(args.config_dir).await?;
                let api_config = nexus_common::ApiConfig::from(dc);
                StackManager::setup(&api_config.stack).await?;
                // Clean rebuild: entities deleted since the backup must not
                // survive in the index.
                nexus_common::db::reindex::rebuild().await;
            }
            DbCommands::Migration(migration_command) => match migration_command {
                MigrationCommands::New(args) => MigrationManager::new_migration(args.name).await?,
                MigrationCommands::Run => {
                    let builder = MigrationBuilder::default().await?;
                    StackManager::setup(builder.stack()).await?;
                    let mut mm = MigrationManager::default();
                    import_migrations(&mut mm);
                    mm.run(&builder.migrations_backfill_ready()).await?;
                }
                MigrationCommands::Check => {
                    let builder = MigrationBuilder::default().await?;
                    StackManager::setup(builder.stack()).await?;
                    let mut mm = MigrationManager::default();
                    import_migrations(&mut mm);
                    let pending = mm.check(&builder.migrations_backfill_ready()).await?;
                    if pending.is_empty() {
                        println!("No pending migrations");
                    } else {
                        for (id, phase) in &pending {
                            println!("{id} ({phase})");
                        }
                        println!("{} pending migration(s)", pending.len());
                        std::process::exit(10);
                    }
                }
            },
        },
        NexusCommands::Api(ApiArgs { config_dir }) => {
            NexusApi::start_from_daemon(config_dir, None).await?;
        }
        NexusCommands::Watcher(WatcherArgs { config_dir }) => {
            NexusWatcher::start_from_daemon(config_dir, None).await?;
        }
        NexusCommands::Run { config_dir } => {
            DaemonLauncher::start(config_dir, None).await?;
        }
    }

    Ok(())
}
