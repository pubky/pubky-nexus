#![allow(unused)]

use axum::response::Html;
use axum::routing::get;
use axum::Router;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let routes_hello = Router::new().route("/hello", get(|| async { Html("Hello world!") }));

    // start server
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
    println!("->> LISTENING on {:?}\n", listener.local_addr());
    axum::serve(listener, routes_hello.into_make_service())
        .await
        .unwrap();
}
