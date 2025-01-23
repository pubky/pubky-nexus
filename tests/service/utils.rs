use log::info;
use pubky_nexus::{setup::setup_redis, Config};
use serde_json::Value;

pub const HOST_URL: &str = "http://localhost:8080";

// #######################################
// ##### Endpoint requests related #######
// #######################################

// Small unit test to test the endpoint
pub async fn make_request(endpoint: &str) -> Result<Value, httpc_test::Error> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get(endpoint).await?;

    assert_eq!(res.status(), 200);
    let body = res.json_body()?;
    Ok(body)
}

pub async fn make_wrong_request(
    endpoint: &str,
    error_code: Option<u16>,
) -> Result<(), httpc_test::Error> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get(endpoint).await?;

    assert_eq!(res.status(), error_code.unwrap_or(404));
    Ok(())
}

pub async fn connect_to_redis() {
    let config = Config::from_env();

    match env_logger::try_init() {
        Ok(_) => info!("Env logger initiated"),
        Err(err) => assert!(false, "{}", format!("Env logger was already set: {}", err)),
    }
    setup_redis(&config).await;
}
