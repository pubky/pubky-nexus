use crate::stream::post::utils::{verify_post_list, verify_timeline_post_list};
use crate::stream::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;
use serde_json::Value;

pub fn assert_excludes_author(body: &Value, observer_id: &str, source: &str) {
    let posts = body.as_array().expect("Post stream should be an array");
    assert!(
        !posts.is_empty(),
        "{source} regression fixture must return posts"
    );
    for post in posts {
        let author = post["details"]["author"]
            .as_str()
            .expect("post author should be a string");
        assert_ne!(
            author, observer_id,
            "{source} must not include posts authored by its observer"
        );
    }
}

/// Tests reach endpoints that use the graph path.
#[allow(clippy::too_many_arguments)]
pub async fn test_reach_filter_with_posts(
    user_id: &str,
    sorting: Option<&str>,
    source: &str,
    tags: Option<&str>,
    start: Option<&str>,
    end: Option<&str>,
    skip: Option<usize>,
    limit: Option<usize>,
    expected_posts: &[&str],
) -> Result<()> {
    let mut path = format!("{ROOT_PATH}?observer_id={user_id}&source={source}");

    let mut verify_timeline = true;

    if let Some(sorting) = sorting {
        path.push_str(&format!("&sorting={sorting}"));
        verify_timeline = false;
    }
    if let Some(tags) = tags {
        path.push_str(&format!("&tags={tags}"));
    }
    if let Some(start) = start {
        path.push_str(&format!("&start={start}"));
    }
    if let Some(end) = end {
        path.push_str(&format!("&end={end}"));
    }
    if let Some(skip) = skip {
        path.push_str(&format!("&skip={skip}"));
    }
    if let Some(limit) = limit {
        path.push_str(&format!("&limit={limit}"));
    }

    println!("PATH: {path:?}");

    let body = get_request(&path).await?;
    assert_excludes_author(&body, user_id, source);

    if verify_timeline {
        verify_timeline_post_list(expected_posts.to_vec(), body);
    } else {
        verify_post_list(expected_posts.to_vec(), body);
    }

    Ok(())
}
