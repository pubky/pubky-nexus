use anyhow::Result;
use pubky_nexus::models::tag::TagDetails;

const HOST_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn test_get_post() -> Result<()> {
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
