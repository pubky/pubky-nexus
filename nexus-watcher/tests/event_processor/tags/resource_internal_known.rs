use super::resource_utils::{compute_resource_id, resource_exists_in_graph};
use super::utils::find_post_tag;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{fetch_row_from_graph, queries};
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::TagCollection;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{post_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser};

/// When a tag at an app-specific path (/pub/mapky/tags/) targets a KNOWN Post URI,
/// the classify_uri logic should delegate to the existing Post tag flow.
/// This means the tag creates a (:User)-[:TAGGED]->(:Post) relationship, NOT a (:Resource).
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_internal_known_delegates_to_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

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
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    test.put(&user_kp, &custom_path, &tag).await?;

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

    // Regression: deleting the universal tag at the app-specific path must
    // actually remove the TAGGED edge. The DEL handler scopes the lookup by
    // `app`, so the PUT must persist the same `app` namespace on the edge.
    test.del(&user_kp, &custom_path).await?;

    let post_tag_after_del = find_post_tag(&user_id, &post_id, label).await?;
    assert!(
        post_tag_after_del.is_none(),
        "Universal tag DEL on InternalKnown URI should remove the TAGGED edge"
    );

    // Cleanup
    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_internal_known_app_tag_delete_preserves_other_app_post_tag_cache() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_internal_known_app_tag_delete_preserves_cache".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:InternalKnown:Cache".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:ResourceTag:InternalKnown:Cache:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

    let label = "internal-known-cache-test";
    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let mapky_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;
    let eventky_path: ResourcePath = format!("/pub/eventky.app/tags/{tag_id}").parse()?;

    test.put(&user_kp, &mapky_path, &tag).await?;
    test.put(&user_kp, &eventky_path, &tag).await?;

    let mapky_edge = fetch_row_from_graph(queries::get::get_tag_target(
        &user_id,
        &tag_id,
        Some("mapky"),
    ))
    .await?;
    assert!(
        mapky_edge.is_some(),
        "mapky post tag edge should exist before delete"
    );

    let eventky_edge = fetch_row_from_graph(queries::get::get_tag_target(
        &user_id,
        &tag_id,
        Some("eventky.app"),
    ))
    .await?;
    assert!(
        eventky_edge.is_some(),
        "eventky.app post tag edge should exist before delete"
    );

    test.del(&user_kp, &mapky_path).await?;

    let mapky_edge_after_del = fetch_row_from_graph(queries::get::get_tag_target(
        &user_id,
        &tag_id,
        Some("mapky"),
    ))
    .await?;
    assert!(
        mapky_edge_after_del.is_none(),
        "mapky post tag edge should be deleted"
    );

    let eventky_edge_after_del = fetch_row_from_graph(queries::get::get_tag_target(
        &user_id,
        &tag_id,
        Some("eventky.app"),
    ))
    .await?;
    assert!(
        eventky_edge_after_del.is_some(),
        "eventky.app post tag edge should remain after mapky delete"
    );

    let post_tag_after_app_del = find_post_tag(&user_id, &post_id, label)
        .await?
        .expect("eventky.app post tag should remain in graph after mapky delete");
    assert_eq!(post_tag_after_app_del.taggers_count, 1);

    let cache_post_tag =
        TagPost::get_from_index(&user_id, Some(&post_id), None, None, None, None, false)
            .await?
            .expect("eventky.app post tag should remain in Redis after mapky delete");
    assert_eq!(cache_post_tag.len(), 1);
    assert_eq!(cache_post_tag[0].label, label);
    assert_eq!(cache_post_tag[0].taggers_count, 1);
    assert_eq!(cache_post_tag[0].taggers[0], user_id);

    test.del(&user_kp, &eventky_path).await?;

    let post_tag_final = find_post_tag(&user_id, &post_id, label).await?;
    assert!(post_tag_final.is_none());

    test.cleanup_post(&user_kp, &post_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}
