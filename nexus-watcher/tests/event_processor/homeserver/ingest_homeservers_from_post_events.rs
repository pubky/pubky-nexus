use super::utils::create_external_test_homeserver;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::homeserver::Homeserver;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::TimestampId, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
    PubkyAppUser, PubkyId,
};

#[tokio_shared_rt::test(shared)]
async fn test_reply_to_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver
    let parent_author_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Create parent post author
    let parent_author_kp = Keypair::random();
    let parent_author_id = parent_author_kp.public_key().to_z32();

    let parent_author_hs_id = PubkyId::try_from(&parent_author_hs_pk.to_z32()).unwrap();

    // Register the parent author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    test.register_user_in_hs(&parent_author_kp, &parent_author_hs_pk)
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
    let parent_post_absolute_uri =
        post_uri_builder(parent_author_id.clone(), parent_post_id.clone());

    // Create reply, written by a separate reply author, on the main test homeserver
    let reply_author = PubkyAppUser {
        bio: Some("test_reply_to_post_on_unknown_homeserver_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyHomeserverIngest:Reply:User".to_string(),
        status: None,
    };
    let reply_author_kp = Keypair::random();
    let _reply_author_id = test.create_user(&reply_author_kp, &reply_author).await?;

    let reply = PubkyAppPost {
        content: "Watcher:ReplyRepost:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_post_absolute_uri),
        embed: None,
        attachments: None,
    };
    let reply_id = test.create_post(&reply_author_kp, &reply).await?;

    // Check if new HS was ingested
    let root_author_hs = Homeserver::get_by_id(parent_author_hs_id).await.unwrap();
    assert!(root_author_hs.is_some());

    // Cleanup
    test.cleanup_user(&reply_author_kp).await?;
    test.cleanup_post(&reply_author_kp, &reply_id).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_repost_of_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver
    let original_author_hs_pk = create_external_test_homeserver(&mut test).await?;
    let original_author_hs_id = PubkyId::try_from(&original_author_hs_pk.to_z32()).unwrap();

    // Create original post author
    let original_author_kp = Keypair::random();
    let original_author_id = original_author_kp.public_key().to_z32();

    // Register the original author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    test.register_user_in_hs(&original_author_kp, &original_author_hs_pk)
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
    let _repost_author_id = test.create_user(&repost_author_kp, &repost_author).await?;

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

    let repost_id = test.create_post(&repost_author_kp, &repost).await?;

    // Check if new HS was ingested
    let original_author_hs = Homeserver::get_by_id(original_author_hs_id).await.unwrap();
    assert!(original_author_hs.is_some());

    // Cleanup
    test.cleanup_user(&repost_author_kp).await?;
    test.cleanup_post(&repost_author_kp, &repost_id).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_and_mention_users_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create three separate homeservers for three mentioned users, each on one HS
    let user_1_hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_1_hs_id = PubkyId::try_from(&user_1_hs_pk.to_z32()).unwrap();
    let user_2_hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_2_hs_id = PubkyId::try_from(&user_2_hs_pk.to_z32()).unwrap();
    let user_3_hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_3_hs_id = PubkyId::try_from(&user_3_hs_pk.to_z32()).unwrap();

    // Create three users, which will be later mentioned in the test post
    let user_1_kp = Keypair::random();
    let user_1_id = user_1_kp.public_key().to_z32();
    let user_2_kp = Keypair::random();
    let user_2_id = user_2_kp.public_key().to_z32();
    let user_3_kp = Keypair::random();
    let user_3_id = user_3_kp.public_key().to_z32();

    // Register each new user in their respective homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    test.register_user_in_hs(&user_1_kp, &user_1_hs_pk).await?;
    test.register_user_in_hs(&user_2_kp, &user_2_hs_pk).await?;
    test.register_user_in_hs(&user_3_kp, &user_3_hs_pk).await?;

    // Create the test post on the main test homeserver, created by a known user (author)
    let post_author = PubkyAppUser {
        bio: Some("test_post_and_mention_users_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:MentionHomeserverIngest:User".to_string(),
        status: None,
    };
    let post_author_kp = Keypair::random();
    let _post_author_id = test.create_user(&post_author_kp, &post_author).await?;

    let post = PubkyAppPost {
        // The post content references the PKs of the external users
        content: format!("Hey pk:{user_1_id}, pk:{user_2_id} and pk:{user_3_id}!"),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&post_author_kp, &post).await?;

    // Check if the new homeserver of the first unknown mentioned user was ingested ...
    assert!(Homeserver::get_by_id(user_1_hs_id).await.unwrap().is_some());
    // ... and the the HS of the other mentioned users are not ingested
    assert!(Homeserver::get_by_id(user_2_hs_id).await.unwrap().is_none());
    assert!(Homeserver::get_by_id(user_3_hs_id).await.unwrap().is_none());

    // Cleanup
    test.cleanup_user(&post_author_kp).await?;
    test.cleanup_post(&post_author_kp, &post_id).await?;

    Ok(())
}
