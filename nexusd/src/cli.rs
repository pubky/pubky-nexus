use clap::{Args, Parser, Subcommand};
use nexus_api::mock::MockType;
use std::path::PathBuf;

const NEXUSD_CONFIG_PATH: &str = "./nexusd/src/conf.toml";
const API_CONFIG_PATH: &str = "./nexus-api/src/conf.toml";
const WATCHER_CONFIG_PATH: &str = "./nexus-watcher/src/conf.toml";

#[derive(Parser, Debug)]
#[command(name = "pubky-nexus")]
#[command(about = "Pubky Nexus CLI", long_about = None)]
pub struct Cli {
    #[arg(short, long, num_args = 0..=1, require_equals = true, default_missing_value = NEXUSD_CONFIG_PATH)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<NexusCommands>,
}

impl Cli {
    pub fn receive_command(cli: Cli) -> NexusCommands {
        match (cli.command, cli.config) {
            (None, Some(file_path)) => NexusCommands::Run {
                config: Some(file_path.to_string_lossy().into_owned()),
            },
            (None, None) => NexusCommands::Run { config: None },
            (Some(command), _) => command,
        }
    }
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
        #[arg(short, long, num_args = 0..=1, default_missing_value = NEXUSD_CONFIG_PATH)]
        config: Option<String>,
    },
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, num_args = 0..=1, require_equals = true, default_missing_value = API_CONFIG_PATH)]
    pub config: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct WatcherArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, num_args = 0..=1, require_equals = true, default_missing_value = WATCHER_CONFIG_PATH)]
    pub config: Option<PathBuf>,
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
