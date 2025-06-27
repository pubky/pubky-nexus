use axum::http::{Method, StatusCode};
use serde_json::Value;
use server::{TestServiceServer, SERVER_URL};

pub mod server;

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
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(&full_endpoint, None, None, None).await?;
    Ok(body)
}

pub async fn invalid_get_request(
    endpoint: &str,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(&full_endpoint, None, None, Some(error_code)).await?;
    Ok(body)
}

pub async fn post_request(endpoint: &str, data: Value) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(&full_endpoint, Some(Method::POST), Some(data), None).await?;
    Ok(body)
}

pub async fn invalid_post_request(
    endpoint: &str,
    data: Value,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
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
        assert_eq!(res.status(), code, "Expected HTTP status {code}");
    } else {
        assert_eq!(res.status(), 200, "Expected HTTP status 200 OK");
    }

    let body = match res.json_body() {
        Ok(body) => body,
        Err(e) => {
            eprintln!("Error parsing response body: {e:?}");
            Value::Null
        }
    };
    Ok(body)
}
