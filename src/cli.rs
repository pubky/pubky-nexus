use clap::{Parser, Subcommand, Args, ValueEnum};

#[derive(Parser, Debug)]
#[command(name = "pubky-nexus")]
#[command(about = "Pubky Nexus CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<NexusCommands>
}

#[derive(Subcommand, Debug)]
pub enum NexusCommands {
    /// Run the API service
    Api,

    /// Run the event watcher
    Watcher,

    /// Database operations
    #[command(subcommand)]
    Db(DbCommands),

    /// Run both the API and the Watcher (default when no arguments are given)
    #[command(hide = true)]
    All,
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

#[derive(ValueEnum, Clone, Debug)]
pub enum MockType {
    Redis,
    Graph,
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