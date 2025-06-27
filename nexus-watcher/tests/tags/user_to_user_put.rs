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
async fn test_homeserver_put_tag_user_another() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create the users
    let tagged_keypair = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_user_another".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagAnother:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_keypair, &tagged_user).await?;

    let tagger_keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_put_tag_user_another".to_string()),
        image: None,
        links: None,
        name: "Watcher:PutTagAnother:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Step 2: Add a tag to the user
    let label = "dev";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    // PUT post tag
    test.put(tag_url.as_str(), tag).await?;

    // Step 3: Verify tag existence and data consistency

    // GRAPH_OP: Check if the tag exists in the graph database
    let user_tag = find_user_tag(&tagged_user_id, label)
        .await
        .unwrap()
        .expect("Failed to find user tag in graph database");
    assert_eq!(user_tag.label, label);
    assert_eq!(user_tag.taggers_count, 1);
    assert_eq!(user_tag.taggers[0], tagger_user_id);

    // CACHE_OP: Check if the tag is correctly cached
    let cache_user_tag =
        TagUser::get_from_index(&tagged_user_id, None, None, None, None, None, false)
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
    assert_eq!(cache_tag_details[0].taggers[0], tagger_user_id);

    // Check if user counts of the tagged updated: User:Counts:user_id
    let user_counts = find_user_counts(&tagged_user_id).await;
    assert_eq!(user_counts.tags, 1);
    assert_eq!(user_counts.unique_tags, 1);

    // Check if user counts of the tagger updated: User:Counts:user_id
    let user_counts = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts.tagged, 1);

    // Check user pionner score: Sorted:Users:Influencers
    let influencer_score = check_member_user_influencer(&tagged_user_id)
        .await
        .expect("Failed to check user influencer score");
    assert!(
        influencer_score.is_some(),
        "Influencer score should be present"
    );
    assert_eq!(influencer_score.unwrap(), 0);

    // Cleanup user
    test.cleanup_user(&tagged_user_id).await?;
    test.cleanup_user(&tagger_user_id).await?;

    Ok(())
}
