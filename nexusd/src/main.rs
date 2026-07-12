use std::sync::Arc;

use clap::Parser;
use nexus_common::types::DynError;
use nexus_common::{DaemonConfig, StackManager, TrustRankConfig};
use nexus_watcher::service::NexusWatcher;
use nexus_webapi::mock::MockDb;
use nexus_webapi::NexusApi;
use nexusd::cli::{
    ApiArgs, Cli, DbCommands, JobCommands, JobRunArgs, MigrationCommands, NexusCommands,
    WatcherArgs,
};
use nexusd::jobs::JobRegistry;
use nexusd::migrations::{import_migrations, MigrationBuilder, MigrationManager};
use nexusd::trust::TrustRecomputeJob;
use nexusd::DaemonLauncher;

/// The registry of jobs available to the daemon, built from config. Config-free
/// callers (e.g. `jobs list`) can pass `TrustRankConfig::default()`.
fn job_registry(trust_rank: &TrustRankConfig) -> JobRegistry {
    JobRegistry::new(vec![Arc::new(TrustRecomputeJob::from_config(trust_rank))])
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cli = Cli::parse();
    let command = Cli::receive_command(cli);

    match command {
        NexusCommands::Db(db_command) => match db_command {
            DbCommands::Clear => MockDb::clear_database().await,
            DbCommands::Mock(args) => MockDb::run(args.mock_type).await,
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
        NexusCommands::Jobs(job_command) => match job_command {
            JobCommands::Run(JobRunArgs { name, config_dir }) => {
                let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
                // run_on_demand validates [jobs.*], so a typo'd section fails here
                // just like `nexusd run`.
                job_registry(&config.trust_rank)
                    .run_on_demand(&name, &config)
                    .await?;
            }
            JobCommands::List => {
                // Listing needs only job names, so a default config suffices.
                for name in job_registry(&TrustRankConfig::default()).job_names() {
                    println!("{name}");
                }
            }
        },
        NexusCommands::Run { config_dir } => {
            let config = DaemonConfig::read_or_create_config_file(config_dir.clone()).await?;
            DaemonLauncher::start(config_dir, &job_registry(&config.trust_rank), None).await?;
        }
    }

    Ok(())
}
