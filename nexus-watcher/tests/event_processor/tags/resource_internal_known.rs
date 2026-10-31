use super::resource_utils::{compute_resource_id, resource_exists_in_graph};
use super::utils::{find_post_tag, find_user_tag};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{post_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use pubky_social_specs::post_uri_builder as social_v1_post_uri_builder;

/// When a tag at an app-specific path (/pub/mapky/tags/) targets a KNOWN Post URI,
/// a target that resolves to a post takes the existing Post tag flow.
/// This means the tag creates a (:User)-[:TAGGED]->(:Post) relationship, NOT a (:Resource).
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_internal_known_delegates_to_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create user + post
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_internal_known_delegation".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:InternalKnown".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:ResourceTag:InternalKnown:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    // Create a tag targeting this post, but stored at a non-pubky.app path
    let post_uri = post_uri_builder(user_id.clone(), post_id.clone());
    let label = "internal-known-test";

    let tag = PubkyAppTag {
        uri: post_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();

    // Store at /pub/mapky/tags/ instead of /pub/pubky.app/tags/
    let universal_tag_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    test.put(&user_kp, &universal_tag_path, &tag).await?;

    // Verify: should be indexed as a POST tag (existing flow), not a Resource
    let post_tag = find_post_tag(&user_id, &post_id, label).await?;
    assert!(
        post_tag.is_some(),
        "InternalKnown URI should be indexed as a Post tag"
    );

    let tag_details = post_tag.unwrap();
    assert_eq!(tag_details.label, label);
    assert_eq!(tag_details.taggers_count, 1);

    // Verify: NO Resource node was created for this URI
    let resource_id = compute_resource_id(&post_uri);
    let resource_exists = resource_exists_in_graph(&resource_id).await?;
    assert!(
        !resource_exists,
        "InternalKnown URI should NOT create a Resource node"
    );

    // Deleting the app-specific tag by URI should remove the tag from DB
    test.del(&user_kp, &universal_tag_path).await?;
    let post_tag = find_post_tag(&user_id, &post_id, label).await?;
    assert!(
        post_tag.is_none(),
        "InternalKnown app-specific tag should be deleted from Post tag indexes"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A tag at an app-specific path whose target is a bare `pubky://<pk>` names that user, so it
/// takes the User tag flow, as a tag on the user's `profile.json` address does.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_on_a_bare_key_delegates_to_user() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_user = PubkyAppUser {
        bio: Some("test_resource_tag_on_a_bare_key_delegates_to_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:BareKey:Tagged".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_resource_tag_on_a_bare_key_delegates_to_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:BareKey:Tagger".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    let bare_key = format!("pubky://{tagged_user_id}");
    let label = "known-bare-key";
    let tag = PubkyAppTag {
        uri: bare_key.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let universal_tag_path: ResourcePath =
        format!("/pub/mapky/tags/{}", tag.create_id()).parse()?;
    test.put(&tagger_kp, &universal_tag_path, &tag).await?;

    let user_tag = find_user_tag(&tagged_user_id, label)
        .await?
        .expect("a tag on a bare key should be indexed as a User tag");
    assert_eq!(user_tag.taggers_count, 1);
    assert_eq!(user_tag.taggers[0], tagger_user_id);
    assert!(
        !resource_exists_in_graph(&compute_resource_id(&bare_key)).await?,
        "a bare key should NOT create a Resource node"
    );

    test.del(&tagger_kp, &universal_tag_path).await?;
    assert!(
        find_user_tag(&tagged_user_id, label).await?.is_none(),
        "deleting the app-specific tag should remove the User tag"
    );

    test.cleanup_user(&tagged_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

/// A tag at an app-specific path whose target is a post's `social/v1` address names the same
/// post its `pubky.app` address does, so it takes the Post tag flow.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_on_a_social_v1_post_address_delegates_to_post() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_on_a_social_v1_post_address_delegates_to_post".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:SocialV1Post".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:ResourceTag:SocialV1Post:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
        lock: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let v1_post_uri = social_v1_post_uri_builder(user_id.clone(), post_id.clone());
    let label = "known-social-v1";
    let tag = PubkyAppTag {
        uri: v1_post_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let universal_tag_path: ResourcePath =
        format!("/pub/mapky/tags/{}", tag.create_id()).parse()?;
    test.put(&user_kp, &universal_tag_path, &tag).await?;

    let post_tag = find_post_tag(&user_id, &post_id, label)
        .await?
        .expect("a tag on a post's social/v1 address should be indexed as a Post tag");
    assert_eq!(post_tag.taggers_count, 1);
    assert!(
        !resource_exists_in_graph(&compute_resource_id(&v1_post_uri)).await?,
        "a post's social/v1 address should NOT create a Resource node"
    );

    test.del(&user_kp, &universal_tag_path).await?;
    assert!(
        find_post_tag(&user_id, &post_id, label).await?.is_none(),
        "deleting the app-specific tag should remove the Post tag"
    );

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
