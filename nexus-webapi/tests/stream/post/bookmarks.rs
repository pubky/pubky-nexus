use super::utils::verify_post_list;
use super::ROOT_PATH;
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
