use super::resource_utils::{
    compute_resource_id, count_resource_tags, find_resource_tag, resource_exists_in_graph,
};
use crate::event_processor::posts::utils::resource_key;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, OperationOutcome};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::types::Pagination;
use nexus_watcher::events::handlers;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{PubkyAppTag, PubkyAppUser};

/// Test the full DEL cycle: PUT → DEL → verify Resource node is cleaned up
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_del_resource_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_del_resource_tag".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:Del".to_string(),
        status: None,
    };
    let _user_id = test.create_user(&user_kp, &user).await?;

    let target_uri = "https://example.com/to-be-deleted";
    let label = "temporary";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;

    // PUT the tag
    test.put(&user_kp, &custom_path, &tag).await?;

    // Verify it exists
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_some(), "Tag should exist after PUT");

    // DEL the tag
    test.del(&user_kp, &custom_path).await?;

    // Verify the TAGGED relationship is removed
    let tag_after_del = find_resource_tag(&resource_id, label).await?;
    assert!(tag_after_del.is_none(), "Tag should be gone after DEL");

    // Verify the Resource node is cleaned up (orphan removal)
    let exists = resource_exists_in_graph(&resource_id).await?;
    assert!(
        !exists,
        "Resource node should be deleted when no tags remain"
    );

    // Verify the tag list is gone with the Resource node
    let view = ResourceView::get_by_id(&resource_id, None, None, None, None).await?;
    assert!(view.is_none(), "Deleted Resource should have no tag list");

    // Cleanup user
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// Test PUT → DEL → re-PUT cycle (like post_put.rs unique count test)
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_resource_tag_put_del_reput() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_reput".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceTag:RePut".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let target_uri = "https://example.com/reput-test";
    let label = "reput-label";

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let custom_path: ResourcePath = format!("/pub/mapky/tags/{tag_id}").parse()?;

    // Step 1: PUT
    test.put(&user_kp, &custom_path, &tag).await?;
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_some(), "Tag should exist after first PUT");

    // Step 2: DEL
    test.del(&user_kp, &custom_path).await?;
    let tag_result = find_resource_tag(&resource_id, label).await?;
    assert!(tag_result.is_none(), "Tag should be gone after DEL");

    // Step 3: Re-PUT
    test.put(&user_kp, &custom_path, &tag).await?;
    let tag_result = find_resource_tag(&resource_id, label)
        .await?
        .expect("Tag should exist again after re-PUT");
    assert_eq!(tag_result.tagger, user_id);
    assert_eq!(tag_result.app, None, "a resource tag edge carries no app");

    // Cleanup
    test.del(&user_kp, &custom_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// The same user tags the same external URI with the same label from two app folders. The
/// tag id ignores the folder, so both files name one tag: one `TAGGED` edge, which the second
/// file leaves as it is. A DEL of either file removes that edge and the orphaned Resource,
/// and a DEL of the other one then finds nothing to remove.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_from_two_folders_is_one_edge() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let target_uri = "https://example.com/two-folders-one-edge";
    let label = "two-folder-label";
    let suffix = Utc::now().timestamp_millis();
    let app1 = &format!("twofoldera{suffix}");
    let app2 = &format!("twofolderb{suffix}");
    let resource_id = compute_resource_id(target_uri);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("resource_del_two_folders_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceDelTwoFolders:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let path1: ResourcePath = format!("/pub/{app1}/tags/{tag_id}").parse()?;
    let path2: ResourcePath = format!("/pub/{app2}/tags/{tag_id}").parse()?;

    // Each file is deleted first once
    for (first, second) in [(&path1, &path2), (&path2, &path1)] {
        test.put(&user_kp, &path1, &tag).await?;
        let (taggers, _) =
            TagResource::get_taggers(&resource_id, label, Pagination::default(), None).await?;
        assert_eq!(taggers, [user_id.as_str()]);

        // The second file names the same tag: still one edge and one tagger
        test.put(&user_kp, &path2, &tag).await?;
        assert_eq!(count_resource_tags(&resource_id).await?, 1, "one edge");
        let tags = ResourceView::get_by_id(&resource_id, None, None, None, None)
            .await?
            .expect("Resource should have a tag list")
            .tags;
        assert_eq!(tags.len(), 1, "one label");
        assert_eq!(tags[0].label, label);
        assert_eq!(tags[0].taggers_count, 1, "the second file adds no tagger");

        // A DEL of either file removes the one edge and the orphaned Resource
        test.del(&user_kp, first).await?;
        assert_eq!(count_resource_tags(&resource_id).await?, 0);
        assert!(
            !resource_exists_in_graph(&resource_id).await?,
            "orphaned Resource node must be removed with its last tag"
        );

        // The other file's DEL finds the tag already gone
        test.del(&user_kp, second).await?;
        assert_eq!(count_resource_tags(&resource_id).await?, 0);
        assert!(!resource_exists_in_graph(&resource_id).await?);
    }

    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// The mock graph's `calendar` tag on the eventky event stands in for the resource tag edges
/// written before a tag became one edge per tagger, target and label: it still carries `app`.
const OLD_EDGE_TAGGER: &str = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
const OLD_EDGE_TAG_ID: &str = "RES_TAG_004";
const OLD_EDGE_LABEL: &str = "calendar";
const OLD_EDGE_RESOURCE_ID: &str = "fb4155a2295ff3a8a8fe02e28229c021";
const OLD_EDGE_RESOURCE_URI: &str = "pubky://somepk/pub/eventky.app/events/E001";
const OLD_EDGE_INDEXED_AT: i64 = 1724544095003;
const OLD_EDGE_TAG_URI: &str =
    "pubky://ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny/pub/eventky/tags/RES_TAG_004";

/// Writes the mock graph's old edge and its Resource as `docker/test-graph/mocks/resources.cypher`
/// does, so the test starts from it however a previous run ended and leaves it for the others.
async fn seed_old_resource_tag_edge() -> Result<()> {
    let query = Query::new(
        "seed_old_resource_tag_edge",
        "MATCH (u:User {id: $tagger_id})
         MERGE (r:Resource {id: $resource_id})
         SET r.uri = $resource_uri, r.scheme = 'pubky', r.indexed_at = 1724234095000
         MERGE (u)-[rel:TAGGED {label: $label, id: $tag_id, indexed_at: $indexed_at, app: 'eventky'}]->(r)
         SET rel.uri = $tag_uri",
    )
    .param("tagger_id", OLD_EDGE_TAGGER)
    .param("resource_id", OLD_EDGE_RESOURCE_ID)
    .param("resource_uri", OLD_EDGE_RESOURCE_URI)
    .param("label", OLD_EDGE_LABEL)
    .param("tag_id", OLD_EDGE_TAG_ID)
    .param("indexed_at", OLD_EDGE_INDEXED_AT)
    .param("tag_uri", OLD_EDGE_TAG_URI);
    exec_single_row(query).await?;
    Ok(())
}

/// A resource tag edge written before the edge lost `app` is the same tag: a replay reuses it
/// (`Updated`, still one edge, `app` left in place), and a DEL removes it and its orphaned
/// Resource.
#[tokio_shared_rt::test(shared)]
async fn test_old_resource_tag_edge_with_app_is_reused_and_deleted() -> Result<()> {
    WatcherTest::setup(None).await?;
    seed_old_resource_tag_edge().await?;

    let old = find_resource_tag(OLD_EDGE_RESOURCE_ID, OLD_EDGE_LABEL)
        .await?
        .expect("the mock graph holds the old edge");
    assert_eq!(old.app.as_deref(), Some("eventky"));
    assert_eq!(count_resource_tags(OLD_EDGE_RESOURCE_ID).await?, 1);

    // Replay: the MERGE on the label matches the old edge
    let outcome = TagResource::put_to_graph_resource(
        OLD_EDGE_TAGGER,
        OLD_EDGE_RESOURCE_ID,
        OLD_EDGE_RESOURCE_URI,
        "pubky",
        OLD_EDGE_TAG_ID,
        OLD_EDGE_TAG_URI,
        OLD_EDGE_LABEL,
        Utc::now().timestamp_millis(),
    )
    .await?;
    assert!(
        matches!(outcome, OperationOutcome::Updated),
        "a replay finds the old edge, got {outcome:?}"
    );
    assert_eq!(
        count_resource_tags(OLD_EDGE_RESOURCE_ID).await?,
        1,
        "no new edge"
    );
    let replayed = find_resource_tag(OLD_EDGE_RESOURCE_ID, OLD_EDGE_LABEL)
        .await?
        .expect("the old edge is still there");
    assert_eq!(
        replayed.app.as_deref(),
        Some("eventky"),
        "the old app stays"
    );
    assert_eq!(replayed.tagger, OLD_EDGE_TAGGER);

    // DEL: found by its id alone, whatever its app
    let deleted = handlers::tag::del(resource_key(OLD_EDGE_TAGGER, OLD_EDGE_TAG_ID)?).await;

    // Put the mock edge back for the other tests before asserting
    let after_del = find_resource_tag(OLD_EDGE_RESOURCE_ID, OLD_EDGE_LABEL).await?;
    let resource_left = resource_exists_in_graph(OLD_EDGE_RESOURCE_ID).await?;
    seed_old_resource_tag_edge().await?;

    deleted?;
    assert!(after_del.is_none(), "the DEL removes the old edge");
    assert!(
        !resource_left,
        "the orphaned Resource node is removed with the old edge"
    );

    Ok(())
}
