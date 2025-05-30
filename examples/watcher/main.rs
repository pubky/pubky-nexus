use clap::Parser;
use nexus_common::{
    db::DatabaseConfig, file::expand_home_dir, types::DynError, StackConfig, WatcherConfig,
    FILES_DIR, LOG_LEVEL,
};
use nexus_watcher::{NexusWatcher, NexusWatcherBuilder};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Example Nexus API server", long_about = None)]
struct Opt {
    /// Path to a directory containing `api.yaml` (or similar)
    /// If omitted, runs the built-in default API config
    #[arg(short, long, value_name = "DIR")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), DynError> {
    let opts = Opt::parse();
    match opts.config {
        Some(path) => {
            let path = expand_home_dir(path);
            if path.exists() && path.is_file() {
                return Err(format!(
                    "create with `mkdir -p folder_path` or point to a directory: {}",
                    path.display()
                )
                .into());
            }
            NexusWatcher::start_from_path(PathBuf::from("examples/watcher")).await?
        }
        None => {
            let homeserver =
                PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap();
            let moderation_id =
                PubkyId::try_from("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
            let stack = StackConfig {
                log_level: LOG_LEVEL,
                files_path: expand_home_dir(PathBuf::from(FILES_DIR)),
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
            NexusWatcherBuilder(config).start().await?;
        }
    }

    Ok(())
}
