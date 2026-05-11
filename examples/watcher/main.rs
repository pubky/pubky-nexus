use std::path::PathBuf;

use clap::Parser;
use nexus_common::{file::validate_and_expand_path, types::DynError};
use nexus_watcher::{service::NexusWatcher, NexusWatcherBuilder};

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
    let opts = Opt::parse();
    match opts.config {
        Some(path) => {
            let expanded_path = validate_and_expand_path(path)?;
            NexusWatcher::start_from_path(expanded_path, None).await?
        }
        None => {
            NexusWatcherBuilder(Default::default()).start(None).await?;
        }
    }

    Ok(())
}
