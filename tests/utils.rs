use anyhow::Result;
// Util backend server for testing
// Performs the same routine the main service server does
// OnceCell is used to ensure the server is only started once
use log::info;
use pubky_nexus::{redis_is_empty, reindex, routes, setup, Config};
use tokio::{net::TcpListener, sync::OnceCell};

#[derive(Clone, Debug)]
pub struct TestServiceServer {}

// static oncecell for the server
pub static TEST_SERVER: OnceCell<TestServiceServer> = OnceCell::const_new();

impl TestServiceServer {
    pub async fn get_test_server() -> TestServiceServer {
        // Start the server if it hasn't been started
        match TEST_SERVER.get() {
            Some(server) => server.clone(),
            None => {
                let server = Self::start_server().await.unwrap();
                TEST_SERVER
                    .set(server.clone())
                    .expect("Failed to set test server");
                server
            }
        }
    }

    async fn start_server() -> Result<TestServiceServer> {
        tokio::spawn(async {
            let config = Config::from_env();
            setup(&config).await;

            // Reindex if REINDEX is set to true or Redis is empty
            let should_reindex = config.reindex || redis_is_empty().await.unwrap_or(false);

            if should_reindex {
                info!("Starting reindexing process.");
                reindex().await;
            }

            // App router
            let app = routes::routes();

            // Start server
            let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
            info!("Listening on {:?}\n", listener.local_addr().unwrap());
            axum::serve(listener, app.into_make_service())
                .await
                .unwrap();
        });
        Ok(TestServiceServer {})
    }
}
