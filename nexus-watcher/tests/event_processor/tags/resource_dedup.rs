use super::resource_utils::{compute_resource_id, count_resource_tags};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::traits::TagCollection;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// Two different users tag the same external URI with the same label
/// from different app paths. Verifies:
/// - One Resource node exists
/// - Two TAGGED relationships (one per user)
/// - Correct aggregated tagger count
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_multi_user_dedup() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let target_uri = "https://example.com/dedup-test";
    let label = "dedup-label";
    let resource_id = compute_resource_id(target_uri);

    // User 1 tags from /pub/mapky/tags/
    let user1_kp = Keypair::random();
    let user1 = PubkyAppUser {
        bio: Some("dedup_user_1".to_string()),
        image: None,
        links: None,
        name: "Watcher:Dedup:User1".to_string(),
        status: None,
    };
    let _user1_id = test.create_user(&user1_kp, &user1).await?;

    let tag1 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag1_id = tag1.create_id();
    let path1: ResourcePath = format!("/pub/mapky/tags/{tag1_id}").parse()?;
    test.put(&user1_kp, &path1, &tag1).await?;

    // User 2 tags same URI, same label from /pub/eventky/tags/
    let user2_kp = Keypair::random();
    let user2 = PubkyAppUser {
        bio: Some("dedup_user_2".to_string()),
        image: None,
        links: None,
        name: "Watcher:Dedup:User2".to_string(),
        status: None,
    };
    let _user2_id = test.create_user(&user2_kp, &user2).await?;

    let tag2 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag2_id = tag2.create_id();
    let path2: ResourcePath = format!("/pub/eventky/tags/{tag2_id}").parse()?;
    test.put(&user2_kp, &path2, &tag2).await?;

    // Verify: Two TAGGED relationships on one Resource
    let tag_count = count_resource_tags(&resource_id).await?;
    assert_eq!(
        tag_count, 2,
        "Should have 2 TAGGED relationships from 2 users"
    );

    // Verify: Tag aggregation shows 2 taggers
    let cache_tags =
        TagResource::get_from_index(&resource_id, None, None, None, None, None, false).await?;

    assert!(cache_tags.is_some());
    let details = cache_tags.unwrap();
    assert_eq!(details.len(), 1, "Should have 1 label");
    assert_eq!(details[0].label, label);
    assert_eq!(details[0].taggers_count, 2, "Should show 2 taggers");

    // Cleanup: del both tags, then both users
    test.del(&user1_kp, &path1).await?;
    test.del(&user2_kp, &path2).await?;
    test.cleanup_user(&user1_kp).await?;
    test.cleanup_user(&user2_kp).await?;

    Ok(())
}
