//! The tags step: a migrated user's tag edges move to their v1 ids and addresses at the flip,
//! so the `DEL` of the migrator's tag file removes them, and reruns change nothing.
//!
//! Tags are written through the event harness so `t.id` is real; only the no-match rows are
//! seeded with an id nothing reproduces. The migrator's copies are written before the profile,
//! as it writes them, and ignored until the flip.

use super::attachments::{create_file, upload_blob, v1_file_uri};
use crate::event_processor::posts::utils::{
    find_post_uri, pubky_id, short_post, test_user, v1_test_user,
};
use crate::event_processor::tags::resource_utils::{
    compute_resource_id, count_resource_tags, resource_exists_in_graph,
};
use crate::event_processor::tags::utils::{find_post_tag, find_tag_uri, find_user_tag};
use crate::event_processor::users::utils::{assert_flipped, set_user_epoch};
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::graph::Query;
use nexus_common::db::{exec_single_row, fetch_all_rows_from_graph};
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::traits::TagCollection;
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::tag::TagDetails;
use nexus_watcher::events::v0_to_v1;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::traits::HashId as V0HashId;
use pubky_social_specs::legacy_v0::{
    file_uri_builder as v0_file_uri_builder, post_uri_builder as v0_post_uri_builder,
    tag_uri_builder as v0_tag_uri_builder, user_uri_builder as v0_user_uri_builder, PubkyAppTag,
};
use pubky_social_specs::traits::HashId;
use pubky_social_specs::{
    post_uri_builder as v1_post_uri_builder, tag_uri_builder as v1_tag_uri_builder,
    user_uri_builder as v1_user_uri_builder, PubkySocialTag,
};

/// A `pubky.app` tag on `target`.
fn v0_tag(target: &str, label: &str) -> PubkyAppTag {
    PubkyAppTag {
        uri: target.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    }
}

/// The migrator's copy of a tag: a `social/v1` tag on `target`, which may be the same address.
fn v1_tag(target: &str, label: &str) -> PubkySocialTag {
    PubkySocialTag::new(target.to_string(), label.to_string())
}

/// The `social/v1` tag file address of `tagger_id`'s tag `tag_id`.
fn v1_tag_uri(tagger_id: &str, tag_id: &str) -> String {
    v1_tag_uri_builder(tagger_id.to_string(), tag_id.to_string())
}

/// The event line deleting `tagger_id`'s `social/v1` tag `tag_id`.
fn v1_tag_del_line(tagger_id: &str, tag_id: &str) -> String {
    format!("DEL {}", v1_tag_uri(tagger_id, tag_id))
}

/// The viewer's address on `label` among `tags`, as the API serves it.
fn uri_on(tags: Option<Vec<TagDetails>>, label: &str) -> Option<String> {
    tags.unwrap_or_default()
        .into_iter()
        .find(|tag| tag.label == label)
        .and_then(|tag| tag.tag_uri)
}

/// The viewer's address on `label`, as the API serves it on the post.
async fn served_post_tag_uri(
    author_id: &str,
    post_id: &str,
    viewer_id: &str,
    label: &str,
) -> Result<Option<String>> {
    let tags = TagPost::get_by_id(
        author_id,
        Some(post_id),
        None,
        None,
        None,
        Some(viewer_id),
        None,
    )
    .await?;
    Ok(uri_on(tags, label))
}

/// The viewer's address on `label`, as the API serves it on the user.
async fn served_user_tag_uri(
    user_id: &str,
    viewer_id: &str,
    label: &str,
) -> Result<Option<String>> {
    let tags = TagUser::get_by_id(user_id, None, None, None, None, Some(viewer_id), None).await?;
    Ok(uri_on(tags, label))
}

