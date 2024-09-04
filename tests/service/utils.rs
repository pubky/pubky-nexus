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

pub async fn make_wrong_request(endpoint: &str) -> Result<(), httpc_test::Error> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get(endpoint).await?;

    assert_eq!(res.status(), 404);
    Ok(())
}
