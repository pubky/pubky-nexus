use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::models::post::PostView;
use pubky_nexus::models::pubky_app::{PubkyAppPost, PubkyAppTag, PubkyAppUser};

#[tokio::test]
async fn test_homeserver_tag_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("This is a test user for tagging posts".to_string()),
        image: None,
        links: None,
        name: "Test User: PostTags".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Create a post under that user
    let post = PubkyAppPost {
        content: "This is a tag test post!".to_string(),
        kind: PubkyAppPost::default().kind,
        embed: None,
    };
    let post_id = test.create_post(&user_id, &post).await?;

    // Step 3: Add a tag to the post
    let label = "cool";
    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.client.put(tag_url.as_str(), &tag_blob).await?;
    test.ensure_event_processing_complete().await?;

    // Step 4: Verify the tag exists in Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The tag should have been created");


    println!("User_id: {:?}, Post_id: {:?}, label {:?}", user_id, post_id, label);

    // Check if the tag is in the new post and also if the user is the tagger
    assert_eq!(result_post.tags[0].taggers_count, 1);
    assert_eq!(result_post.tags[0].taggers[0], user_id);
    assert_eq!(result_post.tags[0].label, label);

    // Step 5: Delete the tag
    test.client.delete(tag_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Step 6: Verify the tag has been deleted
    let _result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .unwrap();

    // TODO: uncomment tests when fixed redis indexing
    // assert_eq!(
    //     result_post.tags[0].taggers_count, 0,
    //     "The tag should have been deleted"
    // );

    // Cleanup user and post
    test.cleanup_post(&user_id, &post_id).await?;
    test.cleanup_user(&user_id).await?;

    Ok(())

}