/// The viewer's address on `label`, as the API serves it on the resource; `None` when the
/// resource is gone.
async fn served_resource_tag_uri(
    resource_id: &str,
    viewer_id: &str,
    label: &str,
) -> Result<Option<Option<String>>> {
    let Some(view) =
        ResourceView::get_by_id(resource_id, None, None, None, Some(viewer_id)).await?
    else {
        return Ok(None);
    };
    Ok(Some(uri_on(Some(view.tags), label)))
}

/// Plants a `TAGGED` edge from `tagger_id` to the user `tagged_id` with the id and address
/// given, as the mock graph's made-up ids reach the graph.
async fn seed_user_tag_edge(
    tagger_id: &str,
    tagged_id: &str,
    tag_id: &str,
    tag_uri: &str,
    label: &str,
) -> Result<()> {
    let query = Query::new(
        "seed_user_tag_edge",
        "MATCH (tagger:User {id: $tagger_id}), (tagged:User {id: $tagged_id})
        CREATE (tagger)-[:TAGGED {id: $tag_id, uri: $tag_uri, label: $label, indexed_at: $now}]->(tagged)",
    )
    .param("tagger_id", tagger_id)
    .param("tagged_id", tagged_id)
    .param("tag_id", tag_id)
    .param("tag_uri", tag_uri)
    .param("label", label)
    .param("now", Utc::now().timestamp_millis());
    exec_single_row(query).await?;
    Ok(())
}

/// Plants a `Post` with no `AUTHORED` author and a `TAGGED` edge from `tagger_id` to it.
async fn seed_authorless_post_tag_edge(
    tagger_id: &str,
    post_id: &str,
    tag_id: &str,
    tag_uri: &str,
    label: &str,
) -> Result<()> {
    let query = Query::new(
        "seed_authorless_post_tag_edge",
        "MATCH (tagger:User {id: $tagger_id})
        CREATE (p:Post {id: $post_id, content: 'Watcher:Migration:Tags:Orphan', kind: 'short', indexed_at: $now})
        CREATE (tagger)-[:TAGGED {id: $tag_id, uri: $tag_uri, label: $label, indexed_at: $now}]->(p)",
    )
    .param("tagger_id", tagger_id)
    .param("post_id", post_id)
    .param("tag_id", tag_id)
    .param("tag_uri", tag_uri)
    .param("label", label)
    .param("now", Utc::now().timestamp_millis());
    exec_single_row(query).await?;
    Ok(())
}

/// Plants a `Resource` and a `TAGGED` edge from `tagger_id` to it stored under another app's
/// folder, as the old per-app edges were.
async fn seed_app_folder_resource_tag_edge(
    tagger_id: &str,
    resource_uri: &str,
    tag_id: &str,
    tag_uri: &str,
    label: &str,
) -> Result<()> {
    let query = Query::new(
        "seed_app_folder_resource_tag_edge",
        "MATCH (tagger:User {id: $tagger_id})
        MERGE (r:Resource {id: $resource_id})
        ON CREATE SET r.uri = $resource_uri, r.scheme = 'https', r.indexed_at = $now
        CREATE (tagger)-[:TAGGED {id: $tag_id, uri: $tag_uri, label: $label, app: 'other.app', indexed_at: $now}]->(r)",
    )
    .param("tagger_id", tagger_id)
    .param("resource_id", compute_resource_id(resource_uri))
    .param("resource_uri", resource_uri)
    .param("tag_id", tag_id)
    .param("tag_uri", tag_uri)
    .param("label", label)
    .param("now", Utc::now().timestamp_millis());
    exec_single_row(query).await?;
    Ok(())
}

/// Removes a seeded node and whatever points at it.
async fn detach_delete(label: &str, id: &str) -> Result<()> {
    let query = Query::new(
        "detach_delete_seeded",
        format!("MATCH (n:{label} {{id: $id}}) DETACH DELETE n"),
    )
    .param("id", id);
    exec_single_row(query).await?;
    Ok(())
}

