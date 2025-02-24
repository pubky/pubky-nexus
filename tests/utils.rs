use anyhow::Result;
use pubky_nexus::{routes, Config, StackManager};
use std::sync::Arc;
use tokio::{
    net::TcpListener,
    sync::{Mutex, OnceCell},
};
use tracing::info;

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
        let config = Config::from_env();
        StackManager::setup(&config).await;

        // Read IP and port from environment (or default to dynamic port)
        let ip = "127.0.0.1".to_string();
        // Default to port 0 so OS assigns an available port.
        let port = "0".to_string();
        let binding = format!("{}:{}", ip, port);

        // Bind to the address.
        let listener = TcpListener::bind(&binding).await?;
        let local_addr = listener.local_addr()?;
        info!("Test server listening on {:?}", local_addr);

        // Save the actual server URL (e.g., "http://127.0.0.1:12345") in a global variable.
        let url = format!("http://{}", local_addr);
        SERVER_URL.set(url).expect("SERVER_URL already set");

        let app = routes::routes();
        tokio::spawn(async {
            // Start the server
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });
        Ok(())
    }
}
