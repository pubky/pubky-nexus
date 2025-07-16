use anyhow::Result;
use nexus_common::{get_files_dir_test_pathbuf, ApiConfig};
use nexus_webapi::{api_context::ApiContextBuilder, NexusApiBuilder};
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::{Mutex, OnceCell};

/// Util backend server for testing.
/// Performs the same routine the main service server does.
/// OnceCell is used to ensure the server is only started once.
#[derive(Clone, Debug)]
pub struct TestServiceServer {
    pub initialized: bool,
}

// Global variable to store the server URL.
pub static SERVER_URL: OnceCell<String> = OnceCell::const_new();

pub static TEST_SERVER: OnceCell<Arc<Mutex<TestServiceServer>>> = OnceCell::const_new();

impl TestServiceServer {
    pub async fn get_test_server() -> Arc<Mutex<TestServiceServer>> {
        // Start the server if it hasn't been started
        TEST_SERVER
            .get_or_init(|| async {
                Self::start_server().await.unwrap();
                Arc::new(Mutex::new(TestServiceServer { initialized: true }))
            })
            .await
            .to_owned()
    }

    async fn start_server() -> Result<()> {
        let api_context = ApiContextBuilder::from_default_config_dir()
            .api_config(ApiConfig::default())
            .try_build()
            .await
            .expect("Failed to create ApiContext");

        let nexus_builder = NexusApiBuilder(api_context)
            // Use local port 0 so OS assigns an available port
            .public_addr(SocketAddr::from(([127, 0, 0, 1], 0)))
            .files_path(get_files_dir_test_pathbuf());

        let nexus_api = nexus_builder.start().await.unwrap();

        // Save the server URL, including OS-chosen port (e.g., "http://127.0.0.1:12345") in a global variable
        let url = format!("http://{}", nexus_api.icann_http_socket);
        SERVER_URL.set(url).expect("SERVER_URL already set");

        Ok(())
    }
}
