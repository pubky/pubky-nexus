use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{db::PubkyClient, models::homeserver::Homeserver};
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::TimestampId, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
    PubkyAppUser, PubkyId,
};
use pubky_testnet::pubky_homeserver::{ConfigToml, MockDataDir};

async fn create_new_test_homeserver(test: &mut WatcherTest) -> Result<Keypair> {
    let mock_dir = MockDataDir::new(ConfigToml::test(), None)?;
    let new_hs_keypair = mock_dir.keypair.clone();
    test.testnet
        .testnet
        .create_homeserver_suite_with_mock(mock_dir)
        .await?;

    Ok(new_hs_keypair)
}

#[tokio_shared_rt::test(shared)]
async fn test_reply_to_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver with a new keypair
    let parent_author_hs_kp = create_new_test_homeserver(&mut test).await?;

    // Create parent post author
    let parent_author_kp = Keypair::random();
    let parent_author_id = parent_author_kp.public_key().to_z32();

    let parent_author_hs_pk = parent_author_hs_kp.public_key();
    let parent_author_hs_id = PubkyId::try_from(&parent_author_hs_pk.to_z32()).unwrap();

    // Register the parent author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    PubkyClient::get()?
        .signup(&parent_author_kp, &parent_author_hs_pk, None)
        .await?;

    // Create parent Post
    // We only need its ID, not necessarily to upload it on the new HS
    let parent_post = PubkyAppPost {
        content: "Watcher:ReplyHomeserverIngest:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let parent_post_id = parent_post.create_id();
    let parent_post_uri = post_uri_builder(parent_author_id.clone(), parent_post_id.clone());

    // Create reply, written by a separate reply author, on the main test homeserver
    let reply_author = PubkyAppUser {
        bio: Some("test_reply_to_post_on_unknown_homeserver_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyHomeserverIngest:Reply:User".to_string(),
        status: None,
    };
    let reply_author_kp = Keypair::random();
    let reply_author_id = test.create_user(&reply_author_kp, &reply_author).await?;

    let reply = PubkyAppPost {
        content: "Watcher:ReplyRepost:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_post_uri),
        embed: None,
        attachments: None,
    };
    let reply_id = test.create_post(&reply_author_id, &reply).await?;

    // Check if new HS was ingested
    let root_author_hs = Homeserver::get_by_id(parent_author_hs_id).await.unwrap();
    assert!(root_author_hs.is_some());

    // Cleanup
    test.cleanup_user(&reply_author_id).await?;
    test.cleanup_post(&reply_author_id, &reply_id).await?;
    test.cleanup_post(&reply_author_id, &parent_post_id).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_repost_of_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver with a new keypair
    let original_author_hs_kp = create_new_test_homeserver(&mut test).await?;

    // Create original post author
    let original_author_kp = Keypair::random();
    let original_author_id = original_author_kp.public_key().to_z32();

    let original_author_hs_pk = original_author_hs_kp.public_key();
    let original_author_hs_id = PubkyId::try_from(&original_author_hs_pk.to_z32()).unwrap();

    // Register the original author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    PubkyClient::get()?
        .signup(&original_author_kp, &original_author_hs_pk, None)
        .await?;

    // Create original Post
    // We only need its ID, not necessarily to upload it on the new HS
    let original_post = PubkyAppPost {
        content: "Watcher:RepostHomeserverIngest:Original:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let original_post_id = original_post.create_id();
    let original_post_uri = post_uri_builder(original_author_id.clone(), original_post_id.clone());

    // Create repost, written by a separate repost author, on the main test homeserver
    let repost_author = PubkyAppUser {
        bio: Some("test_repost_of_post_on_unknown_homeserver_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostHomeserverIngest:Repost:User".to_string(),
        status: None,
    };
    let repost_author_kp = Keypair::random();
    let repost_author_id = test.create_user(&repost_author_kp, &repost_author).await?;

    let repost = PubkyAppPost {
        content: "Watcher:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: original_post_uri,
        }),
        attachments: None,
    };

    let repost_id = test.create_post(&repost_author_id, &repost).await?;

    // Check if new HS was ingested
    let original_author_hs = Homeserver::get_by_id(original_author_hs_id).await.unwrap();
    assert!(original_author_hs.is_some());

    // Cleanup
    test.cleanup_user(&repost_author_id).await?;
    test.cleanup_post(&repost_author_id, &repost_id).await?;
    test.cleanup_post(&repost_author_id, &original_post_id)
        .await?;

    Ok(())
}
