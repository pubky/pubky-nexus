use super::resource_utils::{
    app_stream_resource_ids, compute_resource_id, count_resource_tags, find_resource_tag,
    resource_exists_in_graph,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::types::Pagination;
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
    assert_eq!(tag_result.app.as_deref(), Some("mapky"));

    // Cleanup
    test.del(&user_kp, &custom_path).await?;
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// The same user tags the same external URI with the same label from TWO
/// different app namespaces, creating two app-scoped TAGGED edges. Deleting the
/// first app's tag removes only that app's edge, so the user still tags the
/// label through the other app. Deleting the second removes the last edge and
/// the orphaned Resource. The stream follows the graph: the resource is listed
/// under each app while its edge exists and vanishes with the last edge.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_del_multi_app_full_cleanup() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let target_uri = "https://example.com/multi-app-del-test";
    let label = "multi-app-res-label";
    let suffix = Utc::now().timestamp_millis();
    let app1 = &format!("multiappa{suffix}");
    let app2 = &format!("multiappb{suffix}");
    let resource_id = compute_resource_id(target_uri);

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("resource_del_multi_app_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:ResourceDelMultiApp:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    // Tag the URI from app1
    let tag1 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let path1: ResourcePath = format!("/pub/{app1}/tags/{}", tag1.create_id()).parse()?;
    test.put(&user_kp, &path1, &tag1).await?;

    // Tag the same URI with the same label from app2
    let tag2 = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis() + 1,
    };
    let path2: ResourcePath = format!("/pub/{app2}/tags/{}", tag2.create_id()).parse()?;
    test.put(&user_kp, &path2, &tag2).await?;

    // Two app-scoped TAGGED edges
    assert_eq!(count_resource_tags(&resource_id).await?, 2);
    for app in [app1, app2] {
        assert_eq!(
            app_stream_resource_ids(app).await?,
            vec![resource_id.clone()],
            "resource must be listed under {app}"
        );
    }

    // Delete the app1 tag: only its edge goes, and the app2 tag keeps the
    // user among the label's taggers
    test.del(&user_kp, &path1).await?;

    assert_eq!(count_resource_tags(&resource_id).await?, 1);
    let (taggers, _) =
        TagResource::get_taggers(&resource_id, label, Pagination::default(), None).await?;
    assert_eq!(taggers, [user_id.as_str()], "the app2 tag still counts");
    assert_eq!(
        app_stream_resource_ids(app1).await?,
        Vec::<String>::new(),
        "the app1 stream follows the deleted edge"
    );
    assert_eq!(
        app_stream_resource_ids(app2).await?,
        vec![resource_id.clone()],
        "the app2 stream still lists the resource"
    );

    // Delete the app2 tag: its edge was the last one
    test.del(&user_kp, &path2).await?;

    assert_eq!(count_resource_tags(&resource_id).await?, 0);
    assert!(
        !resource_exists_in_graph(&resource_id).await?,
        "orphaned Resource node must be removed with its last tag"
    );
    for app in [app1, app2] {
        assert_eq!(
            app_stream_resource_ids(app).await?,
            Vec::<String>::new(),
            "resource must be gone from the {app} stream"
        );
    }

    test.cleanup_user(&user_kp).await?;

    Ok(())
}