/// Every `TAGGED` edge `tagger_id` wrote, as `(label, id, uri, target labels, target id)`,
/// ordered by label: what the migration leaves in the graph.
async fn tag_edges_snapshot(
    tagger_id: &str,
) -> Result<Vec<(String, String, String, Vec<String>, String)>> {
    let query = Query::new(
        "tag_edges_snapshot",
        "MATCH (:User {id: $tagger_id})-[t:TAGGED]->(target)
        RETURN t.label AS label, t.id AS id, t.uri AS uri, labels(target) AS kinds,
            coalesce(target.id, '') AS target_id
        ORDER BY t.label ASC",
    )
    .param("tagger_id", tagger_id);
    let rows = fetch_all_rows_from_graph(query).await?;
    let mut edges = Vec::new();
    for row in &rows {
        edges.push((
            row.get("label")?,
            row.get("id")?,
            row.get("uri")?,
            row.get("kinds")?,
            row.get("target_id")?,
        ));
    }
    Ok(edges)
}

/// Puts an edge back as a `pubky.app` tag left it, by its label: a partly migrated state.
async fn reset_tag_edge(tagger_id: &str, label: &str, id: &str, uri: &str) -> Result<()> {
    let query = Query::new(
        "reset_tag_edge",
        "MATCH (:User {id: $tagger_id})-[t:TAGGED {label: $label}]->()
        SET t.id = $id, t.uri = $uri",
    )
    .param("tagger_id", tagger_id)
    .param("label", label)
    .param("id", id)
    .param("uri", uri);
    exec_single_row(query).await?;
    Ok(())
}

