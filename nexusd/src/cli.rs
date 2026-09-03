use clap::{Args, Parser, Subcommand};
use nexus_common::file::{default_config_dir_path, validate_and_expand_path};
use nexus_webapi::mock::MockType;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pubky-nexus")]
#[command(about = "Pubky Nexus CLI", long_about = None)]
pub struct Cli {
    /// Directory containing `config.toml`
    #[arg(short, long, global = true, default_value_os_t = default_config_dir_path(), value_parser = validate_config_dir_path)]
    pub config_dir: PathBuf,

    #[command(subcommand)]
    pub command: Option<NexusCommands>,
}

/// Validate that the data_dir path is a directory.
/// It doesnt need to exist, but if it does, it needs to be a directory.
fn validate_config_dir_path(path: &str) -> Result<PathBuf, String> {
    validate_and_expand_path(PathBuf::from(path)).map_err(|e| e.to_string())
}

#[derive(Subcommand, Debug)]
pub enum NexusCommands {
    /// Run the API service
    Api,

    /// Run the event watcher
    Watcher,

    /// Run scheduled jobs on demand
    #[command(subcommand)]
    Jobs(JobCommands),

    /// Database operations
    #[command(subcommand)]
    Db(DbCommands),

    /// Run the API, the Watcher and the scheduled Jobs (default when no arguments are given)
    #[command(hide = true)]
    Run,
}

#[derive(Subcommand, Debug)]
pub enum JobCommands {
    /// Run a single job once, now
    Run(JobRunArgs),

    /// List the available jobs
    List,
}

#[derive(Args, Debug)]
pub struct JobRunArgs {
    /// Name of the job to run (see `jobs list`)
    #[arg(required = true)]
    pub name: String,
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
    use clap::{error::ErrorKind, CommandFactory};

    #[test]
    fn cli_definition_is_valid() {
        // Panics on duplicate argument IDs, e.g. if a per-subcommand
        // `config_dir` is ever re-introduced alongside the global one.
        Cli::command().debug_assert();
    }

    #[test]
    fn bare_invocation_defaults_to_running_everything() {
        let cli = Cli::try_parse_from(["nexusd"]).expect("bare nexusd should parse");
        assert!(cli.command.is_none());
        assert_eq!(cli.config_dir, default_config_dir_path());
    }

    /// `-c/--config-dir` must be honored both before and after any subcommand
    /// (clap globals are position-independent).
    #[test]
    fn config_dir_is_position_independent() {
        const DIR: &str = "test-config-dir";
        let dir = PathBuf::from(DIR);
        let cases: &[&[&str]] = &[
            &["run"],
            &["api"],
            &["watcher"],
            &["jobs", "run", "some-job"],
            &["db", "clear"],
            &["db", "mock"],
            &["db", "migration", "run"],
            &["db", "migration", "check"],
            &["db", "migration", "new", "some-migration"],
        ];

        for subcommand in cases {
            let mut before = vec!["nexusd", "-c", DIR];
            before.extend_from_slice(subcommand);
            let cli = Cli::try_parse_from(before)
                .unwrap_or_else(|e| panic!("flag before {subcommand:?} should parse: {e}"));
            assert_eq!(
                cli.config_dir, dir,
                "flag before {subcommand:?} should set config_dir"
            );

            let mut after = vec!["nexusd"];
            after.extend_from_slice(subcommand);
            after.extend_from_slice(&["-c", DIR]);
            let cli = Cli::try_parse_from(after)
                .unwrap_or_else(|e| panic!("flag after {subcommand:?} should parse: {e}"));
            assert_eq!(
                cli.config_dir, dir,
                "flag after {subcommand:?} should set config_dir"
            );
        }
    }

    #[test]
    fn run_subcommand_is_still_accepted() {
        let cli = Cli::try_parse_from(["nexusd", "run"]).expect("`nexusd run` should parse");
        assert!(matches!(cli.command, Some(NexusCommands::Run)));
    }

    #[test]
    fn unknown_flag_still_errors() {
        let err = Cli::try_parse_from(["nexusd", "--definitely-not-a-flag"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

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
