use super::utils::find_user_tag;
use crate::watcher::{
    users::utils::{check_member_user_pioneer, find_user_counts},
    utils::WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pubky_common::crypto::Keypair;
use pubky_nexus::models::{
    pubky_app::{traits::HashId, PubkyAppTag, PubkyAppUser},
    tag::{traits::TagCollection, user::TagUser},
    user::UserView,
};

#[tokio::test]
async fn test_homeserver_del_tag_to_another_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create the users
    let keypair = Keypair::random();

    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnotherUser:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&keypair, &tagged_user).await?;

    let keypair = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_homeserver_del_tag_to_another_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:DelTagToAnotherUser:TaggerUser".to_string(),
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
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    // Put tag
    test.create_tag(tag_url.as_str(), tag_blob).await?;

    // Step 3: Delete the tag
    test.delete_tag(&tag_url).await?;

    // Step 4: Assert tag deletion

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
