use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

use crate::utils::{get_request, invalid_get_request};

const PEER_PUBKY: &str = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";
// mocks/hot-tags.cypher users
pub const USER_1: &str = "pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo";
const USER_4: &str = "r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y";
const USER_5: &str = "tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo";

struct StreamTagMockup {
    label: String,
    tagger_ids: usize,
    tagged_count: u64,
    taggers_count: usize,
}

impl StreamTagMockup {
    fn new(label: String, tagger_ids: usize, tagged_count: u64, taggers_count: usize) -> Self {
        Self {
            label,
            tagger_ids,
            tagged_count,
            taggers_count,
        }
    }
}

// Small unit test to compare all the tags composition
fn analyse_hot_tags_structure(tags: &Vec<Value>) {
    for tag in tags {
        assert!(tag["label"].is_string(), "label should be a string");
        assert!(
            tag["taggers_id"].is_array(),
            "tagger_ids should be an array"
        );
        assert!(
            tag["tagged_count"].is_number(),
            "tagged_count should be a number"
        );
        assert!(
            tag["taggers_count"].is_number(),
            "taggers_count should be a number"
        );
    }
}

// Small unit test to compare the tag properties
fn compare_unit_hot_tag(tag: &Value, hot_tag: StreamTagMockup) {
    assert_eq!(tag["tagged_count"], hot_tag.tagged_count);
    assert_eq!(tag["label"], hot_tag.label);
    assert_eq!(tag["taggers_count"], hot_tag.taggers_count);
    let tagger_ids = tag["taggers_id"].as_array().unwrap();
    assert_eq!(tagger_ids.len(), hot_tag.tagger_ids);
}

