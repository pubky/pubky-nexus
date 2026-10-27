use super::utils::{post_keys_in, verify_post_list};
use super::{KEYS_ROOT_PATH, ROOT_PATH};
use crate::utils::get_request;
use anyhow::Result;

// User with most bookmarks
const BOOKMARKER_ID: &str = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";

#[tokio_shared_rt::test(shared)]
async fn test_stream_bookmarked_posts() -> Result<()> {
    let observer_id = BOOKMARKER_ID;
    let path = format!("{ROOT_PATH}?observer_id={observer_id}&source=bookmarks");
    let body = get_request(&path).await?;

    assert!(body.is_array());

    for post in body.as_array().expect("Post stream should be an array") {
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

// Post order by timeline
pub const POST_TA: &str = "2ZDZ87NZ4H700";
pub const POST_TB: &str = "2ZD6BK61CRT00";
pub const POST_TC: &str = "2Z9GWEBYKY400";
pub const POST_TD: &str = "2ZDZR2G775W00";
pub const POST_TE: &str = "2ZDZK595DDRG0";
pub const POST_TF: &str = "2ZDZHGVTQV600";
pub const POST_TG: &str = "2ZDZ7PM0JVK00";
pub const POST_TH: &str = "2ZDZ4DTKRJ900";
pub const POST_TI: &str = "2ZECRNM66G900";
pub const POST_TJ: &str = "2ZD6JHJQ6MZG0";
pub const POST_TK: &str = "2ZAX1DBDD5YG0";

pub const START_TIMELINE: &str = "1724134149050";
pub const END_TIMELINE: &str = "1724134141150";

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_bookmarks_by_timeline_with_start() -> Result<()> {
    let path =
        format!("{ROOT_PATH}?observer_id={BOOKMARKER_ID}&source=bookmarks&start={START_TIMELINE}");

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TC, POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI, POST_TJ, POST_TK,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_bookmarks_by_timeline_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOOKMARKER_ID}&source=bookmarks&start={START_TIMELINE}&end={END_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TC, POST_TD, POST_TE, POST_TF, POST_TG, POST_TH, POST_TI,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

/// In ascending order too, `start` is the highest score kept and `end` the
/// lowest.
#[tokio_shared_rt::test(shared)]
async fn test_stream_user_bookmarks_ascending_with_start_and_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOOKMARKER_ID}&source=bookmarks&start={START_TIMELINE}&end={END_TIMELINE}&order=ascending"
    );

    let body = get_request(&path).await?;
    let post_list = vec![
        POST_TI, POST_TH, POST_TG, POST_TF, POST_TE, POST_TD, POST_TC,
    ];
    verify_post_list(post_list, body);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_stream_user_bookmarks_by_timeline_with_skip_end() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}?observer_id={BOOKMARKER_ID}&source=bookmarks&limit=5&end={END_TIMELINE}"
    );

    let body = get_request(&path).await?;
    let post_list = vec![POST_TA, POST_TB, POST_TC, POST_TD, POST_TE];
    verify_post_list(post_list, body);

    Ok(())
}

// Bookmarked 9 posts in the same millisecond
const TIED_BOOKMARKER_ID: &str = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
const TIED_SCORE: u64 = 1724134095000;

/// Keys and `last_post_score` of a bookmarks stream page.
async fn bookmark_keys(query: &str) -> Result<(Vec<String>, Option<u64>)> {
    let body = get_request(&format!("{KEYS_ROOT_PATH}?source=bookmarks&{query}")).await?;
    Ok((post_keys_in(&body), body["last_post_score"].as_u64()))
}

/// Equal bookmark times are ordered by post id, so `skip` pages neither repeat
/// nor drop a bookmark.
#[tokio_shared_rt::test(shared)]
async fn test_stream_bookmarks_pages_through_ties_by_post_id() -> Result<()> {
    let observer = format!("observer_id={TIED_BOOKMARKER_ID}");
    let (all, last_score) = bookmark_keys(&format!("{observer}&limit=50")).await?;
    assert_eq!(all.len(), 9, "the fixture has 9 tied bookmarks: {all:?}");
    assert_eq!(last_score, Some(TIED_SCORE));

    let post_ids: Vec<&str> = all
        .iter()
        .map(|key| key.split_once(':').expect("author:post key").1)
        .collect();
    let mut by_post_id = post_ids.clone();
    by_post_id.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(post_ids, by_post_id, "ties are ordered by post id");

    let mut paged = Vec::new();
    for skip in [0, 4, 8] {
        let (page, _) = bookmark_keys(&format!("{observer}&skip={skip}&limit=4")).await?;
        paged.extend(page);
    }
    assert_eq!(paged, all, "pages must not repeat or drop a tied bookmark");

    let mut ascending = Vec::new();
    for skip in [0, 4, 8] {
        let (page, _) =
            bookmark_keys(&format!("{observer}&order=ascending&skip={skip}&limit=4")).await?;
        ascending.extend(page);
    }
    let reversed: Vec<String> = all.iter().rev().cloned().collect();
    assert_eq!(
        ascending, reversed,
        "ascending pages run the ties backwards"
    );

    // Both bounds are inclusive
    let (at_score, _) = bookmark_keys(&format!(
        "{observer}&start={TIED_SCORE}&end={TIED_SCORE}&limit=50"
    ))
    .await?;
    assert_eq!(at_score, all);
    let (below, _) = bookmark_keys(&format!("{observer}&start={}", TIED_SCORE - 1)).await?;
    assert!(below.is_empty(), "nothing is bookmarked before the tie");

    Ok(())
}

// Bookmarked one post over 15 edges, from 1724134095000 to 1724134109000; it
// is their oldest bookmark
const REPEAT_BOOKMARKER_ID: &str = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";
const REPEATED_POST_KEY: &str =
    "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro:2Z9P8AN738C00";

/// A post bookmarked over several edges is listed once, at its latest
/// bookmark time.
#[tokio_shared_rt::test(shared)]
async fn test_stream_bookmarks_lists_a_repeated_bookmark_once_at_its_latest() -> Result<()> {
    let (keys, last_score) =
        bookmark_keys(&format!("observer_id={REPEAT_BOOKMARKER_ID}&limit=50")).await?;

    let listed = keys.iter().filter(|key| *key == REPEATED_POST_KEY).count();
    assert_eq!(listed, 1, "the repeated bookmark is listed once: {keys:?}");
    assert_eq!(keys.last().map(String::as_str), Some(REPEATED_POST_KEY));
    assert_eq!(last_score, Some(1724134109000));

    Ok(())
}