/// A tag on an own post and one on another user's `v0` post take the id of the v1 post URI
/// built from the keys, and a tag written on that post's versionless `social/v1` address keeps
/// its id; `tag_uri` names the v1 file on all three, the other user's `Post.uri` stays
/// `pubky.app`, and the `v1` `DEL` removes each edge.
#[tokio_shared_rt::test(shared)]
async fn test_flip_rewrites_post_tags_and_their_v1_del_removes_them() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:Posts:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Tags:Posts:Other", "migration"),
        )
        .await?;
    let (own_post_id, own_post_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Tags:Posts:Own"))
        .await?;
    let (their_post_id, their_post_path) = test
        .create_post(
            &other_kp,
            &short_post("Watcher:Migration:Tags:Posts:Theirs"),
        )
        .await?;

    let own_label = "migownpost";
    let their_label = "migtheirpost";
    let kept_label = "migkeptpost";
    let own_v0 = v0_tag(
        &v0_post_uri_builder(author_id.clone(), own_post_id.clone()),
        own_label,
    );
    let their_v0 = v0_tag(
        &v0_post_uri_builder(other_id.clone(), their_post_id.clone()),
        their_label,
    );
    // A `pubky.app` tag written on the other user's post by its `social/v1` address
    let kept_v0 = v0_tag(
        &v1_post_uri_builder(other_id.clone(), their_post_id.clone()),
        kept_label,
    );
    test.put(&author_kp, &own_v0.hs_path(), &own_v0).await?;
    test.put(&author_kp, &their_v0.hs_path(), &their_v0).await?;
    test.put(&author_kp, &kept_v0.hs_path(), &kept_v0).await?;

    // The migrator's copies, before the profile: ignored, the user is still `v0`
    let own_v1 = v1_tag(
        &v1_post_uri_builder(author_id.clone(), own_post_id.clone()),
        own_label,
    );
    let their_v1 = v1_tag(
        &v1_post_uri_builder(other_id.clone(), their_post_id.clone()),
        their_label,
    );
    let kept_v1 = v1_tag(
        &v1_post_uri_builder(other_id.clone(), their_post_id.clone()),
        kept_label,
    );
    let own_v1_path = test.put_v1_tag(&author_kp, &own_v1).await?;
    let their_v1_path = test.put_v1_tag(&author_kp, &their_v1).await?;
    let kept_v1_path = test.put_v1_tag(&author_kp, &kept_v1).await?;
    assert_eq!(
        find_tag_uri(&author_id, &own_v0.create_id()).await,
        Some(v0_tag_uri_builder(author_id.clone(), own_v0.create_id()))
    );
    assert_eq!(kept_v0.create_id(), kept_v1.create_id());
    assert_eq!(
        find_tag_uri(&author_id, &kept_v0.create_id()).await,
        Some(v0_tag_uri_builder(author_id.clone(), kept_v0.create_id()))
    );

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Tags:Posts:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;

    for (v0, v1) in [(&own_v0, &own_v1), (&their_v0, &their_v1)] {
        assert_ne!(v0.create_id(), v1.create_id());
        assert_eq!(find_tag_uri(&author_id, &v0.create_id()).await, None);
        assert_eq!(
            find_tag_uri(&author_id, &v1.create_id()).await,
            Some(v1_tag_uri(&author_id, &v1.create_id()))
        );
    }
    assert_eq!(
        served_post_tag_uri(&author_id, &own_post_id, &author_id, own_label).await?,
        Some(v1_tag_uri(&author_id, &own_v1.create_id()))
    );
    assert_eq!(
        served_post_tag_uri(&other_id, &their_post_id, &author_id, their_label).await?,
        Some(v1_tag_uri(&author_id, &their_v1.create_id()))
    );
    // The tag on the v1 address kept its id and moved to the v1 file
    assert_eq!(
        find_tag_uri(&author_id, &kept_v0.create_id()).await,
        Some(v1_tag_uri(&author_id, &kept_v0.create_id()))
    );
    assert_eq!(
        served_post_tag_uri(&other_id, &their_post_id, &author_id, kept_label).await?,
        Some(v1_tag_uri(&author_id, &kept_v1.create_id()))
    );
    assert_eq!(
        find_post_uri(&other_id, &their_post_id).await,
        Some(v0_post_uri_builder(other_id.clone(), their_post_id.clone()))
    );

    // The user untags in the new app: the `DEL` of the v1 file finds the edge
    test.del(&author_kp, &own_v1_path).await?;
    test.del(&author_kp, &their_v1_path).await?;
    test.del(&author_kp, &kept_v1_path).await?;
    assert!(find_post_tag(&author_id, &own_post_id, own_label)
        .await?
        .is_none());
    assert!(find_post_tag(&other_id, &their_post_id, their_label)
        .await?
        .is_none());
    assert!(find_post_tag(&other_id, &their_post_id, kept_label)
        .await?
        .is_none());

    // Back to `v0`, so the gate lets the `pubky.app` cleanup through
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&other_kp, &their_post_path).await?;
    test.cleanup_post(&author_kp, &own_post_path).await?;
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A tag on another user's profile takes the id of the v1 profile URI, a bare tag on
/// `pubky://pk` and a tag on a URL with a fragment keep theirs; every address names the v1
/// file, the API serves it, and the `v1` `DEL` removes each one.
#[tokio_shared_rt::test(shared)]
async fn test_flip_rewrites_user_and_url_tags_and_their_v1_del_removes_them() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:Users:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Tags:Users:Other", "migration"),
        )
        .await?;

    let profile_label = "migprofile";
    let bare_label = "migbare";
    let url_label = "migurl";
    let bare = format!("pubky://{other_id}");
    let url = format!("https://example.com/migration/{author_id}#top");
    let resource_id = compute_resource_id(&url);
    let profile_v0 = v0_tag(&v0_user_uri_builder(other_id.clone()), profile_label);
    let bare_v0 = v0_tag(&bare, bare_label);
    let url_v0 = v0_tag(&url, url_label);
    for tag in [&profile_v0, &bare_v0, &url_v0] {
        test.put(&author_kp, &tag.hs_path(), tag).await?;
    }

    // The migrator rewrites the profile, keeps the bare key and the URL as written
    let profile_v1 = v1_tag(&v1_user_uri_builder(other_id.clone()), profile_label);
    let bare_v1 = v1_tag(&bare, bare_label);
    let url_v1 = v1_tag(&url, url_label);
    assert_ne!(profile_v0.create_id(), profile_v1.create_id());
    assert_eq!(bare_v0.create_id(), bare_v1.create_id());
    assert_eq!(url_v0.create_id(), url_v1.create_id());
    let profile_v1_path = test.put_v1_tag(&author_kp, &profile_v1).await?;
    let bare_v1_path = test.put_v1_tag(&author_kp, &bare_v1).await?;
    let url_v1_path = test.put_v1_tag(&author_kp, &url_v1).await?;

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Tags:Users:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;

    assert_eq!(
        find_tag_uri(&author_id, &profile_v0.create_id()).await,
        None
    );
    for tag in [&profile_v1, &bare_v1, &url_v1] {
        assert_eq!(
            find_tag_uri(&author_id, &tag.create_id()).await,
            Some(v1_tag_uri(&author_id, &tag.create_id())),
            "{}",
            tag.label
        );
    }
    assert_eq!(
        served_user_tag_uri(&other_id, &author_id, profile_label).await?,
        Some(v1_tag_uri(&author_id, &profile_v1.create_id()))
    );
    assert_eq!(
        served_user_tag_uri(&other_id, &author_id, bare_label).await?,
        Some(v1_tag_uri(&author_id, &bare_v1.create_id()))
    );
    assert_eq!(
        served_resource_tag_uri(&resource_id, &author_id, url_label).await?,
        Some(Some(v1_tag_uri(&author_id, &url_v1.create_id())))
    );

    test.del(&author_kp, &profile_v1_path).await?;
    test.del(&author_kp, &bare_v1_path).await?;
    test.del(&author_kp, &url_v1_path).await?;
    assert!(find_user_tag(&other_id, profile_label).await?.is_none());
    assert!(find_user_tag(&other_id, bare_label).await?.is_none());
    assert!(!resource_exists_in_graph(&resource_id).await?);

    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A tag on an own file takes the id of its `{hash}.{ext}` v1 address. A late copy at that
