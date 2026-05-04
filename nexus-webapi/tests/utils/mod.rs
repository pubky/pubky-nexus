use axum::http::{Method, StatusCode};
use base32::{encode, Alphabet};
use serde_json::Value;
use server::TestServiceServer;
use tracing::error;

pub mod server;

pub enum BodyType {
    JSON,
    TEXT,
}

// #######################################
// ##### Test data generators ########
// #######################################

/// Generates a valid PostId by encoding the given timestamp as Crockford Base32.
/// The resulting ID is 13 characters long and decodes to 8 bytes, satisfying PostId validation.
pub fn generate_post_id(timestamp: i64) -> String {
    let id = encode(Alphabet::Crockford, &timestamp.to_be_bytes());
    debug_assert!(
        id.len() == 13,
        "Generated post id '{}' has length {}, expected 13",
        id,
        id.len()
    );
    id
}

pub(crate) async fn host_url() -> String {
    let test_server = TestServiceServer::get_test_server().await;

    // Get the server URL, including OS-chosen port (e.g., "http://127.0.0.1:12345")
    test_server.nexus_api.icann_http_url()
}

// #######################################
// ##### Endpoint requests related #######
// #######################################

pub async fn get_request(endpoint: &str) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(&full_endpoint, None, None, None, BodyType::JSON).await?;
    Ok(body)
}

pub async fn invalid_get_request(
    endpoint: &str,
    error_code: StatusCode,
    body_type: BodyType,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(&full_endpoint, None, None, Some(error_code), body_type).await?;
    Ok(body)
}

pub async fn post_request(endpoint: &str, data: Value) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(
        &full_endpoint,
        Some(Method::POST),
        Some(data),
        None,
        BodyType::JSON,
    )
    .await?;
    Ok(body)
}

pub async fn invalid_post_request(
    endpoint: &str,
    data: Value,
    error_code: StatusCode,
    body_type: BodyType,
) -> Result<Value, httpc_test::Error> {
    let url = host_url().await;
    let full_endpoint = format!("{url}{endpoint}");
    let body = inner_make_request(
        &full_endpoint,
        Some(Method::POST),
        Some(data),
        Some(error_code),
        body_type,
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
    body_type: BodyType,
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

    let body = match body_type {
        BodyType::JSON => match res.json_body() {
            Ok(body) => body,
            Err(e) => {
                eprintln!("Error parsing response body: {e:?}");
                Value::Null
            }
        },
        BodyType::TEXT => match res.text_body() {
            Ok(text) => Value::String(text),
            Err(e) => {
                eprintln!("Error reading text body: {e:?}");
                Value::Null
            }
        },
    };
    Ok(body)
}

#[cfg(test)]
mod utils {

    #[test]
    pub fn test_generate_post_id() {
        println!(
            "Generated new id: {:?}",
            super::generate_post_id(chrono::Utc::now().timestamp())
        );
    }
}
