use super::resource_utils::{
    compute_resource_id, count_resource_tags, find_resource_tag, resource_exists_in_graph,
};
use crate::event_processor::posts::utils::resource_key;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::user::UserIngestor;
use nexus_common::types::Pagination;
use nexus_watcher::events::inputs::{ReferenceTarget, TagInput};
use nexus_watcher::events::{handlers, put_tag};
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{file_uri_builder, PubkyAppTag, PubkyAppUser};
use pubky_social_specs::traits::HashId as SocialHashId;
use pubky_social_specs::{
    file_uri_builder as social_v1_file_uri_builder, tag_uri_builder as social_v1_tag_uri_builder,
    PubkySocialTag,
};

/// Full cycle test: PUT a `pubky.app` tag on an external URL and verify the Resource node is
/// created in Neo4j, with its label in the tag search index.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_external_uri() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Create a tagger user
    let user_kp = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_resource_tag_external".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:External".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&user_kp, &tagger).await?;

    // Create a tag targeting an external URL. The label is unique to this run,
    // so only this put can add it to the tag search index
    let target_uri = "https://example.com/article?q=test";
    let label = format!("resput{}", Utc::now().timestamp_millis() % 100_000_000);
    let label = label.as_str();

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let resource_id = compute_resource_id(target_uri);
    let tag_path = tag.hs_path();
    test.put(&user_kp, &tag_path, &tag).await?;

    // =============================================
    // LAYER 1: Verify Neo4j Graph
    // =============================================

    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Resource tag should exist in graph");

    assert_eq!(tag_result.label, label);
    assert_eq!(tag_result.app, None, "a resource tag edge carries no app");
    assert_eq!(tag_result.tagger, tagger_user_id);
    assert_eq!(tag_result.scheme, "https");
    // URI should be normalized (lowercase host, trailing slash rules, etc.)
    assert_eq!(tag_result.uri, "https://example.com/article?q=test");

    // =============================================
    // LAYER 2: Verify the tag list and the tag search index
    // =============================================

    // The tag list is read from the graph
    let tag_details = ResourceView::get_by_id(&resource_id, None, None, None, None)
        .await?
        .expect("Resource should have a tag list")
        .tags;
    assert_eq!(tag_details.len(), 1, "Should have one tag label");
    assert_eq!(tag_details[0].label, label);
    assert_eq!(tag_details[0].taggers, [tagger_user_id.as_str()]);

    // Global tag search index should contain the label
    let tag_search = TagSearch::get_by_label(label, &Pagination::default()).await?;
    assert!(
        tag_search.is_some_and(|v| !v.is_empty()),
        "Label should exist in global tag search"
    );

    // Cleanup: resource tag deletes leave the label in tag search
    test.del(&user_kp, &tag_path).await?;
    TagSearch::del_from_index(label).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A retried put finds the edge already written (`Updated`) and adds the label to