/// address adds a second edge with the same id on a second `Resource`; the `DEL` removes both,
/// and both empty `Resource`s go with them.
#[tokio_shared_rt::test(shared)]
async fn test_flip_rewrites_an_own_file_tag_and_the_del_removes_its_late_copy_too() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:File:Author", "migration"),
        )
        .await?;
    let unique = format!("{author_id}:{}", Utc::now().timestamp_micros());
    let (hash, src) = upload_blob(
        &mut test,
        &author_kp,
        &author_id,
        &format!("tagged:{unique}"),
    )
    .await?;
    let (file_id, file_path, file_uri) = create_file(
        &mut test,
        &author_kp,
        &author_id,
        "tagged.png",
        &src,
        "image/png",
    )
    .await?;
    assert_eq!(
        file_uri,
        v0_file_uri_builder(author_id.clone(), file_id.clone())
    );
    let migrated_file = v1_file_uri(&author_id, &hash, "png");
    let v0_resource_id = compute_resource_id(&file_uri);
    let v1_resource_id = compute_resource_id(&migrated_file);

    let label = "migfile";
    let file_v0 = v0_tag(&file_uri, label);
    test.put(&author_kp, &file_v0.hs_path(), &file_v0).await?;
    assert_eq!(count_resource_tags(&v0_resource_id).await?, 1);

    let file_v1 = v1_tag(&migrated_file, label);
    let tag_id = file_v1.create_id();
    assert_ne!(file_v0.create_id(), tag_id);
    let file_v1_path = test.put_v1_tag(&author_kp, &file_v1).await?;

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Tags:File:Author:V1", "migration"),
        )
        .await?;
    assert_flipped(&author_id).await;
    assert_eq!(find_tag_uri(&author_id, &file_v0.create_id()).await, None);
    assert_eq!(
        find_tag_uri(&author_id, &tag_id).await,
        Some(v1_tag_uri(&author_id, &tag_id))
    );
    assert_eq!(
        served_resource_tag_uri(&v0_resource_id, &author_id, label).await?,
        Some(Some(v1_tag_uri(&author_id, &tag_id)))
    );

    // The late copy: a re-tag in the new app, at the v1 file address
    test.put_v1_tag(&author_kp, &file_v1).await?;
    assert_eq!(count_resource_tags(&v0_resource_id).await?, 1);
    assert_eq!(count_resource_tags(&v1_resource_id).await?, 1);
    assert_eq!(
        served_resource_tag_uri(&v1_resource_id, &author_id, label).await?,
        Some(Some(v1_tag_uri(&author_id, &tag_id)))
    );

    test.del(&author_kp, &file_v1_path).await?;
    assert_eq!(
        served_resource_tag_uri(&v0_resource_id, &author_id, label).await?,
        None,
        "the tag and its empty Resource must be gone at the v0 file address"
    );
    assert_eq!(
        served_resource_tag_uri(&v1_resource_id, &author_id, label).await?,
        None,
        "the tag and its empty Resource must be gone at the v1 file address"
    );
    assert!(!resource_exists_in_graph(&v0_resource_id).await?);
    assert!(!resource_exists_in_graph(&v1_resource_id).await?);
    assert_eq!(find_tag_uri(&author_id, &tag_id).await, None);

    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_file(&author_kp, &file_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// The rows real data can't produce, seeded with ids nothing reproduces: a user edge no
/// spelling matches and an edge to a post with no author keep their id and get a `social/v1`
/// address, the migration finishes, and the user edge stays removable by its old id. An edge
/// in another app's folder and another user's edge are not touched.
#[tokio_shared_rt::test(shared)]
async fn test_flip_keeps_unmatched_ids_and_leaves_other_edges_alone() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:Seeded:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Tags:Seeded:Other", "migration"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(
            &author_kp,
            &short_post("Watcher:Migration:Tags:Seeded:Post"),
        )
        .await?;

    // Another user's edge, on the migrating user's post
    let their_label = "migotheredge";
    let their_tag = v0_tag(
        &v0_post_uri_builder(author_id.clone(), post_id.clone()),
        their_label,
    );
    test.put(&other_kp, &their_tag.hs_path(), &their_tag)
        .await?;
    let their_tag_uri = v0_tag_uri_builder(other_id.clone(), their_tag.create_id());

    // Well-formed ids that no spelling of the target reproduces
    let no_match_label = "mignomatch";
    let no_match_id = v0_tag("https://example.com/elsewhere", no_match_label).create_id();
    let no_match_uri = v0_tag_uri_builder(author_id.clone(), no_match_id.clone());
    seed_user_tag_edge(
        &author_id,
        &other_id,
        &no_match_id,
        &no_match_uri,
        no_match_label,
    )
    .await?;
    let orphan_label = "migorphan";
    let orphan_post_id = format!("ORPHAN{}", author_id[..7].to_uppercase());
    let orphan_id = v0_tag("https://example.com/orphan", orphan_label).create_id();
    let orphan_uri = v0_tag_uri_builder(author_id.clone(), orphan_id.clone());
    seed_authorless_post_tag_edge(
        &author_id,
        &orphan_post_id,
        &orphan_id,
        &orphan_uri,
        orphan_label,
    )
    .await?;
    let app_label = "migappfolder";
    let app_resource = format!("https://example.com/app-folder/{author_id}");
    let app_id = v0_tag(&app_resource, app_label).create_id();
    let app_uri = format!("pubky://{author_id}/pub/other.app/tags/{app_id}");
    seed_app_folder_resource_tag_edge(&author_id, &app_resource, &app_id, &app_uri, app_label)
        .await?;

    let profile_path = test
        .put_v1_profile(
            &author_kp,
            &v1_test_user("Watcher:Migration:Tags:Seeded:Author:V1", "migration"),
        )
        .await?;
    // The migration must finish over the seeded rows
    assert_flipped(&author_id).await;

    assert_eq!(
        find_tag_uri(&author_id, &no_match_id).await,
        Some(v1_tag_uri(&author_id, &no_match_id))
    );
    assert_eq!(
        find_tag_uri(&author_id, &orphan_id).await,
        Some(v1_tag_uri(&author_id, &orphan_id))
    );
    assert_eq!(find_tag_uri(&author_id, &app_id).await, Some(app_uri));
    assert_eq!(
        find_tag_uri(&other_id, &their_tag.create_id()).await,
        Some(their_tag_uri)
    );

    // Removable by its old id, now at the v1 address
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&v1_tag_del_line(&author_id, &no_match_id), event_handler)
        .await?;
    assert!(find_user_tag(&other_id, no_match_label).await?.is_none());
    assert_eq!(find_tag_uri(&author_id, &no_match_id).await, None);

    detach_delete("Post", &orphan_post_id).await?;
    detach_delete("Resource", &compute_resource_id(&app_resource)).await?;
    test.del(&other_kp, &their_tag.hs_path()).await?;
    set_user_epoch(&author_id, "v0").await?;
    test.del(&author_kp, &profile_path).await?;
    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// Over more than one batch, running the migration again, and running it from a partly
