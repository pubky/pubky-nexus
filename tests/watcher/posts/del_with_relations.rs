use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser};
use pubky_nexus::models::post::{PostCounts, PostDetails, PostView};

#[tokio_shared_rt::test(shared)]
async fn test_delete_post_with_relationships() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a new user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("Test user for post deletion".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostDelete:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Create a post without any relationships
    let post = PubkyAppPost {
        content: "User's post to be deleted".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Create a tag
    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        label: "funny".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.put(&tag_url, tag).await?;

    // Delete the post using the event handler
    test.cleanup_post(&user_id, &post_id).await?;

    // Attempt to find post details; should exist, but content is [DELETED]
    let post_details_result = PostDetails::get_by_id(&user_id, &post_id)
        .await
        .unwrap()
        .expect("Post details still be found after deletion");
    assert_eq!(
        post_details_result.content,
        "[DELETED]".to_string(),
        "Post content should exactly be [DELETED] after deletion"
    );

    // Attempt to find post counts; should not exist
    let post_counts_result = PostCounts::get_by_id(&user_id, &post_id).await.unwrap();
    assert!(
        post_counts_result.is_some(),
        "Post counts should exist after deletion"
    );

    // Attempt to get post view; should not exist
    let post_view = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap();
    assert!(post_view.is_some(), "Post view should exist after deletion");

    Ok(())
}
