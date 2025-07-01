use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;

pub fn format_search_tags_by_prefix(prefix: &str) -> String {
    SEARCH_TAGS_BY_PREFIX_ROUTE.replace("{prefix}", prefix)
}

#[tokio_shared_rt::test(shared)]
async fn test_search_tags_by_prefix() -> Result<()> {
    let label_prefix = "he";
    let url_path = format_search_tags_by_prefix(label_prefix);
    let res = get_request(&url_path).await?;

    assert!(res.is_array());

    let fetched_tags: Vec<String> = res
        .as_array()
        .expect("Tag search results should be an array")
        .iter()
        .map(|tag| tag.as_str().expect("Tag should be a string").to_string())
        .collect();

    let expected_tags = vec!["healthily", "heavily", "hello"];

    assert_eq!(fetched_tags, expected_tags);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_tags_by_prefix_sanitized() -> Result<()> {
    let label_prefix = "he "; // extra space at the end, sanitization should remove it
    let url_path = format_search_tags_by_prefix(label_prefix);
    let res = get_request(&url_path).await?;

    assert!(res.is_array());

    let fetched_tags: Vec<String> = res
        .as_array()
        .expect("Tag search results should be an array")
        .iter()
        .map(|tag| tag.as_str().expect("Tag should be a string").to_string())
        .collect();

    let expected_tags = vec!["healthily", "heavily", "hello"];

    assert_eq!(fetched_tags, expected_tags);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_non_existing_prefix() -> Result<()> {
    let non_existing_tag_prefix = "sdfsdf43fsddwt4g";
    let url_path = format_search_tags_by_prefix(non_existing_tag_prefix);
    let res = invalid_get_request(&url_path, StatusCode::NOT_FOUND).await?;

    assert!(res["error"].is_string(), "Error message should be a string");
    assert!(matches!(res["error"].as_str(), Some("Tags not found")));

    Ok(())
}
