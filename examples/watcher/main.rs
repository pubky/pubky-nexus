use std::path::PathBuf;

use clap::Parser;
use nexus_common::{
    db::DatabaseConfig, file::validate_and_expand_path, get_files_dir_pathbuf, types::DynError,
    StackConfig, WatcherConfig, LOG_LEVEL,
};
use nexus_watcher::{NexusWatcher, NexusWatcherBuilder};
use pubky_app_specs::PubkyId;

#[derive(Parser)]
#[command(about = "Example Nexus Watcher server", long_about = None)]
struct Opt {
    /// Path to a directory containing `watcher-config.toml`
    /// If omitted, runs the built-in default API config
    #[arg(short, long, value_name = "DIR")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Ctrl+C handler
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });

    let opts = Opt::parse();
    match opts.config {
        Some(path) => {
            let expanded_path = validate_and_expand_path(path)?;
            NexusWatcher::start_from_path(shutdown_rx, expanded_path).await?
        }
        None => {
            let homeserver =
                PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap();
            let moderation_id =
                PubkyId::try_from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
            let stack = StackConfig {
                log_level: LOG_LEVEL,
                files_path: get_files_dir_pathbuf(),
                otlp_endpoint: None,
                db: DatabaseConfig::default(),
            };
            let config = WatcherConfig {
                name: String::from("nexusd.watcher"),
                testnet: false,
                homeserver,
                events_limit: 100,
                watcher_sleep: 5000,
                stack,
                moderation_id,
                moderated_tags: Vec::new(),
            };
            NexusWatcherBuilder(config).start(shutdown_rx).await?;
        }
    }

    Ok(())
}
