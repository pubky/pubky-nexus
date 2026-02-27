use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use clap::Parser;
use nexus_common::{file::validate_and_expand_path, types::DynError, ApiConfig, StackConfig};
use nexus_webapi::{api_context::ApiContextBuilder, NexusApi, NexusApiBuilder};

#[derive(Parser)]
#[command(about = "Example Nexus API server", long_about = None)]
struct Opt {
    /// Path to a directory containing `api-config.toml`
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
            NexusApi::start_from_path(expanded_path, None).await?
        }
        None => {
            let api_config = ApiConfig {
                name: String::from("nexusd.api"),
                public_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
                public_addr: SocketAddr::from(([127, 0, 0, 1], 8081)),
                pubky_listen_socket: SocketAddr::from(([127, 0, 0, 1], 8082)),
                stack: StackConfig::default(),
            };

            let api_context = ApiContextBuilder::from_default_config_dir()
                .api_config(api_config)
                .try_build()
                .await?;

            NexusApiBuilder::new(api_context).start(None).await?
        }
    };

    Ok(())
}
