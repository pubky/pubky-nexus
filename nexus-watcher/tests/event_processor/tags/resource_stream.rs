//! The resource stream is served from the graph. These tests pin down what a
//! resource tag put/del means for it: a resource's timeline position is its
//! latest matching tag, its taggers count is the number of distinct taggers
//! among the matching tags, and paging is stable.
//!
//! Every test isolates itself in a fresh app namespace, so the shared graph
//! can hold anything else without affecting the assertions.

use super::resource_utils::{compute_resource_id, latest_tag_indexed_at, resource_taggers_count};
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

async fn stream(
    app: Option<&str>,
    tags: Option<&[String]>,
    sorting: ResourceSorting,
    order: SortOrder,
    pagination: Pagination,
) -> Result<ResourceKeyStream> {
    let source = match app {
        Some(app) => ResourceStreamSource::App {
            app: app.to_string(),
        },
        None => ResourceStreamSource::All,
    };
    Ok(ResourceStream::get_resource_keys(&source, pagination, order, &sorting, tags).await?)
}

async fn app_timeline(app: &str, pagination: Pagination) -> Result<ResourceKeyStream> {
    stream(
        Some(app),
        None,
        ResourceSorting::Timeline,
        SortOrder::Descending,
        pagination,
    )
    .await
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
/// resource back to the time of its latest remaining tag. Cursor and offset
/// paging walk the same order.
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

    // Paging: one per page, resume from the last score (cursor + skip past
    // the row the inclusive cursor repeats), or by plain offset
    let first = app_timeline(app, page(1)).await?;
    assert_eq!(first.resource_ids, vec![x.clone()]);
    assert_eq!(first.last_score, Some(x_second_tag as u64));

    let by_cursor = app_timeline(
        app,
        Pagination {
            start: first.last_score.map(|s| s as f64),
            skip: Some(1),
            limit: Some(1),
            end: None,
        },
    )
    .await?;
    assert_eq!(by_cursor.resource_ids, vec![y.clone()]);
    assert_eq!(by_cursor.last_score, Some(y_tag as u64));

    let by_offset = app_timeline(
        app,
        Pagination {
            skip: Some(1),
            limit: Some(1),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(by_offset.resource_ids, vec![y.clone()]);

    // `end` is the hard limit: nothing older than Y's tag
    let bounded = app_timeline(
        app,
        Pagination {
            end: Some(y_tag as f64),
            limit: Some(10),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(bounded.resource_ids, vec![x.clone(), y.clone()]);
    let bounded = app_timeline(
        app,
        Pagination {
            end: Some((y_tag + 1) as f64),
            limit: Some(10),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(bounded.resource_ids, vec![x.clone()]);

    // Ascending walks the same order backwards
    let ascending = stream(
        Some(app),
        None,
        ResourceSorting::Timeline,
        SortOrder::Ascending,
        page(10),
    )
    .await?;
    assert_eq!(ascending.resource_ids, vec![y.clone(), x.clone()]);
    assert_eq!(ascending.last_score, Some(x_second_tag as u64));

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

/// Under a label filter only the matching tags place a resource: a newer tag
/// with another label neither moves it up nor hides it from a cursor walk.
#[tokio_shared_rt::test(shared)]
async fn test_resource_stream_timeline_ignores_non_matching_newer_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let suffix = Utc::now().timestamp_millis();
    let app = &format!("streamflt{suffix}");
    // Labels are capped at 20 chars; a short unique suffix keeps them valid
    let short = suffix % 1_000_000;
    let label = &format!("flt-{short}");
    let other_label = &format!("flto-{short}");
    let labels: Vec<String> = vec![label.clone()];

    let (kp, _) = create_user(&mut test, "Filtered").await?;

    let uri_a = format!("https://example.com/{app}/a");
    let uri_b = format!("https://example.com/{app}/b");
    let a = compute_resource_id(&uri_a);
    let b = compute_resource_id(&uri_b);

    // A then B under the filtered label, then a newer tag on A under another
    let path_a1 = put_tag(&mut test, &kp, app, &uri_a, label).await?;
    let a_matching_tag = latest_tag_indexed_at(&a, Some(app)).await?.unwrap();
    let path_b = put_tag(&mut test, &kp, app, &uri_b, label).await?;
    let b_tag = latest_tag_indexed_at(&b, Some(app)).await?.unwrap();
    let path_a2 = put_tag(&mut test, &kp, app, &uri_a, other_label).await?;
    let a_newest_tag = latest_tag_indexed_at(&a, Some(app)).await?.unwrap();
    assert!(a_matching_tag < b_tag && b_tag < a_newest_tag);

    let label_timeline = |pagination| {
        stream(
            Some(app),
            Some(&labels),
            ResourceSorting::Timeline,
            SortOrder::Descending,
            pagination,
        )
    };

    // Without the label filter the newer tag puts A first
    let keys = app_timeline(app, page(10)).await?;
    assert_eq!(keys.resource_ids, vec![a.clone(), b.clone()]);

    // With it, A sits at its latest matching tag, behind B
    let keys = label_timeline(page(10)).await?;
    assert_eq!(keys.resource_ids, vec![b.clone(), a.clone()]);
    assert_eq!(keys.last_score, Some(a_matching_tag as u64));

    // A cursor at B's tag: A's newest tag is past it, so the app timeline has
    // served A already, while the label timeline still has A to come
    let from_b = Pagination {
        start: Some(b_tag as f64),
        limit: Some(10),
        ..Default::default()
    };
    let keys = app_timeline(app, from_b).await?;
    assert_eq!(keys.resource_ids, vec![b.clone()]);
    let keys = label_timeline(from_b).await?;
    assert_eq!(keys.resource_ids, vec![b.clone(), a.clone()]);

    // One per page under the filter: B, then A, and B does not come back
    let first = label_timeline(page(1)).await?;
    assert_eq!(first.resource_ids, vec![b.clone()]);
    assert_eq!(first.last_score, Some(b_tag as u64));
    let second = label_timeline(Pagination {
        start: first.last_score.map(|s| s as f64),
        skip: Some(1),
        limit: Some(1),
        end: None,
    })
    .await?;
    assert_eq!(second.resource_ids, vec![a.clone()]);
    assert_eq!(second.last_score, Some(a_matching_tag as u64));

    // Cleanup
    test.del(&kp, &path_a1).await?;
    test.del(&kp, &path_a2).await?;
    test.del(&kp, &path_b).await?;
    test.cleanup_user(&kp).await?;

    Ok(())
}

/// The taggers count is the number of distinct taggers among the tags that
/// match the filters. One person counts once however many labels or apps they
/// tagged from; a second person counts. It is not the per-label sum a
/// ResourceView displays.
#[tokio_shared_rt::test(shared)]
async fn test_resource_stream_taggers_count_semantics() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let suffix = Utc::now().timestamp_millis();
    let app_a = &format!("streamsca{suffix}");
    let app_b = &format!("streamscb{suffix}");
    // Labels are capped at 20 chars; a short unique suffix keeps them valid
    let short = suffix % 1_000_000;
    let label_1 = &format!("sc1-{short}");
    let label_2 = &format!("sc2-{short}");
    let labels_1: Vec<String> = vec![label_1.clone()];

    let (kp_alice, _) = create_user(&mut test, "Alice").await?;
    let (kp_bob, _) = create_user(&mut test, "Bob").await?;

    let uri = format!("https://example.com/{suffix}/scored");
    let resource = compute_resource_id(&uri);

    // Alice tags with two labels from app A: still one tagger
    let path_a1 = put_tag(&mut test, &kp_alice, app_a, &uri, label_1).await?;
    let path_a2 = put_tag(&mut test, &kp_alice, app_a, &uri, label_2).await?;
    assert_eq!(
        resource_taggers_count(Some(app_a), None).await?,
        vec![(resource.clone(), 1)],
        "two labels by one person count once"
    );

    // The count is the sort key only: no cursor is handed out for it
    let keys = stream(
        Some(app_a),
        None,
        ResourceSorting::TaggersCount,
        SortOrder::Descending,
        page(10),
    )
    .await?;
    assert_eq!(keys.resource_ids, vec![resource.clone()]);
    assert_eq!(keys.last_score, None);

    // Alice repeats label 1 from app B: one more TAGGED edge, but the same
    // tagger, so the count across apps does not move
    let path_b1 = put_tag(&mut test, &kp_alice, app_b, &uri, label_1).await?;
    assert_eq!(
        resource_taggers_count(None, Some(&labels_1)).await?,
        vec![(resource.clone(), 1)],
        "the same person from two apps counts once"
    );
    // Under app B alone it is the same single tagger
    assert_eq!(
        resource_taggers_count(Some(app_b), None).await?,
        vec![(resource.clone(), 1)]
    );

    // Bob tags label 1 from app A: a second person counts
    let path_bob = put_tag(&mut test, &kp_bob, app_a, &uri, label_1).await?;
    assert_eq!(
        resource_taggers_count(Some(app_a), None).await?,
        vec![(resource.clone(), 2)]
    );
    assert_eq!(
        resource_taggers_count(None, Some(&labels_1)).await?,
        vec![(resource.clone(), 2)]
    );

    // Combined app + label filter counts the taggers on the matching edges only
    assert_eq!(
        resource_taggers_count(Some(app_a), Some(&labels_1)).await?,
        vec![(resource.clone(), 2)]
    );

    // The ranking counts people, so it stays below the per-label sum the view
    // displays (label 1 has two taggers, label 2 has one)
    let view = ResourceStream::from_listed_resource_ids(None, std::slice::from_ref(&resource))
        .await?
        .expect("resource view");
    assert_eq!(view.0.len(), 1);
    assert_eq!(view.0[0].taggers_count, 3);

    // Removing Bob's tag recomputes the count
    test.del(&kp_bob, &path_bob).await?;
    assert_eq!(
        resource_taggers_count(Some(app_a), None).await?,
        vec![(resource.clone(), 1)]
    );

    // Cleanup
    test.del(&kp_alice, &path_a1).await?;
    test.del(&kp_alice, &path_a2).await?;
    test.del(&kp_alice, &path_b1).await?;
    test.cleanup_user(&kp_alice).await?;
    test.cleanup_user(&kp_bob).await?;

    Ok(())
}