/// the tag search index again, so a lost index write heals.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_resource_tag_retry_heals_search_index() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_retry_heals_search_index".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:RetryHeal".to_string(),
        status: None,
    };
    test.create_user(&user_kp, &user).await?;

    // A label unique to this run, so no other tag keeps it in the search index
    let label = format!("resretry{}", Utc::now().timestamp_millis() % 100_000_000);
    let tag = PubkyAppTag {
        uri: "https://example.com/retry-heal".to_string(),
        label: label.clone(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&user_kp, &tag_path, &tag).await?;

    // Simulate the partial failure a retry recovers from: the edge exists, but
    // the search index write was lost
    TagSearch::del_from_index(&label).await?;

    // Re-put the same tag: the id hashes (uri, label), so it lands on the same
    // path and the graph reports the edge as Updated
    let retry_tag = PubkyAppTag {
        created_at: Utc::now().timestamp_millis(),
        ..tag
    };
    test.put(&user_kp, &tag_path, &retry_tag).await?;

    let tag_search = TagSearch::get_by_label(&label, &Pagination::default()).await?;
    assert!(
        tag_search.is_some_and(|v| !v.is_empty()),
        "The retry should add the label to tag search again"
    );

    // Resource tag deletes leave the label in tag search, so remove it here
    test.del(&user_kp, &tag_path).await?;
    TagSearch::del_from_index(&label).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Test tagging a pubky:// URI that is NOT a known Post/User (Internal-Unknown)
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_resource_tag_internal_unknown() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_internal_unknown".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:InternalUnknown".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Tag a pubky URI that's NOT pubky.app (eventky event)
    let target_uri = format!("pubky://{user_id}/pub/eventky.app/events/E001");
    let label = "conference";

    let tag = PubkyAppTag {
        uri: target_uri.clone(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let resource_id = compute_resource_id(&target_uri);
    let tag_path = tag.hs_path();
    test.put(&user_kp, &tag_path, &tag).await?;

    // Verify it was indexed as a Resource (not a Post or User)
    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Internal-unknown tag should be indexed as Resource");

    assert_eq!(tag_result.label, label);
    assert_eq!(tag_result.app, None, "a resource tag edge carries no app");
    assert_eq!(tag_result.scheme, "pubky");

    // Cleanup
    test.del(&user_kp, &tag_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A `pubky.app` tag picks its flow by what it tags: a marketplace listing in its own namespace,
/// `pubky.app` or `social/v1`, the Shop's `shop.json`, an `https` URL and a file at either
/// epoch's address each become a `Resource` tag.
/// The file is never indexed and never waited for: it is keyed by its address alone. A DEL of
/// the tag file removes the edge and the orphaned `Resource`.
#[tokio_shared_rt::test(shared)]
async fn test_pubky_app_tag_on_a_resource_target_creates_a_resource_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_pubky_app_tag_on_a_resource_target_creates_a_resource_tag".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:PubkyAppFolder".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let listing_id = "5d277f7f913f48659cbc6822362c8deb";
    let targets = [
        (
            format!("pubky://{user_id}/pub/marketplace/listings/0032SSN7Q4EVG"),
            "pubky",
        ),
        (
            format!("pubky://{user_id}/pub/pubky.app/marketplace/v1/listings/{listing_id}"),
            "pubky",
        ),
        (
            format!("pubky://{user_id}/pub/pubky.app/marketplace/v1/shop.json"),
            "pubky",
        ),
        (
            format!("pubky://{user_id}/pub/social/v1/marketplace/v1/listings/{listing_id}"),
            "pubky",
        ),
        (
            format!("https://example.com/pubky-app-folder/{user_id}"),
            "https",
        ),
        (
            file_uri_builder(user_id.clone(), "0032SSN7Q4EVG".to_string()),
            "pubky",
        ),
        (
            social_v1_file_uri_builder(
                user_id.clone(),
                "8Z8CWH8NVYQY39ZEBFGKQWWEKG.jpg".to_string(),
            ),
            "pubky",
        ),
    ];
    let label = "pubky-app-resource";
    for (target_uri, scheme) in targets {
        let tag = PubkyAppTag {
            uri: target_uri.clone(),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_path = tag.hs_path();
        let resource_id = compute_resource_id(&target_uri);
        test.put(&user_kp, &tag_path, &tag).await?;

        let tag_result = find_resource_tag(&resource_id, label)
            .await?
            .unwrap_or_else(|| panic!("{target_uri}: should be indexed as a Resource tag"));
        assert_eq!(tag_result.tagger, user_id, "{target_uri}");
        assert_eq!(tag_result.scheme, scheme, "{target_uri}");
        assert_eq!(count_resource_tags(&resource_id).await?, 1, "{target_uri}");

        test.del(&user_kp, &tag_path).await?;
        assert_eq!(count_resource_tags(&resource_id).await?, 0, "{target_uri}");
        assert!(
            !resource_exists_in_graph(&resource_id).await?,
            "{target_uri}: the orphaned Resource is removed with its last tag"
        );
    }

    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A `social/v1` tag names no app, and a tag on another app's object reaches the `Resource` flow
/// from there too. The classifier still skips every `social/v1` event, so the translated input
/// is handed to the tag dispatch directly.
#[tokio_shared_rt::test(shared)]
async fn test_social_v1_tag_on_a_foreign_target_reaches_the_resource_flow() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_social_v1_tag_on_a_foreign_target_reaches_the_resource_flow".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:SocialV1".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let target_uri = format!("pubky://{user_id}/pub/mapky/reviews/0032SSN7Q4EVG");
    let label = "social-v1-resource";
    let resource_id = compute_resource_id(&target_uri);
    let tag = PubkySocialTag::new(target_uri.clone(), label.to_string());
    let tag_id = SocialHashId::create_id(&tag);
    let tag_uri = social_v1_tag_uri_builder(user_id.clone(), tag_id.clone());
    let input = TagInput::from_v1(tag, tag_uri);
    assert_eq!(input.tagged.target, ReferenceTarget::Foreign);
    let key = resource_key(&user_id, &tag_id)?;
    let ingestor = UserIngestor::new([], test.event_processor_runner.client.clone());

    put_tag(input, key.clone(), &ingestor).await?;

    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("a social/v1 tag on another app's object should be indexed as a Resource tag");
    assert_eq!(tag_result.tagger, user_id);
    assert_eq!(tag_result.scheme, "pubky");

    handlers::tag::del(key).await?;
    assert!(
        !resource_exists_in_graph(&resource_id).await?,
        "the orphaned Resource is removed with its last tag"
    );

    test.cleanup_user(&user_kp).await?;

    Ok(())
}
