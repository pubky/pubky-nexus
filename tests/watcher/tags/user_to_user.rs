use super::utils::find_user_tag;
use crate::watcher::{
    users::utils::{check_member_user_pioneer, find_user_counts},
    utils::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    pubky_app::{traits::GenerateHashId, PubkyAppTag, PubkyAppUser},
    tag::{traits::TagCollection, user::TagUser},
    user::UserView,
};

#[tokio::test]
async fn test_homeserver_tag_to_another_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create the users
    let keypair = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagToAnotherUser:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&keypair, &tagged_user).await?;

    let keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:TagToAnotherUser:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&keypair, &tagger_user).await?;

    // Step 2: Add a tag to the user
    let label = "dev";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", tagged_user_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", tagger_user_id, tag.create_id());

    // Put tag
    test.create_tag(tag_url.as_str(), tag_blob).await?;

    // GRAPH_OP
    let post_tag = find_user_tag(&tagged_user_id, label).await.unwrap();
    assert_eq!(post_tag.label, label);
    assert_eq!(post_tag.taggers_count, 1);
    assert_eq!(post_tag.taggers[0], tagger_user_id);

    // CACHE_OP
    let cache_user_tag = TagUser::get_from_index(&tagged_user_id, None, None, None)
        .await
        .unwrap();

    assert_eq!(cache_user_tag.is_some(), true);
    let cache_tag_details = cache_user_tag.unwrap();
    assert_eq!(cache_tag_details.len(), 1);

    // TagUser related
    assert_eq!(cache_tag_details[0].label, label);
    // Count user profile taggers: Sorted:Users:Tag:user_id:{label}
    assert_eq!(cache_tag_details[0].taggers_count, 1);
    // Find user as tagger in the user profile: User:Taggers:user_id
    assert_eq!(cache_tag_details[0].taggers[0], tagger_user_id);

    // Check if user counts updated: User:Counts:user_id
    let user_counts = find_user_counts(&tagged_user_id).await;
    assert_eq!(user_counts.tags, 1);

    // Check user pionner score: Sorted:Users:Pioneers
    let pioneer_score = check_member_user_pioneer(&tagged_user_id).await.unwrap();
    assert_eq!(pioneer_score.is_some(), true);
    assert_eq!(pioneer_score.unwrap(), 0);

    // Step 4: Delete the tag
    test.client.delete(tag_url.as_str()).await?;
    test.ensure_event_processing_complete().await?;

    // Step 5: Verify the tag has been deleted
    let _result_user = UserView::get_by_id(&tagged_user_id, None).await.unwrap().unwrap();

    // TODO: uncomment tests when fixed redis de-indexing
    // assert!(
    //     result_user.tags.is_empty(),
    //     "The tag should have been deleted"
    // );

    // Cleanup user
    test.cleanup_user(&tagged_user_id).await?;
    test.cleanup_user(&tagger_user_id).await?;

    Ok(())
}