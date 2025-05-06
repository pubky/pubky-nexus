use nexus_api::{NexusApi, NexusApiBuilder};
use nexus_common::{types::DynError, ApiConfig, StackConfig};
use std::{net::SocketAddr, path::PathBuf};

const FROM_FILE: bool = true;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    match FROM_FILE {
        true => NexusApi::start_from_path(PathBuf::from("examples/api")).await?,
        false => {
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
