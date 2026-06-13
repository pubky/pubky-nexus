use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::user::UserDetails;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::TimestampId, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
    PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_reply_to_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let parent_author_hs_pk = create_external_test_homeserver(&mut test).await?;

    let parent_author_kp = Keypair::random();
    let parent_author_id = parent_author_kp.public_key().to_z32();

    test.register_user_in_hs(&parent_author_kp, &parent_author_hs_pk)
        .await?;

    let parent_post = PubkyAppPost {
        content: "Watcher:ReplyUserIngest:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let parent_post_id = parent_post.create_id();
    let parent_post_absolute_uri =
        post_uri_builder(parent_author_id.clone(), parent_post_id.clone());

    let reply_author = PubkyAppUser {
        bio: Some("test_reply_to_post_on_unknown_homeserver_reply".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyUserIngest:Reply:User".to_string(),
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
    let (_reply_id, reply_path) = test.create_post(&reply_author_kp, &reply).await?;

    assert_user_ingested(&parent_author_id, &parent_author_hs_pk).await;

    test.cleanup_user(&reply_author_kp).await?;
    test.cleanup_post(&reply_author_kp, &reply_path).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_repost_of_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let original_author_hs_pk = create_external_test_homeserver(&mut test).await?;

    let original_author_kp = Keypair::random();
    let original_author_id = original_author_kp.public_key().to_z32();

    test.register_user_in_hs(&original_author_kp, &original_author_hs_pk)
        .await?;

    let original_post = PubkyAppPost {
        content: "Watcher:RepostUserIngest:Original:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let original_post_id = original_post.create_id();
    let original_post_uri = post_uri_builder(original_author_id.clone(), original_post_id.clone());

    let repost_author = PubkyAppUser {
        bio: Some("test_repost_of_post_on_unknown_homeserver_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:RepostUserIngest:Repost:User".to_string(),
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

    let (_repost_id, repost_path) = test.create_post(&repost_author_kp, &repost).await?;

    assert_user_ingested(&original_author_id, &original_author_hs_pk).await;

    test.cleanup_user(&repost_author_kp).await?;
    test.cleanup_post(&repost_author_kp, &repost_path).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_post_and_mention_users_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_1_hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_2_hs_pk = create_external_test_homeserver(&mut test).await?;
    let user_3_hs_pk = create_external_test_homeserver(&mut test).await?;

    let user_1_kp = Keypair::random();
    let user_1_id = user_1_kp.public_key().to_z32();
    let user_2_kp = Keypair::random();
    let user_2_id = user_2_kp.public_key().to_z32();
    let user_3_kp = Keypair::random();
    let user_3_id = user_3_kp.public_key().to_z32();

    test.register_user_in_hs(&user_1_kp, &user_1_hs_pk).await?;
    test.register_user_in_hs(&user_2_kp, &user_2_hs_pk).await?;
    test.register_user_in_hs(&user_3_kp, &user_3_hs_pk).await?;

    let post_author = PubkyAppUser {
        bio: Some("test_post_and_mention_users_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:MentionUserIngest:User".to_string(),
        status: None,
    };
    let post_author_kp = Keypair::random();
    let _post_author_id = test.create_user(&post_author_kp, &post_author).await?;

    let post = PubkyAppPost {
        content: format!("Hey pubky{user_1_id}, pubky{user_2_id} and pubky{user_3_id}!"),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (_post_id, post_path) = test.create_post(&post_author_kp, &post).await?;

    // The first unknown mentioned user should have been ingested ...
    assert_user_ingested(&user_1_id, &user_1_hs_pk).await;
    // ... but the others are not ingested (only first mention triggers ingestion)
    assert!(UserDetails::get_by_id(&user_2_id).await.unwrap().is_none());
    assert!(UserDetails::get_by_id(&user_3_id).await.unwrap().is_none());

    test.cleanup_user(&post_author_kp).await?;
    test.cleanup_post(&post_author_kp, &post_path).await?;

    Ok(())
}
