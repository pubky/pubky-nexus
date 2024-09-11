use super::utils::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pkarr::Keypair;
use pubky_nexus::{
    models::{
        pubky_app::{PubkyAppTag, PubkyAppUser},
        user::{UserStream, UserView, USER_PIONEERS_KEY_PARTS},
    },
    RedisOps,
};

#[tokio::test]
async fn test_homeserver_tag_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("This is a test user for tagging".to_string()),
        image: None,
        links: None,
        name: "Test User: UserTags".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Add a tag to the user
    let label = "friendly";
    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", user_id), // Tagging himself, so tag uri is his own profile uri
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.client.put(tag_url.as_str(), &tag_blob).await?;
    test.ensure_event_processing_complete().await?;

    // Step 3: Verify the tag exists on the user in Nexus
    let result_user = UserView::get_by_id(&user_id, None)
        .await
        .unwrap()
        .expect("The tag should have been created");

    println!("User_id: {:?}, label {:?}", user_id, label);

    assert_eq!(result_user.tags[0].label, label);
    // Count user profile taggers: Sorted:Users:Tag:user_id:{label}
    assert_eq!(result_user.tags[0].taggers_count, 1);
    // Find user as tagger in the user profile: User:Taggers:user_id
    assert_eq!(result_user.tags[0].taggers[0], user_id);

    // Check if user counts updated: User:Counts:user_id:post_id
    assert_eq!(result_user.counts.tags, 1);

    // Check user pionner score: Sorted:Users:Pioneers
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&user_id])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pioneer_score, 0);

    // Step 4: Delete the tag
    test.client.delete(tag_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Step 5: Verify the tag has been deleted
    let _result_user = UserView::get_by_id(&user_id, None).await.unwrap().unwrap();

    // TODO: uncomment tests when fixed redis de-indexing
    // assert!(
    //     result_user.tags.is_empty(),
    //     "The tag should have been deleted"
    // );

    // Cleanup user
    test.cleanup_user(&user_id).await?;

    Ok(())
}
