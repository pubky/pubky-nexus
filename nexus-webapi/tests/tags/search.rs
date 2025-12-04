use crate::utils::get_request;
use anyhow::Result;
use nexus_webapi::routes::v0::endpoints::SEARCH_TAGS_BY_PREFIX_ROUTE;

pub fn format_search_tags_by_prefix(prefix: &str) -> String {
    SEARCH_TAGS_BY_PREFIX_ROUTE.replace("{prefix}", prefix)
}

fn extract_str_vec(res: serde_json::Value) -> Vec<String> {
    assert!(res.is_array());

    res.as_array()
        .expect("Tag search results should be an array")
        .iter()
        .map(|tag| tag.as_str().expect("Tag should be a string").to_string())
        .collect()
}

#[tokio_shared_rt::test(shared)]
async fn test_search_tags_by_prefix() -> Result<()> {
    let label_prefix = "he";
    let url_path = format_search_tags_by_prefix(label_prefix);
    let res = get_request(&url_path).await?;

    let fetched_tags: Vec<String> = extract_str_vec(res);

    let expected_tags = vec!["healthily", "heavily", "hello"];

    assert_eq!(fetched_tags, expected_tags);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_tags_by_prefix_sanitized() -> Result<()> {
    let label_prefix = "he "; // extra space at the end, sanitization should remove it
    let url_path = format_search_tags_by_prefix(label_prefix);
    let res = get_request(&url_path).await?;

    let fetched_tags: Vec<String> = extract_str_vec(res);

    let expected_tags = vec!["healthily", "heavily", "hello"];

    assert_eq!(fetched_tags, expected_tags);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_tags_with_emojis_by_prefix() -> Result<()> {
    let label_prefix = "⭐⭐⭐";
    let url_path = format_search_tags_by_prefix(label_prefix);
    let res = get_request(&url_path).await?;

    let fetched_tags: Vec<String> = extract_str_vec(res);

    let expected_tags = vec!["⭐⭐⭐", "⭐⭐⭐⭐", "⭐⭐⭐⭐⭐"];

    assert_eq!(fetched_tags, expected_tags);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_search_non_existing_prefix() -> Result<()> {
    let non_existing_tag_prefix = "sdfsdf43fsddwt4g";
    let url_path = format_search_tags_by_prefix(non_existing_tag_prefix);
    let body = get_request(&url_path).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
