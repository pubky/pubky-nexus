use clap::Parser;
use nexus_common::{file::try_expand_home_dir, types::DynError, ApiConfig, StackConfig};
use nexus_webapi::{NexusApi, NexusApiBuilder};
use std::{net::SocketAddr, path::PathBuf};

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
            let expanded_path = try_expand_home_dir(path)?;
            NexusApi::start_from_path(expanded_path).await?
        }
        None => {
            let config = ApiConfig {
                name: String::from("nexusd.api"),
                public_addr: SocketAddr::from(([127, 0, 0, 1], 8081)),
                stack: StackConfig::default(),
            };
            NexusApiBuilder(config).start().await?;
        }
    }

    Ok(())
}
