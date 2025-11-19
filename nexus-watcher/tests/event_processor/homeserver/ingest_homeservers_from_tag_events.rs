use crate::event_processor::{
    homeserver::utils::create_external_test_homeserver,
    utils::watcher::{HomeserverHashIdPath, WatcherTest},
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::homeserver::Homeserver;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, traits::TimestampId, user_uri_builder, PubkyAppPost, PubkyAppPostKind,
    PubkyAppTag, PubkyAppUser, PubkyId,
};

#[tokio_shared_rt::test(shared)]
async fn test_tag_post_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver for the tagged post
    let tagged_post_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Create tagged post author
    let tagged_post_author_kp = Keypair::random();
    let tagged_post_author_id = tagged_post_author_kp.public_key().to_z32();

    // Register the tagged post author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    test.register_user_in_hs(&tagged_post_author_kp, &tagged_post_hs_pk)
        .await?;

    // Create tagged post
    let post = PubkyAppPost {
        content: "Watcher:Homeserver:Tagged:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    // We cannot PUT that event because the tagged user is not signed up (missing profile.json, missing graph node)
    // That one will force in the post event handler to ingest the homeserver of the tagged user
    // because it will throw a MissingDependency error
    let post_id = post.create_id();
    let post_uri = post_uri_builder(tagged_post_author_id.clone(), post_id.clone());

    // Create tagger user
    let tagger_author_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_tag_post_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:Homeserver:Tagger:User".to_string(),
        status: None,
    };
    test.create_user(&tagger_author_kp, &tagger_user).await?;

    // Add a tag to the post
    let tag = PubkyAppTag {
        uri: post_uri.clone(),
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    // PUT tag
    let tag_path = tag.hs_path();
    test.put(&tagger_author_kp, &tag_path, tag).await?;

    // Check if the new homeserver of the unknown tagged user was ingested
    let tagged_post_hs_id = PubkyId::try_from(&tagged_post_hs_pk.to_z32()).unwrap();
    assert!(Homeserver::get_by_id(tagged_post_hs_id)
        .await
        .unwrap()
        .is_some());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_tag_user_on_unknown_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Create a separate homeserver for the tagged post
    let tagged_user_hs_pk = create_external_test_homeserver(&mut test).await?;

    // Create tagged post author
    let tagged_user_author_kp = Keypair::random();
    let tagged_user_author_id = tagged_user_author_kp.public_key().to_z32();

    // Register the tagged post author PK in the new homeserver
    // We only need the record mapping, not necessarily the profile.json being uploaded
    test.register_user_in_hs(&tagged_user_author_kp, &tagged_user_hs_pk)
        .await?;

    // Create tagger user
    let tagger_author_kp = Keypair::random();

    let tagger_user = PubkyAppUser {
        bio: Some("test_tag_user_on_unknown_homeserver".to_string()),
        image: None,
        links: None,
        name: "Watcher:Homeserver:Tagger:User".to_string(),
        status: None,
    };
    test.create_user(&tagger_author_kp, &tagger_user).await?;

    let tagged_user_uri = user_uri_builder(tagged_user_author_id.clone());

    // Add a tag to the user
    let tag = PubkyAppTag {
        uri: tagged_user_uri.clone(),
        label: "test".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    // PUT tag
    let tag_path = tag.hs_path();
    test.put(&tagger_author_kp, &tag_path, tag).await?;

    // Check if the new homeserver of the unknown tagged user was ingested
    let tagged_user_hs_id = PubkyId::try_from(&tagged_user_hs_pk.to_z32()).unwrap();
    assert!(Homeserver::get_by_id(tagged_user_hs_id)
        .await
        .unwrap()
        .is_some());

    Ok(())
}
