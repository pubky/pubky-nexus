use log::info;
use pubky_nexus::{routes, setup, Config};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    setup(&config).await;

    // App router
    let app = routes::routes();

    // Start server
    let listener = TcpListener::bind(&config.server_binding()).await.unwrap();
    info!("Listening on {:?}\n", listener.local_addr().unwrap());
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
