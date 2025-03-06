use clap::{Args, Parser, Subcommand};
use nexus_api::mock::MockType;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pubky-nexus")]
#[command(about = "Pubky Nexus CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<NexusCommands>,
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
    All,
}

#[derive(Args, Debug)]
pub struct ApiArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, num_args = 0..=1, default_missing_value = "./src/aconf_template.toml")]
    pub config: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct WatcherArgs {
    /// Optional configuration file for the watcher
    #[arg(short, long, num_args = 0..=1, default_missing_value = "./src/wconf_template.toml")]
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