/// migrated state, leaves the same edges.
#[tokio_shared_rt::test(shared)]
async fn test_tags_migration_rerun_gives_the_same_result() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:Rerun:Author", "migration"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other_id = test
        .create_user(
            &other_kp,
            &test_user("Watcher:Migration:Tags:Rerun:Other", "migration"),
        )
        .await?;
    let mut post_paths = Vec::new();
    let mut tags = Vec::new();
    // The id each edge takes: the migrator's copy, on the v1 target
    let mut expected_ids = Vec::new();
    for n in 0..3 {
        let (post_id, post_path) = test
            .create_post(
                &author_kp,
                &short_post(format!("Watcher:Migration:Tags:Rerun:Post:{n}")),
            )
            .await?;
        post_paths.push(post_path);
        let label = format!("migrerun{n}");
        tags.push(v0_tag(
            &v0_post_uri_builder(author_id.clone(), post_id.clone()),
            &label,
        ));
        expected_ids.push((
            label.clone(),
            v1_tag(&v1_post_uri_builder(author_id.clone(), post_id), &label).create_id(),
        ));
    }
    tags.push(v0_tag(
        &v0_user_uri_builder(other_id.clone()),
        "migrerunuser",
    ));
    expected_ids.push((
        "migrerunuser".to_string(),
        v1_tag(&v1_user_uri_builder(other_id.clone()), "migrerunuser").create_id(),
    ));
    let url = format!("https://example.com/rerun/{author_id}");
    tags.push(v0_tag(&url, "migrerunurl"));
    // A URL target is kept, so its id is
    expected_ids.push(("migrerunurl".to_string(), tags[4].create_id()));
    for tag in &tags {
        test.put(&author_kp, &tag.hs_path(), tag).await?;
    }

    let author = pubky_id(&author_id)?;
    let files = v0_to_v1::files::FileMap::load(&author).await?;
    v0_to_v1::tags::migrate(&author, &files, 2).await?;
    let first = tag_edges_snapshot(&author_id).await?;
    assert_eq!(first.len(), tags.len());
    for (label, id, uri, _, _) in &first {
        let (_, expected_id) = expected_ids
            .iter()
            .find(|(expected_label, _)| expected_label == label)
            .expect("an expected id per label");
        assert_eq!(id, expected_id, "{label}");
        assert_eq!(uri, &v1_tag_uri(&author_id, id), "{label}");
    }

    v0_to_v1::tags::migrate(&author, &files, 2).await?;
    assert_eq!(tag_edges_snapshot(&author_id).await?, first);

    // Partly migrated, as a failed run leaves it: some edges back at `pubky.app`
    for tag in [&tags[1], &tags[3]] {
        reset_tag_edge(
            &author_id,
            &tag.label,
            &tag.create_id(),
            &v0_tag_uri_builder(author_id.clone(), tag.create_id()),
        )
        .await?;
    }
    assert_ne!(tag_edges_snapshot(&author_id).await?, first);
    v0_to_v1::tags::migrate(&author, &files, 2).await?;
    assert_eq!(tag_edges_snapshot(&author_id).await?, first);

    // The edges answer to their v1 ids now: handled at `v1`
    set_user_epoch(&author_id, "v1").await?;
    let event_handler = test.event_processor_runner.event_handler.clone();
    for (_, id, _, _, _) in &first {
        retrieve_and_handle_event_line(&v1_tag_del_line(&author_id, id), event_handler.clone())
            .await?;
    }
    assert!(tag_edges_snapshot(&author_id).await?.is_empty());
    assert!(!resource_exists_in_graph(&compute_resource_id(&url)).await?);
    set_user_epoch(&author_id, "v0").await?;
    for post_path in &post_paths {
        test.cleanup_post(&author_kp, post_path).await?;
    }
    test.cleanup_user(&other_kp).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}

