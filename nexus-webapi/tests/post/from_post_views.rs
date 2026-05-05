use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::models::post::PostView;
use nexus_webapi::models::PostStreamDetailed;

// Cairo user and post 00000039YD9DP from posts.cypher
const CAIRO_USER: &str = super::CAIRO_USER;
const POST_ID: &str = "00000039YD9DP";

// File metadata from files.cypher lines 7-8
const FILE_ID_1: &str = "2ZK3A1B2C3D40";
const FILE_ID_2: &str = "2ZK3E5F6G7H80";

#[tokio_shared_rt::test(shared)]
async fn test_from_post_views_with_attachment_metadata() -> Result<()> {
    TestServiceServer::get_test_server().await;

    let post_view = PostView::get_by_id(CAIRO_USER, POST_ID, None, None, None)
        .await
        .unwrap()
        .expect("Post 00000039YD9DP should exist in the test database");

    let result = PostStreamDetailed::from_post_views(vec![post_view], true)
        .await
        .unwrap();

    assert_eq!(result.0.len(), 1);

    let detailed = &result.0[0];
    assert_eq!(detailed.attachments_metadata.len(), 2);

    assert_eq!(detailed.attachments_metadata[0].id, FILE_ID_1);
    assert_eq!(detailed.attachments_metadata[0].owner_id, CAIRO_USER);
    assert_eq!(detailed.attachments_metadata[0].name, "cairo_file1");
    assert_eq!(detailed.attachments_metadata[0].content_type, "image/png");

    assert_eq!(detailed.attachments_metadata[1].id, FILE_ID_2);
    assert_eq!(detailed.attachments_metadata[1].owner_id, CAIRO_USER);
    assert_eq!(detailed.attachments_metadata[1].name, "cairo_file2");
    assert_eq!(detailed.attachments_metadata[1].content_type, "image/jpeg");

    Ok(())
}
