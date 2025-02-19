use crate::service::utils::{get_request, invalid_get_request};
use anyhow::Result;
use pubky_nexus::models::tag::TagDetails;
use reqwest::StatusCode;

#[tokio_shared_rt::test(shared)]
async fn test_get_post() -> Result<()> {
    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let res = get_request(&format!(
        "/v0/post/{}/{}?viewer_id={}",
        author_id, post_id, author_id
    ))
    .await?;

    assert_eq!(res["details"]["content"], "I am told we can reply now!");
    assert_eq!(res["details"]["indexed_at"].as_u64(), Some(1718616844478));
    assert_eq!(res["details"]["id"], post_id);
    assert_eq!(res["details"]["author"], author_id);
    assert_eq!(res["details"]["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(
        (res["details"]["attachments"].as_array().unwrap())[0],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/files/2ZKH7K7M9G3G0".to_string()
    );
    assert_eq!(
        res["details"]["uri"],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/posts/2ZCW1TGR5BKG0"
    );
    assert_eq!(res["counts"]["tags"].as_u64(), Some(5));
    assert_eq!(res["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(res["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(res["bookmark"]["indexed_at"].as_u64(), Some(1721764200000));
    assert_eq!(res["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = res["tags"][0].clone();
    let post_tag: TagDetails = serde_json::from_value(post_tag_object.clone())?;
    assert_eq!(post_tag.label, "pubky");

    // Test non-existing post
    invalid_get_request(
        &format!("/v0/post/{}/{}", author_id, "no_post"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}
