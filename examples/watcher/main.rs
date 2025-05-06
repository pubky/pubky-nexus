use nexus_common::{
    db::DatabaseConfig, file::expand_home_dir, types::DynError, StackConfig, WatcherConfig,
    FILES_DIR, LOG_LEVEL,
};
use nexus_watcher::{NexusWatcher, NexusWatcherBuilder};
use pubky_app_specs::PubkyId;
use std::path::PathBuf;

const FROM_FILE: bool = true;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    match FROM_FILE {
        true => NexusWatcher::start_from_path(PathBuf::from("examples/watcher")).await?,
        false => {
            let homeserver =
                PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap();
            let stack = StackConfig {
                log_level: LOG_LEVEL,
                files_path: expand_home_dir(PathBuf::from(FILES_DIR)),
                otlp_endpoint: None,
                db: DatabaseConfig::default(),
            };
            let config = WatcherConfig {
                name: String::from("nexusd.api"),
                testnet: false,
                homeserver,
                events_limit: 100,
                watcher_sleep: 5000,
                stack,
            };
            NexusWatcherBuilder(config).start().await?;
        }
    }

    Ok(())
}
