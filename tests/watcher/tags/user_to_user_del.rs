use super::utils::find_user_tag;
use crate::watcher::{
    users::utils::{check_member_user_pioneer, find_user_counts},
    utils::watcher::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};
use pubky_nexus::models::tag::{traits::TagCollection, user::TagUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_del_tag_to_another_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create the users
    let tagged_keypair = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnother:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_keypair, &tagged_user).await?;

    let tagger_keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnother:TaggerUser".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger_user).await?;

    // Step 2: Add a tag to the user
    let label = "neo4j";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", tagged_user_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    // Put tag
    test.put(tag_url.as_str(), tag).await?;

    // Step 3: Delete the tag
    test.del(&tag_url).await?;

    // Step 4: Assert tag deletion
    // GRAPH_OP: Check if the tag node was deleted
    let user_tag = find_user_tag(&tagged_user_id, label).await.unwrap();
    assert!(user_tag.is_none());

    // CACHE_OP: Check if the tag is correctly updated in the cache
    let cache_user_tag =
        TagUser::get_from_index(&tagged_user_id, None, None, None, None, None, false)
            .await
            .expect("Failed to get tag from cache");

    assert!(
        cache_user_tag.is_none(),
        "The SORTED SET index cannot exist for the tag"
    );

    // Check if user counts is updated, User:Counts:user_id
    let user_counts = find_user_counts(&tagger_user_id).await;
    assert_eq!(user_counts.tagged, 0);

    let user_counts = find_user_counts(&tagged_user_id).await;
    assert_eq!(user_counts.tags, 0);
    assert_eq!(user_counts.unique_tags, 0);

    // Check user pionner score: Sorted:Users:Pioneers
    let pioneer_score = check_member_user_pioneer(&tagger_user_id)
        .await
        .expect("Failed to check user pioneer score");

    assert!(pioneer_score.is_some(), "Pioneer score should be present");
    assert_eq!(pioneer_score.unwrap(), 0);

    // Cleanup user
    test.cleanup_user(&tagged_user_id).await?;
    test.cleanup_user(&tagger_user_id).await?;

    Ok(())
}
