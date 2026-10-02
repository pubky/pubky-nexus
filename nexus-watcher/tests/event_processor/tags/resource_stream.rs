//! The resource stream is served from the graph. These tests pin down what a
//! resource tag put/del means for it: a resource's timeline position is its
//! latest matching tag.
//!
//! Every test isolates itself in a fresh app namespace, so the shared graph
//! can hold anything else without affecting the assertions.

use super::resource_utils::{compute_resource_id, latest_tag_indexed_at};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::stream::{
    ResourceKeyStream, ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};
use std::time::Duration;

/// Watcher timestamps have millisecond resolution; keep consecutive tags apart
const TAG_GAP: Duration = Duration::from_millis(10);

async fn app_timeline(app: &str, pagination: Pagination) -> Result<ResourceKeyStream> {
    let source = ResourceStreamSource::App {
        app: app.to_string(),
    };
    Ok(ResourceStream::get_resource_keys(
        &source,
        pagination,
        SortOrder::Descending,
        &ResourceSorting::Timeline,
        None,
    )
    .await?)
}

fn page(limit: usize) -> Pagination {
    Pagination {
        limit: Some(limit),
        ..Default::default()
    }
}

async fn put_tag(
    test: &mut WatcherTest,
    kp: &Keypair,
    app: &str,
    uri: &str,
    label: &str,
) -> Result<ResourcePath> {
    // Consecutive tags must not share a millisecond, or their order is a coin toss
    tokio::time::sleep(TAG_GAP).await;
    let tag = PubkyAppTag {
        uri: uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let path: ResourcePath = format!("/pub/{app}/tags/{}", tag.create_id()).parse()?;
    test.put(kp, &path, &tag).await?;
    Ok(path)
}

async fn create_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some(format!("resource_stream_{name}")),
        image: None,
        links: None,
        name: format!("Watcher:ResourceStream:{name}"),
        status: None,
    };
    let id = test.create_user(&kp, &user).await?;
    Ok((kp, id))
}

/// Adding a newer tag moves a resource up the timeline; removing it drops the
/// resource back to the time of its latest remaining tag; removing the last
/// tag removes it from the stream.
#[tokio_shared_rt::test(shared)]
async fn test_resource_stream_timeline_follows_latest_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let app = &format!("streamtl{}", Utc::now().timestamp_millis());
    let label = "stream-timeline";

    let (kp, _) = create_user(&mut test, "Timeline").await?;

    let uri_x = format!("https://example.com/{app}/x");
    let uri_y = format!("https://example.com/{app}/y");
    let x = compute_resource_id(&uri_x);
    let y = compute_resource_id(&uri_y);

    // X is tagged first, then Y: Y leads the timeline
    let path_x1 = put_tag(&mut test, &kp, app, &uri_x, label).await?;
    let path_y = put_tag(&mut test, &kp, app, &uri_y, label).await?;
    let x_first_tag = latest_tag_indexed_at(&x, Some(app)).await?.unwrap();
    let y_tag = latest_tag_indexed_at(&y, Some(app)).await?.unwrap();
    assert!(x_first_tag < y_tag);

    let keys = app_timeline(app, page(10)).await?;
    assert_eq!(keys.resource_ids, vec![y.clone(), x.clone()]);
    assert_eq!(keys.last_score, Some(x_first_tag as u64));

    // A newer tag on X (another label) moves X ahead of Y
    let path_x2 = put_tag(&mut test, &kp, app, &uri_x, "stream-newer").await?;
    let x_second_tag = latest_tag_indexed_at(&x, Some(app)).await?.unwrap();
    assert!(x_second_tag > y_tag);

    let keys = app_timeline(app, page(10)).await?;
    assert_eq!(keys.resource_ids, vec![x.clone(), y.clone()]);
    assert_eq!(keys.last_score, Some(y_tag as u64));

    // Removing the newer tag recomputes X from its remaining tag: Y leads again
    test.del(&kp, &path_x2).await?;
    assert_eq!(
        latest_tag_indexed_at(&x, Some(app)).await?,
        Some(x_first_tag)
    );
    let keys = app_timeline(app, page(10)).await?;
    assert_eq!(keys.resource_ids, vec![y.clone(), x.clone()]);
    assert_eq!(keys.last_score, Some(x_first_tag as u64));

    // Removing the last tag removes the resource from the stream
    test.del(&kp, &path_x1).await?;
    let keys = app_timeline(app, page(10)).await?;
    assert_eq!(keys.resource_ids, vec![y.clone()]);

    // Cleanup
    test.del(&kp, &path_y).await?;
    test.cleanup_user(&kp).await?;

    Ok(())
}
