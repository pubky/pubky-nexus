use clap::{Args, Parser, Subcommand};
use nexus_common::file::{expand_home_dir, DEFAULT_HOME_DIR};
use nexus_webapi::mock::MockType;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pubky-nexus")]
#[command(about = "Pubky Nexus CLI", long_about = None)]
pub struct Cli {
    /// Directory containing `config.toml`
    #[arg(short, long, default_value_os_t = default_config_dir_path(), value_parser = validate_config_dir_path)]
    pub config_dir: PathBuf,

    #[command(subcommand)]
    pub command: Option<NexusCommands>,
}

impl Cli {
    pub fn receive_command(cli: Cli) -> NexusCommands {
        match cli.command {
            None => NexusCommands::Run {
                config_dir: cli.config_dir,
            },
            Some(command) => command,
        }
    }
}

fn default_config_dir_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(DEFAULT_HOME_DIR)
}

/// Validate that the data_dir path is a directory.
/// It doesnt need to exist, but if it does, it needs to be a directory.
fn validate_config_dir_path(path: &str) -> Result<PathBuf, String> {
    let path = expand_home_dir(PathBuf::from(path));
    if path.exists() && path.is_file() {
        return Err(format!(
            "create with `mkdir -p folder_path` or point to a directory: {}",
            path.display()
        ));
    }
    Ok(path)
}

#[derive(Subcommand, Debug)]
pub enum NexusCommands {
    /// Run the API service
    Api(ApiArgs),

    /// Run the event watcher
    Watcher(WatcherArgs),

    /// Database operations
    #[command(subcommand)]
    Db(DbCommands),

    /// Run both the API and the Watcher (default when no arguments are given)
    #[command(hide = true)]
    Run {
        /// Path to the configuration file
        #[arg(short, long, default_value_os_t = default_config_dir_path(), value_parser = validate_config_dir_path)]
        config_dir: PathBuf,
    },
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, default_value_os_t = default_config_dir_path(), value_parser = validate_config_dir_path)]
    pub config_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct WatcherArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, default_value_os_t = default_config_dir_path(), value_parser = validate_config_dir_path)]
    pub config_dir: PathBuf,
}

#[derive(Subcommand, Debug)]
pub enum DbCommands {
    /// Clear the databases
    Clear,

    /// Mock the database (optional redis/graph). Usually for tests
    Mock(MockArgs),

    /// Manage database migrations
    #[command(subcommand)]
    Migration(MigrationCommands),
}

#[derive(Args, Debug)]
pub struct MockArgs {
    /// Specify which part of the database to mock: redis, graph, or both (default: both)
    #[arg(long)]
    pub mock_type: Option<MockType>,
}

#[derive(Subcommand, Debug)]
pub enum MigrationCommands {
    /// Create a new migration with a required migration name
    New(MigrationNewArgs),

    /// Run pending migrations
    Run,
}

#[derive(Args, Debug)]
pub struct MigrationNewArgs {
    /// The name of the new migration
    #[arg(required = true)]
    pub name: String,
}
