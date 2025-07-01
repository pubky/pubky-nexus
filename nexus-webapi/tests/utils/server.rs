use anyhow::Result;
use nexus_common::FILES_DIR_TEST;
use nexus_webapi::NexusApi;
use std::{net::Ipv4Addr, path::PathBuf, sync::Arc};
use tokio::{
    net::TcpListener,
    sync::{Mutex, OnceCell},
};

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
        let mut nexus_builder = NexusApi::builder();

        // Define IP and port
        let ip = [127, 0, 0, 1];
        // Default to port 0 so OS assigns an available port.
        let port = "0".to_string();
        let binding = format!("{}:{}", Ipv4Addr::from(ip), port);

        // Bind to the address.
        let listener = TcpListener::bind(&binding).await?;
        let local_addr = listener.local_addr()?;
        // Init the stack before create the spawn. if not the app does not have time to initialise the stack and some tests fail
        nexus_builder
            .public_addr(local_addr)
            .files_path(PathBuf::from(FILES_DIR_TEST))
            .init_stack()
            .await
            .unwrap();

        // Save the actual server URL (e.g., "http://127.0.0.1:12345") in a global variable
        let url = format!("http://{local_addr}");
        SERVER_URL.set(url).expect("SERVER_URL already set");

        tokio::spawn(async { nexus_builder.start_test(listener).await });

        Ok(())
    }
}
