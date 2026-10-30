//! Bookmarks of external resources: a bookmark whose target is not a post is a BOOKMARKED edge
//! to the target's Resource, shown to the bookmarker on the Resource view and removed (with an
//! unreferenced Resource) on delete.

use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use nexus_common::db::kv::SortOrder;
use nexus_common::models::resource::references;
use nexus_common::models::resource::stream::{
    ResourceSorting, ResourceStream, ResourceStreamSource,
};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::types::Pagination;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{traits::HashId, PubkyAppBookmark, PubkyAppUser};

fn user(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        bio: Some("test_resource_bookmark".to_string()),
        image: None,
        links: None,
        name: format!("Watcher:ResourceBookmark:{name}"),
        status: None,
    }
}

async fn bookmarked_ids(user_id: &str) -> Result<Vec<String>> {
    let keys = ResourceStream::get_resource_keys(
        &ResourceStreamSource::Bookmarks {
            user_id: user_id.to_string(),
        },
        Pagination {
            skip: None,
            limit: Some(10),
            start: None,
            end: None,
        },
        SortOrder::Descending,
        &ResourceSorting::Timeline,
        None,
    )
    .await?;
    Ok(keys.resource_ids)
}

#[tokio_shared_rt::test(shared)]
async fn test_bookmark_of_an_external_uri_is_a_resource_bookmark() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let alice_kp = Keypair::random();
    let alice = test.create_user(&alice_kp, &user("Alice")).await?;
    let bob_kp = Keypair::random();
    let bob = test.create_user(&bob_kp, &user("Bob")).await?;

    // The target as a client writes it; Nexus keys the Resource by its normalized form.
    let target = format!("https://Example.com/books/{alice}");
    let (normalized, _) = normalize_uri(&target).map_err(anyhow::Error::msg)?;
    let rid = resource_id(&normalized);
    let bookmark = PubkyAppBookmark {
        uri: target.clone(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let bookmark_id = bookmark.create_id();
    let path = bookmark.hs_path();

    test.put(&alice_kp, &path, &bookmark).await?;
    test.put(&bob_kp, &path, &bookmark).await?;

    let view = ResourceView::get_by_id(&rid, None, None, None, Some(&alice))
        .await?
        .expect("the bookmark creates the Resource");
    assert_eq!(view.details.uri, normalized);
    let mine = view.bookmark.expect("the viewer sees their bookmark");
    assert_eq!(mine.id, bookmark_id);
    assert!(mine.indexed_at > 0);
    let anonymous = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .unwrap();
    assert!(anonymous.bookmark.is_none(), "no viewer, no bookmark");
    assert_eq!(bookmarked_ids(&alice).await?, vec![rid.clone()]);

    // A repeated PUT keeps one edge and its first timestamp.
    test.put(&alice_kp, &path, &bookmark).await?;
    let again = ResourceView::get_by_id(&rid, None, None, None, Some(&alice))
        .await?
        .unwrap();
    assert_eq!(again.bookmark.unwrap().indexed_at, mine.indexed_at);

    // Alice's delete leaves Bob's bookmark, so the Resource stays.
    test.del(&alice_kp, &path).await?;
    assert!(bookmarked_ids(&alice).await?.is_empty());
    let view = ResourceView::get_by_id(&rid, None, None, None, Some(&alice))
        .await?
        .expect("Bob still references the Resource");
    assert!(view.bookmark.is_none());
    let view = ResourceView::get_by_id(&rid, None, None, None, Some(&bob))
        .await?
        .unwrap();
    assert!(view.bookmark.is_some());

    // The last bookmark's delete removes the unreferenced Resource.
    test.del(&bob_kp, &path).await?;
    assert!(!references::exists(&rid).await?);

    test.cleanup_user(&alice_kp).await?;
    test.cleanup_user(&bob_kp).await?;
    Ok(())
}