#[tokio_shared_rt::test(shared)]
async fn test_global_hot_tags() -> Result<()> {
    let body = get_request("/v0/tags/hot").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 4th index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 9, 15, 9);
    compare_unit_hot_tag(&tags[4], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_global_hot_tags_with_today_timeframe() -> Result<()> {
    let body = get_request("/v0/tags/hot?timeframe=today").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    let hot_tag = StreamTagMockup::new(String::from("today"), 3, 3, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_global_hot_tags_with_this_month_timeframe() -> Result<()> {
    let body = get_request("/v0/tags/hot?timeframe=this_month").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the first tag
    let hot_tag = StreamTagMockup::new(String::from("today"), 3, 3, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_global_hot_tags_skip_limit() -> Result<()> {
    let body = get_request("/v0/tags/hot?skip=3&limit=5").await?;

    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // assert limit
    assert_eq!(tags.len(), 5);

    // Analyse the first tag
    let hot_tag = StreamTagMockup::new(String::from("ha"), 9, 16, 9);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=following");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 4, 5, 4);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach_and_today_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_1}&reach=following&timeframe=today");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 3);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("today"), 2, 2, 2);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach_and_month_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_1}&reach=following&timeframe=this_month");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 5);
    let hot_tag = StreamTagMockup::new(String::from("month"), 3, 2, 3);

    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach_and_month_timeframe_skip_and_limit() -> Result<()> {
    let endpoint = &format!(
        "/v0/tags/hot?user_id={USER_1}&reach=following&timeframe=this_month&skip=1&limit=2"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 2);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("today"), 2, 2, 2);
    compare_unit_hot_tag(&tags[0], hot_tag);
    let hot_tag = StreamTagMockup::new(String::from("tag1"), 3, 1, 3);
    compare_unit_hot_tag(&tags[1], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach_and_all_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_1}&reach=following&timeframe=all_time");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 6);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("all"), 4, 2, 4);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_reach_with_skip_limit() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=following&skip=3&limit=1",);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("💯"), 1, 3, 1);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_reach_no_user_id() -> Result<()> {
    let endpoint = "/v0/tags/hot?reach=following";

    invalid_get_request(endpoint, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_reach_no_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}");

    invalid_get_request(endpoint, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_following_using_taggers_limit() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=following&taggers_limit=3",);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 3, 5, 4);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=followers");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 2, 3, 2);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach_with_skip_limit() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=followers&skip=3&limit=1",);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("✅"), 1, 2, 1);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach_and_today_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_4}&reach=followers&timeframe=today");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 3);

    let hot_tag = StreamTagMockup::new(String::from("today"), 3, 3, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach_and_month_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_4}&reach=followers&timeframe=this_month");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 5);

    let hot_tag = StreamTagMockup::new(String::from("month"), 3, 3, 3);

    // Analyse the tag that is in the 0 index
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach_and_month_timeframe_skip_and_limit() -> Result<()> {
    let endpoint = &format!(
        "/v0/tags/hot?user_id={USER_4}&reach=followers&timeframe=this_month&skip=1&limit=2"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 2);

    let hot_tag = StreamTagMockup::new(String::from("today"), 3, 3, 3);
    compare_unit_hot_tag(&tags[0], hot_tag);
    let hot_tag = StreamTagMockup::new(String::from("tag1"), 3, 2, 3);
    compare_unit_hot_tag(&tags[1], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_followers_reach_and_all_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_4}&reach=followers&timeframe=all_time");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 6);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("all"), 4, 3, 4);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=friends");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 1st index
    let hot_tag = StreamTagMockup::new(String::from("pubky"), 2, 3, 2);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach_with_skip_limit() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={PEER_PUBKY}&reach=friends&skip=2&limit=1",);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("bitkit"), 2, 2, 2);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach_and_today_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_5}&reach=friends&timeframe=today");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 1);

    let hot_tag = StreamTagMockup::new(String::from("today"), 1, 1, 1);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach_and_month_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_5}&reach=friends&timeframe=this_month");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 4);
    let hot_tag = StreamTagMockup::new(String::from("month"), 1, 1, 1);

    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach_and_month_timeframe_skip_and_limit() -> Result<()> {
    let endpoint =
        &format!("/v0/tags/hot?user_id={USER_5}&reach=friends&timeframe=this_month&skip=1&limit=2");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 2);

    let hot_tag = StreamTagMockup::new(String::from("tag1"), 1, 1, 1);
    compare_unit_hot_tag(&tags[0], hot_tag);
    let hot_tag = StreamTagMockup::new(String::from("tag2"), 1, 1, 1);
    compare_unit_hot_tag(&tags[1], hot_tag);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_by_friends_reach_and_all_timeframe() -> Result<()> {
    let endpoint = &format!("/v0/tags/hot?user_id={USER_5}&reach=friends&timeframe=all_time");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let tags = body.as_array().expect("Stream tags should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    analyse_hot_tags_structure(tags);

    assert_eq!(tags.len(), 6);

    // Analyse the tag that is in the 0 index
    let hot_tag = StreamTagMockup::new(String::from("all"), 1, 1, 1);
    compare_unit_hot_tag(&tags[0], hot_tag);

    Ok(())
}

const PUBKY_TAG: &str = "pubky";

const TAGGERS: [&str; 9] = [
    "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
    "s1empmp4x6owkewyijcbnn1faejhhu536w8i7n9oqh57om9qjfho",
    "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy",
    "7w4hmktqa7gia5thmk7zki8px7ttwpwjtgaaaou4tbqx64re8d1o",
    "ze86rtgp6x1qdyno4uzp8gexbb887dtemmonoh4j3iisbzitcppo",
    "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso",
    "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
    "omynbjw4ksjc4at5gretyoatw1g5h53tkee5z55fh69sng1d3jpy",
    "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
];

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers() -> Result<()> {
    let endpoint = &format!("/v0/tags/taggers/{PUBKY_TAG}");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 9);

    for (index, tagger) in TAGGERS.into_iter().enumerate() {
        assert_eq!(TAGGERS[index], tagger);
    }

    let endpoint = &format!("/v0/tags/taggers/{PUBKY_TAG}?skip=4&limit=2");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers_with_filters = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers_with_filters.len(), 2);

    let skip_and_limit_taggers: Vec<String> = taggers
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .skip(4)
        .take(2)
        .collect();

    for (index, tagger) in taggers_with_filters.iter().enumerate() {
        assert_eq!(&skip_and_limit_taggers[index], tagger);
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_skip_limit_and_timeframe() -> Result<()> {
    let endpoint = &format!(
        "/v0/tags/taggers/{}?skip=1&limit=2&timeframe=this_month",
        "tag1"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 2);

    assert_eq!(
        &taggers[0],
        "qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy"
    );
    assert_eq!(
        &taggers[1],
        "r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo"
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_following() -> Result<()> {
    let endpoint = &format!("/v0/tags/taggers/{PUBKY_TAG}?reach=following&user_id={PEER_PUBKY}");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 4);

    assert_eq!(
        &taggers[0],
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"
    );

    assert_eq!(
        &taggers[2],
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_following_skip_and_limit() -> Result<()> {
    let endpoint = &format!(
        "/v0/tags/taggers/{PUBKY_TAG}?reach=following&user_id={PEER_PUBKY}&skip=1&limit=2"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 2);

    assert_eq!(
        &taggers[0],
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"
    );

    assert_eq!(
        &taggers[1],
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_followers() -> Result<()> {
    let endpoint = &format!("/v0/tags/taggers/{PUBKY_TAG}?reach=followers&user_id={PEER_PUBKY}");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 2);

    assert_eq!(
        &taggers[0],
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"
    );

    assert_eq!(
        &taggers[1],
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_followers_skip_and_limit() -> Result<()> {
    let endpoint = &format!(
        "/v0/tags/taggers/{PUBKY_TAG}?reach=followers&user_id={PEER_PUBKY}&skip=1&limit=1"
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 1);

    assert_eq!(
        &taggers[0],
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_friends() -> Result<()> {
    let endpoint = &format!("/v0/tags/taggers/{PUBKY_TAG}?reach=friends&user_id={PEER_PUBKY}");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 2);

    assert_eq!(
        &taggers[0],
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy"
    );

    assert_eq!(
        &taggers[1],
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_hot_tags_label_taggers_with_reach_friends_skip_and_limit() -> Result<()> {
    let endpoint =
        &format!("/v0/tags/taggers/{PUBKY_TAG}?reach=friends&user_id={PEER_PUBKY}&skip=1&limit=1");

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let taggers = body.as_array().expect("Taggers ids should be an array");
    assert_eq!(taggers.len(), 1);

    assert_eq!(
        &taggers[0],
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro"
    );

    Ok(())
}
