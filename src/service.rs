use pubky_nexus::{redis_is_empty, reindex, routes, Config, StackManager};
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    StackManager::setup(&config).await;

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
}
