#![allow(unused)]

use tokio::net::TcpListener;
use axum::Router;
use axum::routing::get;
use axum::response::Html;

#[tokio::main]
async fn main() {
    let routes_hello = Router::new().route("/hello", get(|| async {Html("Hello world!")}),);

    // start server
    let listener = TcpListener::bind("127.0.0.1:8080").await.unwrap();
	println!("->> LISTENING on {:?}\n", listener.local_addr());
	axum::serve(listener, routes_hello.into_make_service())
		.await
		.unwrap();
}
