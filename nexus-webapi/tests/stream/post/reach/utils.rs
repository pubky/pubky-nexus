use crate::stream::post::utils::{verify_post_list, verify_timeline_post_list};
use crate::stream::post::ROOT_PATH;
use crate::utils::get_request;
use anyhow::Result;

// Test all the reach endpoints that hits the graph
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

    if verify_timeline {
        verify_timeline_post_list(expected_posts.to_vec(), body);
    } else {
        verify_post_list(expected_posts.to_vec(), body);
    }

    Ok(())
}