/// A flip that gives a post tag a new id and then fails leaves the edge with its `pubky.app`
/// id beside the new one. A replay of the `pubky.app` tag file puts the edge back at that
/// file's address, keeping the new id; the retried tags step rewrites the edge to the same new
/// id and must keep the `pubky.app` id, so the `DEL` of that file still removes the tag when
/// it is handled at `v0` with no flip queued (the gate holds it while one is).
#[tokio_shared_rt::test(shared)]
async fn test_retried_flip_keeps_the_pubky_app_id_of_a_replayed_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let author_kp = Keypair::random();
    let author_id = test
        .create_user(
            &author_kp,
            &test_user("Watcher:Migration:Tags:Replayed:Author", "migration"),
        )
        .await?;
    let (post_id, post_path) = test
        .create_post(&author_kp, &short_post("Watcher:Migration:Tags:Replayed"))
        .await?;
    let label = "migreplayed";
    let v0 = v0_tag(
        &v0_post_uri_builder(author_id.clone(), post_id.clone()),
        label,
    );
    test.put(&author_kp, &v0.hs_path(), &v0).await?;
    let v1_id = v1_tag(
        &v1_post_uri_builder(author_id.clone(), post_id.clone()),
        label,
    )
    .create_id();
    assert_ne!(v0.create_id(), v1_id);

    // The flip's tags step commits; a later step fails, so the user stays `v0`
    let user = pubky_id(&author_id)?;
    let files = v0_to_v1::files::FileMap::load(&user).await?;
    v0_to_v1::tags::migrate(&user, &files, v0_to_v1::tags::BATCH_SIZE).await?;
    assert_eq!(
        find_tag_uri(&author_id, &v1_id).await,
        Some(v1_tag_uri(&author_id, &v1_id))
    );
    // A replay of the `pubky.app` file
    test.put(&author_kp, &v0.hs_path(), &v0).await?;
    assert_eq!(
        find_tag_uri(&author_id, &v1_id).await,
        Some(v0_tag_uri_builder(author_id.clone(), v0.create_id()))
    );
    // The retried tags step
    v0_to_v1::tags::migrate(&user, &files, v0_to_v1::tags::BATCH_SIZE).await?;
    assert_eq!(
        find_tag_uri(&author_id, &v1_id).await,
        Some(v1_tag_uri(&author_id, &v1_id))
    );

    test.del(&author_kp, &v0.hs_path()).await?;
    retrieve_and_handle_event_line(
        &format!(
            "DEL {}",
            v0_tag_uri_builder(author_id.clone(), v0.create_id())
        ),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "the pubky.app DEL must remove the tag"
    );

    test.cleanup_post(&author_kp, &post_path).await?;
    test.cleanup_user(&author_kp).await?;
    Ok(())
}
