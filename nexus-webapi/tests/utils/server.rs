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
    pub testnet: pubky_testnet::Testnet,
}

static TEST_SERVER: OnceCell<TestServiceServer> = OnceCell::const_new();
static TEST_SERVER_WITH_KEY_REPUBLISHER: OnceCell<TestServiceServer> = OnceCell::const_new();

impl TestServiceServer {
    pub async fn get_test_server() -> &'static TestServiceServer {
        TEST_SERVER
            .get_or_init(|| async {
                let testnet = pubky_testnet::Testnet::new().await.unwrap();
                let nexus_api = Self::start_server(&testnet, false).await.unwrap();
                TestServiceServer { nexus_api, testnet }
            })
            .await
    }

    /// Returns a test server with the [KeyRepublisher] enabled.
    ///
    /// Use this in tests that access the API via the Pubky TLS DNS URL, which requires
    /// the server's pkarr packet to be published to the DHT.
    pub async fn get_test_server_with_key_republisher() -> &'static TestServiceServer {
        TEST_SERVER_WITH_KEY_REPUBLISHER
            .get_or_init(|| async {
                let testnet = pubky_testnet::Testnet::new().await.unwrap();
                let nexus_api = Self::start_server(&testnet, true).await.unwrap();
                TestServiceServer { nexus_api, testnet }
            })
            .await
    }

    async fn start_server(
        testnet: &pubky_testnet::Testnet,
        enable_key_republisher: bool,
    ) -> Result<NexusApi> {
        let test_api_config = ApiConfig {
            // When we define the sockets, use local port 0 so OS assigns an available port
            public_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            pubky_listen_socket: SocketAddr::from(([127, 0, 0, 1], 0)),
            ..Default::default()
        };

        // Every time we start a test server, use a new temp config dir, which is automatically removed after the tests
        let temp_config_dir = tempfile::TempDir::new_in(".")?;
        let api_context = ApiContextBuilder::from_config_dir(temp_config_dir.path().to_path_buf())
            .api_config(test_api_config)
            .pkarr_builder(testnet.pkarr_client_builder())
            .try_build()
            .await
            .expect("Failed to create ApiContext");
        let nexus_builder = NexusApiBuilder::new(api_context)
            .files_path(get_files_dir_test_pathbuf())
            .enable_key_republisher(enable_key_republisher);

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let _ = shutdown_tx.send(true); // We want the test server to return right away after start()
        let nexus_api = nexus_builder.start(Some(shutdown_rx)).await.unwrap();

        Ok(nexus_api)
    }
}
