use reqwest::{Method, StatusCode};
use serde_json::Value;

use crate::utils::TestServiceServer;

pub const HOST_URL: &str = "http://localhost:8080";

// #######################################
// ##### Endpoint requests related #######
// #######################################

pub async fn get_request(endpoint: &str) -> Result<Value, httpc_test::Error> {
    let body = inner_make_request(endpoint, None, None, None).await?;
    Ok(body)
}

pub async fn invalid_get_request(
    endpoint: &str,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let body = inner_make_request(endpoint, None, None, Some(error_code)).await?;
    Ok(body)
}

pub async fn post_request(endpoint: &str, data: Value) -> Result<Value, httpc_test::Error> {
    let body = inner_make_request(endpoint, Some(Method::POST), Some(data), None).await?;
    Ok(body)
}

pub async fn invalid_post_request(
    endpoint: &str,
    data: Value,
    error_code: StatusCode,
) -> Result<Value, httpc_test::Error> {
    let body =
        inner_make_request(endpoint, Some(Method::POST), Some(data), Some(error_code)).await?;
    Ok(body)
}

// Small unit test to test the endpoint
async fn inner_make_request(
    endpoint: &str,
    method: Option<Method>,
    data: Option<Value>,
    error_code: Option<StatusCode>,
) -> Result<Value, httpc_test::Error> {
    // make sure server is running
    TestServiceServer::get_test_server().await;
    let client = httpc_test::new_client(HOST_URL)?;

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

    match error_code {
        Some(code) => assert_eq!(res.status(), code, "Expected HTTP status {}", code),
        None => assert_eq!(res.status(), 200, "Expected HTTP status 200 OK"),
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
