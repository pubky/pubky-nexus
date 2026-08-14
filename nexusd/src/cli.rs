use clap::{Args, Parser, Subcommand};
use nexus_common::file::{default_config_dir_path, validate_and_expand_path};
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

/// Validate that the data_dir path is a directory.
/// It doesnt need to exist, but if it does, it needs to be a directory.
fn validate_config_dir_path(path: &str) -> Result<PathBuf, String> {
    validate_and_expand_path(PathBuf::from(path)).map_err(|e| e.to_string())
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
    /// Clear the databases (destructive, requires --yes)
    Clear {
        /// Confirm wiping the Redis logical database (FLUSHDB) and every node
        /// in the Neo4j graph configured via --config-dir.
        #[arg(long)]
        yes: bool,
    },

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

    /// Check for pending migrations without running them.
    /// Exits 0 when nothing is pending, 10 when at least one migration has pending work.
    Check,
}

#[derive(Args, Debug)]
pub struct MigrationNewArgs {
    /// The name of the new migration
    #[arg(required = true)]
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_clear_without_yes_parses_as_unconfirmed() {
        let cli = Cli::try_parse_from(["nexusd", "db", "clear"]).expect("should parse");
        match cli.command {
            Some(NexusCommands::Db(DbCommands::Clear { yes })) => assert!(!yes),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn db_clear_with_yes_parses_as_confirmed() {
        let cli = Cli::try_parse_from(["nexusd", "db", "clear", "--yes"]).expect("should parse");
        match cli.command {
            Some(NexusCommands::Db(DbCommands::Clear { yes })) => assert!(yes),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    /// The top-level --config-dir must be available to db commands so they
    /// operate on the configured stack rather than the default one.
    #[test]
    fn db_clear_keeps_top_level_config_dir() {
        let cli = Cli::try_parse_from([
            "nexusd",
            "--config-dir",
            "/custom/dir",
            "db",
            "clear",
            "--yes",
        ])
        .expect("should parse");
        assert_eq!(cli.config_dir, PathBuf::from("/custom/dir"));
        match cli.command {
            Some(NexusCommands::Db(DbCommands::Clear { yes })) => assert!(yes),
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
