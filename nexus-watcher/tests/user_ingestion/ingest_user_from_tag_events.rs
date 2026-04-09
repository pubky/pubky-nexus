use super::utils::{assert_user_ingested, create_external_test_homeserver};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::TimestampId, user_uri_builder, PubkyAppPost, PubkyAppPostKind,
    PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_tag_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagged_post_hs_pk = create_external_test_homeserver(&mut test).await?;

    let tagged_post_author_kp = Keypair::random();
    let tagged_post_author_id = tagged_post_author_kp.public_key().to_z32();

    test.register_user_in_hs(&tagged_post_author_kp, &tagged_post_hs_pk)
        .await?;

    let post = PubkyAppPost {
        content: "Watcher:UserIngestion:Tagged:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = post.create_id();
    let post_uri = post_uri_builder(tagged_post_author_id.clone(), post_id.clone());

    let tagger_author_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_tag_post_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:Tagger:User".to_string(),
        status: None,
    };
    test.create_user(&tagger_author_kp, &tagger_user).await?;

    let tag = PubkyAppTag {
        uri: post_uri.clone(),
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_author_kp, &tag_path, tag).await?;

    assert_user_ingested(&tagged_post_author_id, &tagged_post_hs_pk).await;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tag_user_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagged_user_hs_pk = create_external_test_homeserver(&mut test).await?;

    let tagged_user_author_kp = Keypair::random();
    let tagged_user_author_id = tagged_user_author_kp.public_key().to_z32();

    test.register_user_in_hs(&tagged_user_author_kp, &tagged_user_hs_pk)
        .await?;

    let tagger_author_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_tag_user_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:UserIngestion:Tagger:User".to_string(),
        status: None,
    };
    test.create_user(&tagger_author_kp, &tagger_user).await?;

    let tagged_user_uri = user_uri_builder(tagged_user_author_id.clone());

    let tag = PubkyAppTag {
        uri: tagged_user_uri.clone(),
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_author_kp, &tag_path, tag).await?;

    assert_user_ingested(&tagged_user_author_id, &tagged_user_hs_pk).await;

    Ok(())
}
