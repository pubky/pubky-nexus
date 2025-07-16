use std::net::SocketAddr;

use anyhow::Result;
use nexus_common::{get_files_dir_test_pathbuf, ApiConfig};
use nexus_webapi::{api_context::ApiContextBuilder, NexusApi, NexusApiBuilder};
use tokio::sync::OnceCell;

/// Util backend server for testing.
/// Performs the same routine the main service server does.
/// OnceCell is used to ensure the server is only started once.
pub struct TestServiceServer {
    pub nexus_api: NexusApi,
}

static TEST_SERVER: OnceCell<TestServiceServer> = OnceCell::const_new();

impl TestServiceServer {
    pub async fn get_test_server() -> &'static TestServiceServer {
        TEST_SERVER
            .get_or_init(|| async {
                let nexus_api = Self::start_server().await.unwrap();
                TestServiceServer { nexus_api }
            })
            .await
    }

    async fn start_server() -> Result<NexusApi> {
        let test_api_config = ApiConfig {
            // When we define the sockets, use local port 0 so OS assigns an available port
            public_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            pubky_listen_socket: SocketAddr::from(([127, 0, 0, 1], 0)),
            ..Default::default()
        };

        let api_context = ApiContextBuilder::from_default_config_dir()
            .api_config(test_api_config)
            .try_build()
            .await
            .expect("Failed to create ApiContext");
        let nexus_builder = NexusApiBuilder(api_context).files_path(get_files_dir_test_pathbuf());
        let nexus_api = nexus_builder.start().await.unwrap();

        Ok(nexus_api)
    }
}
