use nexus_common::types::DynError;

use crate::utils::get_request;
pub mod hot;
pub mod post;
pub mod search;
pub mod user;
pub mod utils;
pub mod wot;

// CMD to run test: cargo watch -q -c -w tests/ -x "test tag -- --nocapture"

const PEER_PUBKY: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";

#[tokio_shared_rt::test(shared)]
async fn check_mockups_loaded() -> Result<(), DynError> {
    let endpoint = format!("/v0/user/{PEER_PUBKY}/tags");

    get_request(&endpoint).await?;

    Ok(())
}
