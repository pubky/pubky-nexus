use crate::{
    post::{CAIRO_USER, ENCRYPTION_TAG, ROOT_PATH},
    stream::post::{kind::DETROIT, POST_H, TAG_LABEL_2},
    utils::{get_request, invalid_get_request},
};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::models::tag::TagDetails;

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view() -> Result<()> {
    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let body = get_request(&format!(
        "/v0/post/{author_id}/{post_id}?viewer_id={author_id}"
    ))
    .await?;

    assert_eq!(body["details"]["content"], "I am told we can reply now!");
    assert_eq!(body["details"]["indexed_at"].as_u64(), Some(1718616844478));
    assert_eq!(body["details"]["id"], post_id);
    assert_eq!(body["details"]["author"], author_id);
    assert_eq!(body["details"]["attachments"].as_array().unwrap().len(), 1);
    assert_eq!(
        (body["details"]["attachments"].as_array().unwrap())[0],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/files/2ZKH7K7M9G3G0".to_string()
    );
    assert_eq!(
        body["details"]["uri"],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/posts/2ZCW1TGR5BKG0"
    );
    assert_eq!(body["counts"]["tags"].as_u64(), Some(5));
    assert_eq!(body["counts"]["unique_tags"].as_u64(), Some(1));
    assert_eq!(body["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(body["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(body["bookmark"]["indexed_at"].as_u64(), Some(1721764200000));
    assert_eq!(body["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = body["tags"][0].clone();
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

#[tokio_shared_rt::test(shared)]
async fn test_get_post_counts() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}/counts");

    let body = get_request(&path).await?;
    //let post_tag: PostView = serde_json::from_value(body.clone())?;
    assert!(body.is_object());

    // Check the post counts
    assert_eq!(body["tags"], 6);
    assert_eq!(body["unique_tags"], 2);
    assert_eq!(body["replies"], 0);
    assert_eq!(body["reposts"], 0);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1");

    let body = get_request(&path).await?;
    //let post_tag: PostView = serde_json::from_value(body.clone())?;
    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 4);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_taggers() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_taggers=2");

    let body = get_request(&path).await?;

    assert!(body.is_object());
    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let opensource_taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(opensource_taggers.len(), 2);

    assert_eq!(tags[1]["label"], ENCRYPTION_TAG);
    assert_eq!(tags[1]["taggers_count"], 2);
    let encryption_taggers = tags[1]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(encryption_taggers.len(), 2);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags_and_taggers() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1&limit_taggers=2");

    let body = get_request(&path).await?;

    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 2);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_attachment_metadata() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?include_attachment_metadata=true");

    let body = get_request(&path).await?;
    assert!(body.is_object());

    let attachments_metadata = body["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);

    // First attachment
    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[0]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[0]["name"], "cairo_file1");
    assert_eq!(attachments_metadata[0]["content_type"], "image/png");

    // Second attachment
    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");
    assert_eq!(attachments_metadata[1]["owner_id"], CAIRO_USER);
    assert_eq!(attachments_metadata[1]["name"], "cairo_file2");
    assert_eq!(attachments_metadata[1]["content_type"], "image/jpeg");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_limit_tags_and_taggers_and_attachment_metadata() -> Result<()> {
    let path = format!(
        "{ROOT_PATH}/{CAIRO_USER}/{POST_H}?limit_tags=1&limit_taggers=2&include_attachment_metadata=true"
    );

    let body = get_request(&path).await?;
    assert!(body.is_object());

    // Check tags with limits
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"]
        .as_array()
        .expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 2);

    // Check attachments metadata
    let attachments_metadata = body["attachments_metadata"]
        .as_array()
        .expect("Post attachments_metadata should be an array");
    assert_eq!(attachments_metadata.len(), 2);
    assert_eq!(attachments_metadata[0]["id"], "2ZK3A1B2C3D40");
    assert_eq!(attachments_metadata[1]["id"], "2ZK3E5F6G7H80");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_post_view_with_viewer() -> Result<()> {
    let path = format!("{ROOT_PATH}/{CAIRO_USER}/{POST_H}?viewer_id={DETROIT}");

    let body = get_request(&path).await?;
    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"]
        .as_array()
        .expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    assert!(
        tags[0]["relationship"].as_bool().unwrap(),
        "Expected to be part of the taggers"
    );
    assert!(
        !tags[1]["relationship"].as_bool().unwrap(),
        "Expected not to be part of the taggers"
    );

    Ok(())
}
