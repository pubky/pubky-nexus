use super::utils::find_user_tag;
use crate::event_processor::{
    users::utils::{check_member_user_influencer, find_user_counts},
    utils::watcher::{HomeserverHashIdPath, WatcherTest},
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::event::Event;
use nexus_common::models::tag::{traits::TagCollection, user::TagUser};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_del_tag_to_another_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create the users
    let tagged_kp = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnother:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    let tagger_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnother:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    // Step 2: Add a tag to the user
    let label = "neo4j";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_path = tag.hs_path();
    let (_, events_in_redis_before) = Event::get_events_from_redis(None, 1000).await.unwrap();

    // Put tag
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Step 3: Delete the tag
    test.del(&tagger_kp, &tag_path).await?;

    // Step 4: Assert tag deletion
    // GRAPH_OP: Check if the tag node was deleted
    let user_tag = find_user_tag(&tagged_user_id, label).await.unwrap();
    assert!(user_tag.is_none());

    // CACHE_OP: Check if the tag is correctly updated in the cache
    let (_, events_in_redis_after) = Event::get_events_from_redis(None, 1000).await.unwrap();
    assert!(events_in_redis_after > events_in_redis_before);
    let cache_user_tag =
        TagUser::get_from_index(&tagged_user_id, None, None, None, None, None, false)
            .await
            .expect("Failed to get tag from cache");

    assert!(
        cache_user_tag.unwrap().is_empty(),
        "The SORTED SET index cannot exist for the tag"
    );

    // Check if user counts is updated, User:Counts:user_id
    let user_counts = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts.tagged, 0);

    let user_counts = find_user_counts(&tagged_user_id).await;
    assert_eq!(user_counts.tags, 0);
    assert_eq!(user_counts.unique_tags, 0);

    // Check user pionner score: Sorted:Users:Influencers
    let influencer_score = check_member_user_influencer(&tagger_user_id)
        .await
        .expect("Failed to check user influencer score");

    assert!(
        influencer_score.is_some(),
        "Influencer score should be present"
    );
    assert_eq!(influencer_score.unwrap(), 0);

    // Cleanup user
    test.cleanup_user(&tagged_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}
