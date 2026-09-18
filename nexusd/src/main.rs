use clap::Parser;
use nexus_common::types::DynError;
use nexus_common::{DaemonConfig, StackManager, TrustRankConfig};
use nexus_watcher::service::NexusWatcher;
use nexus_webapi::mock::MockDb;
use nexus_webapi::NexusApi;
use nexusd::cli::{Cli, DbCommands, JobCommands, JobRunArgs, MigrationCommands, NexusCommands};
use nexusd::jobs::JobRegistry;
use nexusd::migrations::{import_migrations, MigrationBuilder, MigrationManager};
use nexusd::DaemonLauncher;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let cli = Cli::parse();
    let config_dir = cli.config_dir;

    match cli.command {
        Some(NexusCommands::Db(db_command)) => match db_command {
            DbCommands::Clear { yes } => {
                if !yes {
                    eprintln!(
                        "db clear is destructive: it wipes the Redis logical database (FLUSHDB) and deletes every node in the Neo4j graph configured in {}.",
                        config_dir.display()
                    );
                    eprintln!("Re-run with --yes to proceed.");
                    std::process::exit(1);
                }
                let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
                MockDb::clear_database(&config.stack).await
            }
            DbCommands::Mock(args) => {
                let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
                MockDb::run(args.mock_type, &config.stack).await
            }
            DbCommands::Reindex => {
                let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
                StackManager::setup(&config.stack).await?;
                nexus_common::db::reindex::rebuild().await?;
            }
            DbCommands::Migration(migration_command) => match migration_command {
                MigrationCommands::New(args) => MigrationManager::new_migration(args.name).await?,
                MigrationCommands::Run => {
                    let builder = MigrationBuilder::new(config_dir).await?;
                    StackManager::setup(builder.stack()).await?;
                    let mut mm = MigrationManager::default();
                    import_migrations(&mut mm);
                    mm.run(&builder.migrations_backfill_ready()).await?;
                }
                MigrationCommands::Check => {
                    let builder = MigrationBuilder::new(config_dir).await?;
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
        Some(NexusCommands::Api) => {
            NexusApi::start_from_daemon(config_dir, None).await?;
        }
        Some(NexusCommands::Watcher) => {
            NexusWatcher::start_from_daemon(config_dir, None).await?;
        }
        Some(NexusCommands::Jobs(job_command)) => match job_command {
            JobCommands::Run(JobRunArgs { name }) => {
                let config = DaemonConfig::read_or_create_config_file(config_dir).await?;
                JobRegistry::catalog(&config.trust_rank)
                    .run_on_demand(&name, &config)
                    .await?;
            }
            JobCommands::List => {
                // Names don't depend on config, so no file is read.
                for name in JobRegistry::catalog(&TrustRankConfig::default()).job_names() {
                    println!("{name}");
                }
            }
        },
        None | Some(NexusCommands::Run) => {
            DaemonLauncher::start(config_dir, None).await?;
        }
    }

    Ok(())
}
