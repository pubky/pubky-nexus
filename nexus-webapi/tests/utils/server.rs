use std::net::SocketAddr;

use anyhow::Result;
use nexus_common::ApiConfig;
use nexus_webapi::{api_context::ApiContextBuilder, NexusApi, NexusApiBuilder};
use tempfile::TempDir;
use tokio::sync::OnceCell;

/// Util backend server for testing.
/// Performs the same routine the main service server does.
/// OnceCell is used to ensure the server is only started once.
pub struct TestServiceServer {
    pub nexus_api: NexusApi,
    pub testnet: pubky_testnet::Testnet,
    /// Temp directory for static files. Kept alive so the directory is not deleted.
    pub temp_dir: TempDir,
}

/// [TestServiceServer] with no key republisher
static TEST_SERVER: OnceCell<TestServiceServer> = OnceCell::const_new();
/// [TestServiceServer] where the [NexusApi] is initialized with a key republisher
static TEST_SERVER_WITH_KEY_REPUBLISHER: OnceCell<TestServiceServer> = OnceCell::const_new();

impl TestServiceServer {
    /// Returns a test server with no [KeyRepublisher]. This is the default setup used in most tests.
    pub async fn get_test_server() -> &'static TestServiceServer {
        TEST_SERVER
            .get_or_init(|| async {
                let testnet = pubky_testnet::Testnet::new().await.unwrap();
                let (nexus_api, temp_dir) = Self::start_server(&testnet, false).await.unwrap();
                TestServiceServer {
                    nexus_api,
                    testnet,
                    temp_dir,
                }
            })
            .await
    }

    /// Returns a test server with the [KeyRepublisher] enabled, for the few tests that need it
    ///
    /// Use this in tests that access the API via the Pubky TLS DNS URL, which requires
    /// the server's pkarr packet to be published to the DHT.
    pub async fn get_test_server_with_key_republisher() -> &'static TestServiceServer {
        TEST_SERVER_WITH_KEY_REPUBLISHER
            .get_or_init(|| async {
                let testnet = pubky_testnet::Testnet::new().await.unwrap();
                let (nexus_api, temp_dir) = Self::start_server(&testnet, true).await.unwrap();
                TestServiceServer {
                    nexus_api,
                    testnet,
                    temp_dir,
                }
            })
            .await
    }

    async fn start_server(
        testnet: &pubky_testnet::Testnet,
        enable_key_republisher: bool,
    ) -> Result<(NexusApi, TempDir)> {
        let test_api_config = ApiConfig {
            // When we define the sockets, use local port 0 so OS assigns an available port
            public_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            pubky_listen_socket: SocketAddr::from(([127, 0, 0, 1], 0)),
            ..Default::default()
        };

        // Separate temp directories: one for config (keypair), one for static files
        let temp_config_dir = TempDir::new()?;
        let temp_dir = TempDir::new()?;

        let api_context = ApiContextBuilder::from_config_dir(temp_config_dir.path().to_path_buf())
            .api_config(test_api_config)
            .pkarr_builder(testnet.pkarr_client_builder())
            .try_build()
            .await
            .expect("Failed to create ApiContext");
        let nexus_builder = NexusApiBuilder::new(api_context)
            .files_path(temp_dir.path().to_path_buf())
            .enable_key_republisher(enable_key_republisher);

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let _ = shutdown_tx.send(true); // We want the test server to return right away after start()
        let nexus_api = nexus_builder.start(Some(shutdown_rx)).await.unwrap();

        Ok((nexus_api, temp_dir))
    }
}
