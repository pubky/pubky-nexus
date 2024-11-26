use pubky_nexus::types::DynError;

use crate::service::utils::HOST_URL;

pub mod hot;
pub mod post;
pub mod search;
pub mod user;
pub mod utils;
pub mod wot;

// CMD to run test: cargo watch -q -c -w tests/ -x "test tag -- --nocapture"

const PEER_PUBKY: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";

#[tokio::test]
async fn check_mockups_loaded() -> Result<(), DynError> {
    let endpoint = format!("/v0/user/{}/tags", PEER_PUBKY);

    let client = httpc_test::new_client(HOST_URL)?;
    let res = client.do_get(&endpoint).await?;

    assert_eq!(
        res.status(),
        200,
        "Check if the tags.cypher graph is imported before run that tests"
    );

    Ok(())
}
