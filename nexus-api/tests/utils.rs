use nexus_api::builder::NexusApi;
use reqwest::{Method, StatusCode};
use serde_json::Value;

/// Instead of hardcoding the host, we have a function that returns it.
pub async fn host_url() -> String {
    // Ensure the server is running.
    TestServiceServer::get_test_server().await;
    // Get the URL that was stored when the server started.
    SERVER_URL.get().expect("SERVER_URL should be set").clone()
}

// #######################################
// ##### Endpoint requests related #######
// #######################################

pub async fn get_request(endpoint: &str) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{}{}", url, endpoint);
    let body = inner_make_request(&full_endpoint, None, None, None).await?;
    Ok(body)
}

pub async fn invalid_get_request(
    endpoint: &str,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{}{}", url, endpoint);
    let body = inner_make_request(&full_endpoint, None, None, Some(error_code)).await?;
    Ok(body)
}

pub async fn post_request(endpoint: &str, data: Value) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{}{}", url, endpoint);
    let body = inner_make_request(&full_endpoint, Some(Method::POST), Some(data), None).await?;
    Ok(body)
}

pub async fn invalid_post_request(
    endpoint: &str,
    data: Value,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{}{}", url, endpoint);
    let body = inner_make_request(
        &full_endpoint,
        Some(Method::POST),
        Some(data),
        Some(error_code),
    )
    .await?;
    Ok(body)
}

// Small helper function to send requests.
async fn inner_make_request(
    endpoint: &str,
    method: Option<Method>,
    data: Option<Value>,
    error_code: Option<StatusCode>,
) -> Result<Value, httpc_test::Error> {
    let client = httpc_test::new_client("")?; // now client doesn't need a hardcoded host

    let request_method = method.unwrap_or(Method::GET);
    let res = match request_method {
        Method::GET => client.do_get(endpoint).await?,
        Method::POST => {
            client
                .do_post(endpoint, data.unwrap_or(Value::Null))
                .await?
        }
        Method::PUT => client.do_put(endpoint, data.unwrap_or(Value::Null)).await?,
        Method::DELETE => client.do_delete(endpoint).await?,
        _ => panic!("Unsupported method"),
    };

    if let Some(code) = error_code {
        assert_eq!(res.status(), code, "Expected HTTP status {}", code);
    } else {
        assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");
    }

    let body = match res.json_body() {
        Ok(body) => body,
        Err(e) => {
            eprintln!("Error parsing response body: {:?}", e);
            Value::Null
        }
    };
    Ok(body)
}

// TODO: utils.rs and this code has to be inside a utils folder

use anyhow::Result;
use std::{net::Ipv4Addr, sync::Arc};
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
        let binding = format!("{}:{}", Ipv4Addr::from(ip).to_string(), port);

        // Bind to the address.
        let listener = TcpListener::bind(&binding).await?;
        let local_addr = listener.local_addr()?;
        // Init the stack before create the spawn. if not the app does not have time to initialise the stack and some tests fail
        nexus_builder
            .public_addr(local_addr)
            .init_stack()
            .await
            .unwrap();

        // Save the actual server URL (e.g., "http://127.0.0.1:12345") in a global variable
        let url = format!("http://{}", local_addr);
        SERVER_URL.set(url).expect("SERVER_URL already set");

        tokio::spawn(async { nexus_builder.run_test(listener).await });

        Ok(())
    }
}
