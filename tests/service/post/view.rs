use crate::service::{post::{CAIRO_USER, ENCRYPTION_TAG, ROOT_PATH}, stream::post::{POST_H, TAG_LABEL_2}, utils::{make_request, HOST_URL}};
use anyhow::Result;
use pubky_nexus::models::tag::TagDetails;

#[tokio::test]
async fn test_get_post_view() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let res = client
        .do_get(&format!(
            "/v0/post/{}/{}?viewer_id={}",
            author_id, post_id, author_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;

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
    assert_eq!(body["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(body["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(body["bookmark"]["indexed_at"].as_u64(), Some(1721764200000));
    assert_eq!(body["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = body["tags"][0].clone();
    let post_tag: TagDetails = serde_json::from_value(post_tag_object.clone())?;
    assert_eq!(post_tag.label, "pubky");

    // Test non-existing post
    let res = client
        .do_get(&format!("/v0/post/{}/{}", author_id, "no_post"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}



#[tokio::test]
async fn test_get_post_view_with_limit_tags() -> Result<()> {

    let path = format!("{}/{}/{}?limit_tags=1", ROOT_PATH,CAIRO_USER, POST_H);

    let body = make_request(&path).await?;
    //let post_tag: PostView = serde_json::from_value(body.clone())?;
    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"].as_array().expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"].as_array().expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 4);

    Ok(())
}

#[tokio::test]
async fn test_get_post_view_with_limit_taggers() -> Result<()> {

    let path = format!("{}/{}/{}?limit_taggers=2", ROOT_PATH,CAIRO_USER, POST_H);

    let body = make_request(&path).await?;

    assert!(body.is_object());
    // Check the tag list
    let tags = body["tags"].as_array().expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 2);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let opensource_taggers = tags[0]["taggers"].as_array().expect("Tag taggers should be an array");
    assert_eq!(opensource_taggers.len(), 2);


    assert_eq!(tags[1]["label"], ENCRYPTION_TAG);
    assert_eq!(tags[1]["taggers_count"], 2);
    let encryption_taggers = tags[1]["taggers"].as_array().expect("Tag taggers should be an array");
    assert_eq!(encryption_taggers.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_get_post_view_with_limit_tags_and_taggers() -> Result<()> {

    let path = format!("{}/{}/{}?limit_tags=1&limit_taggers=2", ROOT_PATH,CAIRO_USER, POST_H);

    let body = make_request(&path).await?;

    assert!(body.is_object());

    // Check the tag list
    let tags = body["tags"].as_array().expect("Post tags should be an array");
    // Check the total posts using that tag
    assert_eq!(tags.len(), 1);

    // opersource tag
    assert_eq!(tags[0]["label"], TAG_LABEL_2);
    assert_eq!(tags[0]["taggers_count"], 4);
    let taggers = tags[0]["taggers"].as_array().expect("Tag taggers should be an array");
    assert_eq!(taggers.len(), 2);

    Ok(())
}

