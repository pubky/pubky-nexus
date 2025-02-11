use anyhow::Result;
use reqwest::StatusCode;

use crate::service::utils::{get_request, invalid_get_request};

// TODO: Create deterministic integration tests

const PEER_PUBKY: &str = "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo";

#[tokio::test]
async fn test_global_influencers() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
        "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
    ];

    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio::test]
async fn test_global_influencers_preview() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&preview=true").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    // assert preview size is respected
    assert_eq!(influencers.len(), 3);

    let first_influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // make the request a second time to ensure the preview is generating different results
    let body = get_request("/v0/stream/users?source=influencers&preview=true").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    assert!(!influencers.is_empty(), "Influencers should not be empty");

    // assert preview size is respected
    assert_eq!(influencers.len(), 3);

    let second_influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    assert!(first_influencer_ids != second_influencer_ids);

    Ok(())
}

#[tokio::test]
async fn test_global_influencers_skip_limit() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&skip=3&limit=3").await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    // assert limit
    assert_eq!(influencers.len(), 3);

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so",
    ];

    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio::test]
async fn test_global_influencers_with_today_timeframe() -> Result<()> {
    let body = get_request("/v0/stream/users?source=influencers&timeframe=today&limit=4").await?;

    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y",
        "qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy",
        "r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo",
        "tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = influencer_ids.clone().into_iter().any(|item| item == *id);
        assert!(exists, "Expected user ID not found: {}", id);
    }

    Ok(())
}

#[tokio::test]
async fn test_global_influencers_with_this_month_timeframe() -> Result<()> {
    let body =
        get_request("/v0/stream/users?source=influencers&timeframe=this_month&limit=5").await?;

    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "r91hi8kc3x6761gwfiigr7yn6nca1z47wm6jadhw1jbx1co93r9y",
        "tkpeqpx3ywoawiw6q8e6kuo9o3egr7fnhx83rudznbrrmqgdmomo",
        "pyc598poqkdgtx1wc4aeptx67mqg71mmywyh7uzkffzittjmbiuo",
        "r4irb481b8qspaixq1brwre8o87cxybsbk9iwe1f6f9ukrxxs7bo",
        "qumq6fady4bmw4w5tpsrj1tg36g3qo4tcfedga9p4bg4so4ikyzy",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = influencer_ids.clone().into_iter().any(|item| item == *id);
        assert!(exists, "Expected user ID not found: {}", id);
    }

    Ok(())
}

#[tokio::test]
async fn test_influencers_by_reach_no_user_id() -> Result<()> {
    let endpoint =
        "/v0/stream/users?source=influencers&timeframe=this_month&limit=3&reach=following";

    invalid_get_request(endpoint, StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio::test]
async fn test_influencers_by_following_reach() -> Result<()> {
    let endpoint = &format!("/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={}&reach=following", PEER_PUBKY);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body
        .as_array()
        .expect("Stream influencers should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
        "9arfi37owcrdywc9zqw3m5uc7gd5gqu1yfuykzo66od6tcayqk9y",
    ];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio::test]
async fn test_influencers_by_followers_reach() -> Result<()> {
    let endpoint = &format!("/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={}&reach=followers", PEER_PUBKY);

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body.as_array().expect("Post stream should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y",
        "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy",
    ];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}

#[tokio::test]
async fn test_influencers_by_friends_reach() -> Result<()> {
    let endpoint = &format!(
        "/v0/stream/users?source=influencers&timeframe=this_month&limit=3&user_id={}&reach=friends",
        PEER_PUBKY
    );

    let body = get_request(endpoint).await?;
    assert!(body.is_array());

    let influencers = body.as_array().expect("Post stream should be an array");

    let influencer_ids = influencers
        .iter()
        .map(|f| f["details"]["id"].as_str().unwrap())
        .collect::<Vec<&str>>();

    // List of expected user IDs
    let expected_user_ids = vec![
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y",
        "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy",
    ];
    assert!(influencer_ids == expected_user_ids);

    Ok(())
}
