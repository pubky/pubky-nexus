use anyhow::Result;
use axum::http::StatusCode;

use crate::utils::{get_request, invalid_get_request};

const PUBKY_TAGGER_ID: &str = "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y";
const PUBKY_TAG_ID: &str = "2Z1N8QBQK9EG0";
const INVALID_PUBKY_TAGGER_ID: &str = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
const INVALID_PUBKY_TAG_ID: &str = "0000000000000";

#[tokio_shared_rt::test(shared)]
async fn test_tag_view() -> Result<()> {
    let path = format!("/v0/tags/{INVALID_PUBKY_TAGGER_ID}/{PUBKY_TAG_ID}");
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

    let path = format!("/v0/tags/{PUBKY_TAGGER_ID}/{INVALID_PUBKY_TAG_ID}");
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

    let path = format!("/v0/tags/{INVALID_PUBKY_TAGGER_ID}/{INVALID_PUBKY_TAG_ID}");
    invalid_get_request(&path, StatusCode::NOT_FOUND).await?;

    let path = format!("/v0/tags/{PUBKY_TAGGER_ID}/{PUBKY_TAG_ID}");
    let body = get_request(&path).await?;

    assert!(body.is_object());

    assert_eq!(body["tag_uri"], "pubky://78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y/pub/pubky.app/tags/2Z1N8QBQK9EG0");
    assert_eq!(body["indexed_at"], 1724134095000 as i64);
    assert_eq!(body["label"], "anti");

    Ok(())
}
