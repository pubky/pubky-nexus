use std::sync::Arc;

use anyhow::Result;
use log::info;
use pubky_nexus::{routes, Config, StackManager};
use tokio::{
    net::TcpListener,
    sync::{Mutex, OnceCell},
};

// Util backend server for testing
// Performs the same routine the main service server does
// OnceCell is used to ensure the server is only started once
#[derive(Clone, Debug)]
pub struct TestServiceServer {
    pub initialized: bool,
}

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

        // App router
        let app = routes::routes();
        let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
        info!("Listening on {:?}\n", listener.local_addr().unwrap());

        tokio::spawn(async {
            // Start server
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });
        Ok(())
    }
}
