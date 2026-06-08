use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::models::ErrorResponsePayload;
use nexus_webapi::routes::v0::endpoints::{
    SEARCH_POSTS_BY_CONTENT_ROUTE, SEARCH_POSTS_BY_TAG_ROUTE,
};
use serde_json::Value;

use crate::{
    stream::post::TAG_LABEL_2,
    utils::{get_request, invalid_get_request},
};

const POST_A: &str = "2VDW8YBDZJ02";
const POST_B: &str = "1TDV7XBCF4M1";
const POST_C: &str = "HC3T5CEPBPHQQ";

pub fn format_search_posts_by_tag(tag: &str) -> String {
    SEARCH_POSTS_BY_TAG_ROUTE.replace("{tag}", tag)
}

fn search_posts_by_tag_free() -> String {
    format_search_posts_by_tag("free")
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_by_timeline() -> Result<()> {
    let post_order = vec![POST_A, POST_B, POST_C];
    let path = search_posts_by_tag_free();
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 3);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_skip() -> Result<()> {
    let post_order = vec![POST_B, POST_C];
    let path = format!("{}?skip=1", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_limit() -> Result<()> {
    let post_order = vec![POST_A];
    let path = format!("{}?limit=1", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let posts = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(posts.len(), 1);

    // Validate that each post has the searched tag
    search_posts(posts, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_with_limit_and_skip() -> Result<()> {
    let post_order = vec![POST_C];
    let path = format!("{}?limit=1&skip=2", search_posts_by_tag_free());
    let body = get_request(&path).await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream posts should be an array");

    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // Validate that each post has the searched tag
    search_posts(tags, post_order);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_rejects_invalid_tag() -> Result<()> {
    let over_length_tag = "a".repeat(21);
    let path = format_search_posts_by_tag(&over_length_tag);
    let res = invalid_get_request(&path, StatusCode::BAD_REQUEST).await?;

    let error_response: ErrorResponsePayload =
        serde_json::from_value(res).expect("Response should be a valid ErrorResponsePayload");
    assert!(
        error_response.error.contains("20")
            || error_response.error.to_lowercase().contains("maximum"),
        "Error message should mention the maximum length limit, got: {}",
        error_response.error
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_specific_tag_with_no_result() -> Result<()> {
    let path = format_search_posts_by_tag("randommm");
    let body = get_request(&path).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}

// ── Content search tests ──────────────────────────────────────────────────────

fn content_search_url(q: &str) -> String {
    format!("{SEARCH_POSTS_BY_CONTENT_ROUTE}?q={q}")
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_exact_match() -> Result<()> {
    // "Julian Assange is free" is in the seed data
    let body = get_request(&content_search_url("Assange")).await?;
    let results = body.as_array().expect("should be array");
    assert!(!results.is_empty(), "expected at least one result");
    assert!(
        results
            .iter()
            .all(|r| r.get("post_key").is_some() && r.get("score").is_some()),
        "each result must have post_key and score"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_no_results() -> Result<()> {
    let body = get_request(&content_search_url("xyzzyunmatchabletoken")).await?;
    let results = body.as_array().expect("should be array");
    assert!(results.is_empty(), "expected no results for unknown term");
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_hyphenated_term() -> Result<()> {
    // "e-mail" in the query must be treated as two tokens ("e" and "mail"), matching RediSearch's
    // own tokenization of the indexed content — not as "e NOT mail" (raw injection) or "email"
    // (stripped concatenation). The seed post "...via e-mail notifications" must be returned.
    let body = get_request(&content_search_url("e-mail")).await?;
    let results = body.as_array().expect("should be array");
    assert!(
        !results.is_empty(),
        "searching 'e-mail' should find the seed post containing 'e-mail'"
    );
    assert!(
        results.iter().any(|r| r["post_key"]
            .as_str()
            .is_some_and(|k| k.ends_with(":00000039YD9DA"))),
        "seed post 00000039YD9DA ('...via e-mail notifications') must be in results"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_collection_post() -> Result<()> {
    // Collection posts are indexed via their raw JSON content envelope, so searching
    // for a word from the collection name finds the collection.
    // COLW1TGL5BKG3 has content {"name":"Cryptography classics","items":[]}.
    let body = get_request(&content_search_url("cryptography")).await?;
    let results = body.as_array().expect("should be array");
    assert!(
        !results.is_empty(),
        "searching 'cryptography' should find the collection post"
    );
    assert!(
        results.iter().any(|r| r["post_key"]
            .as_str()
            .is_some_and(|k| k.ends_with(":COLW1TGL5BKG3"))),
        "collection post COLW1TGL5BKG3 ('Cryptography classics') must be in results"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_fuzzy_match() -> Result<()> {
    // "disappearing" is in the seed data; "disapearing" (one 'p') is edit-distance 1
    let url = format!("{SEARCH_POSTS_BY_CONTENT_ROUTE}?q=disapearing");
    let body = get_request(&url).await?;
    let results = body.as_array().expect("should be array");
    assert!(
        !results.is_empty(),
        "server-side fuzzy match should find 'disappearing'"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_too_short_query_rejected() -> Result<()> {
    let res = invalid_get_request(&content_search_url("a"), StatusCode::BAD_REQUEST).await?;
    let error_response: ErrorResponsePayload =
        serde_json::from_value(res).expect("should be ErrorResponsePayload");
    assert!(
        error_response.error.contains("2") || error_response.error.to_lowercase().contains("least"),
        "error should mention minimum length, got: {}",
        error_response.error
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_pagination() -> Result<()> {
    // "amendment" appears in multiple seed posts
    let url_all = format!("{SEARCH_POSTS_BY_CONTENT_ROUTE}?q=amendment&limit=10");
    let all = get_request(&url_all).await?;
    let all = all.as_array().expect("should be array");

    let url_skip = format!("{SEARCH_POSTS_BY_CONTENT_ROUTE}?q=amendment&limit=10&skip=1");
    let skipped = get_request(&url_skip).await?;
    let skipped = skipped.as_array().expect("should be array");

    if all.len() > 1 {
        assert_eq!(
            skipped.len(),
            all.len() - 1,
            "skip=1 should return one fewer result"
        );
        assert_eq!(
            skipped[0]["post_key"], all[1]["post_key"],
            "first result after skip should match second result without skip"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_content_search_skip_over_max_rejected() -> Result<()> {
    let res = invalid_get_request(
        &format!("{SEARCH_POSTS_BY_CONTENT_ROUTE}?q=test&skip=1001"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    let error_response: ErrorResponsePayload =
        serde_json::from_value(res).expect("should be ErrorResponsePayload");
    assert!(
        error_response.error.contains("1000")
            || error_response.error.to_lowercase().contains("maximum"),
        "error should mention the maximum offset, got: {}",
        error_response.error
    );
    Ok(())
}

fn search_posts(posts: &[Value], post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
        let post_parts: Vec<&str> = post["post_key"].as_str().unwrap().split(':').collect();
        // Check if the order of the post is the right one
        assert_eq!(
            post_parts[1], post_order[index],
            "The post does not have the right ordering"
        );
    }
}

#[tokio_shared_rt::test(shared)]
async fn test_post_search_skip_beyond_range() -> Result<()> {
    // Search opensource tag
    let path = format_search_posts_by_tag(TAG_LABEL_2);

    let body = get_request(&path).await?;
    let length = body.as_array().expect("Post list should be an array").len();

    assert!(body.is_array());

    let path_w_skip = format!(
        "{}?skip={}",
        format_search_posts_by_tag(TAG_LABEL_2),
        length
    );
    let body = get_request(&path_w_skip).await?;

    assert!(body.is_array());
    assert!(body.as_array().unwrap().is_empty());

    Ok(())
}
