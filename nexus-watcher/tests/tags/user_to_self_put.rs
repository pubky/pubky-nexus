use super::utils::find_user_tag;
use crate::{
    users::utils::{check_member_user_influencer, find_user_counts},
    utils::watcher::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::tag::{traits::TagCollection, user::TagUser};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_tag_user_self() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create a user
    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_user_self".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagSelf:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;

    // Step 2: Add a tag to the user
    let label = "friendly";

    let tag = PubkyAppTag {
        uri: format!("pubky://{user_id}/pub/pubky.app/profile.json"), // Tagging himself, so tag uri is his own profile uri
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

    // Put tag
    test.put(tag_url.as_str(), tag).await?;

    // Step 3: Verify tag existence and data consistency

    // GRAPH_OP: Check if the tag exists in the graph database
    let user_tag = find_user_tag(&user_id, label)
        .await
        .unwrap()
        .expect("Failed to find user tag in graph database");
    assert_eq!(user_tag.label, label);
    assert_eq!(user_tag.taggers_count, 1);
    assert_eq!(user_tag.taggers[0], user_id);

    // CACHE_OP: Check if the tag is correctly cached
    let cache_user_tag = TagUser::get_from_index(&user_id, None, None, None, None, None, false)
        .await
        .expect("Failed to get tag from cache");

    assert!(cache_user_tag.is_some(), "Tag should exist in cache");
    let cache_tag_details = cache_user_tag.unwrap();
    assert_eq!(cache_tag_details.len(), 1);

    // TagUser related
    assert_eq!(cache_tag_details[0].label, label);
    // Count user profile taggers: Sorted:Users:Tag:user_id:{label}
    assert_eq!(cache_tag_details[0].taggers_count, 1);
    // Find user as tagger in the user profile: User:Taggers:user_id
    assert_eq!(cache_tag_details[0].taggers[0], user_id);

    // Check if user counts updated: User:Counts:user_id
    let user_counts = find_user_counts(&user_id).await;
    assert_eq!(user_counts.tagged, 1);
    assert_eq!(user_counts.tags, 1);
    assert_eq!(user_counts.unique_tags, 1);

    // Check user pionner score: Sorted:Users:Influencers
    let influencer_score = check_member_user_influencer(&user_id)
        .await
        .expect("Failed to check user influencer score");
    assert!(
        influencer_score.is_some(),
        "Influencer score should be present"
    );
    assert_eq!(influencer_score.unwrap(), 0);

    // Cleanup user
    test.cleanup_user(&user_id).await?;

    Ok(())
}
