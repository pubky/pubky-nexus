use super::posts::utils::v1_test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use futures::TryStreamExt;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{get_neo4j_graph, queries, RedisOps};
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use nexus_common::models::post::{
    KindFilter, PostCounts, PostDetails, PostKind, PostRelationships, StreamSource,
};
use nexus_common::models::resource::{references, view::ResourceView};
use nexus_common::models::user::UserCounts;
use nexus_common::types::{Pagination, StreamSorting};
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use nexus_common::utils::test_utils::{
    fail_references, pause_references, ReferenceFault, ReferencePause,
};
use nexus_watcher::EventProcessorError;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::{
    traits::{HasIdPath, HashId},
    PubkySocialTag,
};
use serde_json::json;
use std::time::Duration;

#[tokio_shared_rt::test(shared)]
async fn replies_embeds_and_collections_share_resources_and_replay_keys() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("External references", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/references/{author}");
    let second_target = format!("https://example.com/second/{author}");
    let rid = resource_id(&target);
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let tag = PubkySocialTag::new(
        target.replace("example.com", "EXAMPLE.com"),
        "shared".into(),
    );
    let tag_path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
    test.put(&key, &tag_path, &tag).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"reply and embed","parent":target,"embed":target}),
    )
    .await?;
    let collection_id = generate_post_id();
    let collection: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{collection_id}.json").parse()?;
    test.put(&key, &collection, &json!({"kind":"collection", "content":json!({"name":"Resources","items":[{"uri":target},{"uri":second_target}]}).to_string()})).await?;
    let view = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .unwrap();
    assert_eq!(
        (
            view.replies_count,
            view.embeds_count,
            view.collections_count,
            view.taggers_count
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Embed, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .replied
            .as_deref(),
        Some(target.as_str())
    );
    // Graph recovery must not turn a Resource reply into a root timeline post.
    PostDetails::remove_from_index_multiple_json(&[&[author.as_str(), id.as_str()]]).await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    // A tag deletion must preserve the node while posts still reference it.
    test.del(&key, &tag_path).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_some());
    let edit = generate_post_id();
    let collection_edit: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{edit}.json").parse()?;
    test.put(&key, &collection_edit, &json!({"kind":"collection", "content":json!({"name":"Changed","items":[{"uri":second_target}]}).to_string()})).await?;
    assert_eq!(
        ResourceView::get_by_id(&rid, None, None, None, None)
            .await?
            .unwrap()
            .collections_count,
        0
    );
    // The external reply parent is fixed, as a post parent is: an edit that moves or drops it
    // is refused, never retried, and changes nothing.
    let handler = test.event_processor_runner.event_handler.clone();
    let second_rid = resource_id(&second_target);
    let (_, _, edit_cursor) = ResourceChange::get_from_index(0, 0).await?;
    for (case, body) in [
        (
            "moved to another URL",
            json!({"kind":"note","content":"moved", "parent":second_target, "embed":target}),
        ),
        (
            "dropped",
            json!({"kind":"note","content":"root again", "embed":target}),
        ),
    ] {
        let version: ResourcePath =
            format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
        test.put(&key, &version, &body).await?;
        let err = retrieve_and_handle_event_line(
            &format!("PUT pubky://{author}{version}"),
            handler.clone(),
        )
        .await
        .expect_err(case);
        assert!(
            matches!(err, EventProcessorError::PostParentChanged(_)),
            "{case}: {err}"
        );
        assert_eq!(
            PostDetails::get_from_graph(&author, &id)
                .await?
                .unwrap()
                .0
                .content,
            "reply and embed",
            "{case}"
        );
        assert_eq!(
            references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
            [(author.clone(), id.clone())],
            "{case}"
        );
        assert!(
            references::posts(&second_rid, ResourceChangeKind::Reply, 0, 10)
                .await?
                .is_empty(),
            "{case}"
        );
        assert!(
            super::posts::utils::check_member_user_replies_timeline(&author, &id)
                .await?
                .is_some(),
            "{case}"
        );
        assert!(
            super::posts::utils::check_member_global_timeline_user_post(&author, &id)
                .await?
                .is_none(),
            "{case}"
        );
        assert!(
            super::posts::utils::check_member_total_engagement_user_posts(&[&author, &id])
                .await?
                .is_none(),
            "{case}"
        );
    }
    assert_eq!(
        ResourceChange::get_from_index(edit_cursor, 1000)
            .await?
            .0
            .lines
            .iter()
            .filter(|line| line.author == author)
            .count(),
        0,
        "a refused edit publishes nothing"
    );
    // Another spelling of the same URL is the same parent; the embed stays editable.
    let reply_edit_id = generate_post_id();
    let reply_edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{reply_edit_id}.json").parse()?;
    test.put(
        &key,
        &reply_edit,
        &json!({"kind":"note","content":"same parent, another spelling", "parent":target.replace("example.com", "EXAMPLE.com"), "embed":second_target}),
    )
    .await?;
    let relationships = PostRelationships::get_from_graph(&author, &id)
        .await?
        .unwrap();
    assert_eq!(relationships.replied.as_deref(), Some(target.as_str()));
    assert_eq!(
        relationships.embedded.as_deref(),
        Some(second_target.as_str())
    );
    assert_eq!(
        PostDetails::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .0
            .content,
        "same parent, another spelling"
    );
    assert!(references::posts(&rid, ResourceChangeKind::Embed, 0, 10)
        .await?
        .is_empty());
    assert_eq!(
        references::posts(&second_rid, ResourceChangeKind::Embed, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some()
    );
    assert!(
        super::posts::utils::check_member_total_engagement_user_posts(&[&author, &id])
            .await?
            .is_none()
    );
    test.del(&key, &reply_edit).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    test.del(&key, &collection_edit).await?;
    assert!(
        ResourceView::get_by_id(&resource_id(&second_target), None, None, None, None)
            .await?
            .is_none()
    );
    let page = ResourceChange::get_from_index(cursor, 1000).await?.0;
    let own: Vec<_> = page
        .lines
        .iter()
        .filter(|line| line.author == author)
        .collect();
    assert!(own.iter().any(|line| line.kind == ResourceChangeKind::Reply
        && line.op == ResourceChangeOp::Put
        && line.object_path == path.to_string()));
    let first_collection: Vec<_> = own
        .iter()
        .filter(|line| {
            line.kind == ResourceChangeKind::Collection
                && line.op == ResourceChangeOp::Put
                && line.object_path == collection.to_string()
        })
        .collect();
    assert_eq!(
        first_collection.len(),
        2,
        "one file names two distinct targets"
    );
    let mut consumer = std::collections::BTreeMap::new();
    for line in own {
        let key = (
            line.author.clone(),
            line.object_path.clone(),
            format!("{:?}", line.kind),
            line.resource_id.clone(),
        );
        match line.op {
            ResourceChangeOp::Put => {
                consumer.insert(key, line.uri.clone());
            }
            ResourceChangeOp::Del => {
                consumer.remove(&key);
            }
        }
    }
    assert!(
        consumer.is_empty(),
        "deletes balance every reference, including edited collection versions"
    );
    test.del(&key, &profile).await?;
    Ok(())
}

/// Takes `id`'s references back to how a post indexed before posts referenced Resources holds
/// them: no Resource edge, no reconciled revision, and the Redis lists of a root post.
async fn as_indexed_before_references(author: &str, id: &str) -> Result<()> {
    get_neo4j_graph()?
        .run(
            nexus_common::db::graph::Query::new(
                "test_unreconcile_post",
                "MATCH (:User {id: $author})-[:AUTHORED]->(p:Post {id: $id})
             OPTIONAL MATCH (p)-[e:REPLIED|EMBEDS|COLLECTED]->(r:Resource)
             DELETE e
             WITH p, collect(DISTINCT r) AS targets
             REMOVE p.resource_ref_revision, p.resource_ref_outbox, p.resource_ref_retracted
             WITH targets UNWIND targets AS r
             WITH r WHERE NOT EXISTS { (r)--() } DELETE r",
            )
            .param("author", author)
            .param("id", id),
        )
        .await?;
    PostDetails::reindex(author, id).await?;
    PostRelationships::reindex(author, id).await?;
    PostCounts::reindex(author, id).await?;
    Ok(())
}

/// Whether the post is in the root lists (global and author timelines, engagement ranking) and
/// whether it is in the author's replies.
async fn root_lists_and_replies(author: &str, id: &str) -> Result<(bool, bool, bool, bool)> {
    use super::posts::utils::*;
    Ok((
        check_member_global_timeline_user_post(author, id)
            .await?
            .is_some(),
        check_member_user_post_timeline(author, id).await?.is_some(),
        check_member_total_engagement_user_posts(&[author, id])
            .await?
            .is_some(),
        check_member_user_replies_timeline(author, id)
            .await?
            .is_some(),
    ))
}

const ROOT: (bool, bool, bool, bool) = (true, true, true, false);
const REPLY: (bool, bool, bool, bool) = (false, false, false, true);

/// A post's external reply parent is fixed from its first PUT on, also when it has none: a root
/// post edited into a reply to a URL is refused and changes nothing. A post indexed before posts
/// referenced Resources has no Resource reply edge yet, and its replay gains the one it names:
/// it leaves the root lists for the author's replies. From then on its parent is fixed too.
#[tokio_shared_rt::test(shared)]
async fn an_external_reply_parent_is_fixed_once_reconciled() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Fixed external parent", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let handler = test.event_processor_runner.event_handler.clone();
    let target = format!("https://example.com/fixed-parent/{author}");
    let rid = resource_id(&target);

    // Root edited into a reply to a URL
    let root_id = generate_post_id();
    let root: ResourcePath = format!("/pub/social/v1/posts/{root_id}/{root_id}.json").parse()?;
    test.put(&key, &root, &json!({"kind":"note","content":"a root post"}))
        .await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let version: ResourcePath =
        format!("/pub/social/v1/posts/{root_id}/{}.json", generate_post_id()).parse()?;
    test.put(
        &key,
        &version,
        &json!({"kind":"note","content":"now a reply","parent":target}),
    )
    .await?;
    let err =
        retrieve_and_handle_event_line(&format!("PUT pubky://{author}{version}"), handler.clone())
            .await
            .expect_err("a root post edited into a reply to a URL is refused");
    assert!(
        matches!(err, EventProcessorError::PostParentChanged(_)),
        "{err}"
    );
    assert_eq!(
        PostDetails::get_from_graph(&author, &root_id)
            .await?
            .unwrap()
            .0
            .content,
        "a root post"
    );
    assert!(!PostDetails::is_reply_in_graph(&author, &root_id).await?);
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    assert_eq!(root_lists_and_replies(&author, &root_id).await?, ROOT);
    let own_since = |cursor| {
        let author = author.clone();
        async move {
            anyhow::Ok(
                ResourceChange::get_from_index(cursor, 1000)
                    .await?
                    .0
                    .lines
                    .into_iter()
                    .filter(|line| line.author == author)
                    .map(|line| (line.op, line.resource_id))
                    .collect::<Vec<_>>(),
            )
        }
    };
    assert_eq!(
        own_since(cursor).await?,
        [],
        "a refused edit publishes nothing"
    );

    // A reply to the URL indexed before posts referenced Resources: its replay gains the edge
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let envelope = json!({"kind":"note","content":"an old reply","parent":target});
    test.put(&key, &path, &envelope).await?;
    as_indexed_before_references(&author, &id).await?;
    assert!(!PostDetails::is_reply_in_graph(&author, &id).await?);
    assert_eq!(root_lists_and_replies(&author, &id).await?, ROOT);
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone()).await?;
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())],
        "the replay gains its Resource reply edge"
    );
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);
    // From now on its parent is fixed
    let moved: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
    test.put(
        &key,
        &moved,
        &json!({"kind":"note","content":"moved","parent":format!("{target}/elsewhere")}),
    )
    .await?;
    let err =
        retrieve_and_handle_event_line(&format!("PUT pubky://{author}{moved}"), handler.clone())
            .await
            .expect_err("a reconciled post's URL parent is fixed");
    assert!(
        matches!(err, EventProcessorError::PostParentChanged(_)),
        "{err}"
    );
    assert_eq!(
        own_since(cursor).await?,
        [(ResourceChangeOp::Put, rid.clone())],
        "the replay publishes the reference it gains, the refused edit nothing"
    );

    test.del(&key, &path).await?;
    test.del(&key, &root).await?;
    test.del(&key, &profile).await?;
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());
    Ok(())
}

/// A new reply to a URL is a reply from its first write on: it never enters the root
/// timelines or the ranking, also when its reference write fails, and its retry finishes it.
#[tokio_shared_rt::test(shared)]
async fn a_new_reply_to_a_url_stays_out_of_the_root_lists_when_its_references_fail() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reply before its references", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let target = format!("https://example.com/reply-before-references/{author}");
    let rid = resource_id(&target);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"a reply","parent":target}),
    )
    .await?;

    fail_references(ReferenceFault::BeforeStage, &author, true);
    let failed =
        retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone())
            .await;
    fail_references(ReferenceFault::BeforeStage, &author, false);
    assert!(failed.is_err(), "the injected failure");
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    assert!(
        !PostDetails::is_reply_in_graph(&author, &id).await?,
        "no edge yet"
    );
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);

    retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone()).await?;
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);

    for line in [
        format!("DEL pubky://{author}{path}"),
        format!("DEL pubky://{author}{profile}"),
    ] {
        retrieve_and_handle_event_line(&line, handler.clone()).await?;
    }
    Ok(())
}

/// A PUT that stops after its reference write leaves Redis as the post was before it. Its
/// retry reads back what the graph says now, and a DEL that comes first cleans up whatever the
/// lists hold, whichever way the post's reply status went: a post indexed before posts
/// referenced Resources gains its Resource reply edge (a root post becomes a reply), and one
/// that had such an edge without a reconciled revision may drop it (a reply becomes a root
/// post).
#[tokio_shared_rt::test(shared)]
async fn a_put_that_stops_after_its_references_is_finished_by_its_retry_or_its_delete() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Stopped after references", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let handle = |line: String| {
        let handler = handler.clone();
        async move { retrieve_and_handle_event_line(&line, handler).await }
    };
    let target = format!("https://example.com/stopped-after-references/{author}");
    let mut posts = Vec::new();
    let mut new_reply = async |content: &str| -> Result<(String, ResourcePath)> {
        let id = generate_post_id();
        let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
        test.put(
            &key,
            &path,
            &json!({"kind":"note","content":content,"parent":target}),
        )
        .await?;
        retrieve_and_handle_event_line(
            &format!("PUT pubky://{author}{path}"),
            test.event_processor_runner.event_handler.clone(),
        )
        .await?;
        Ok((id, path))
    };
    let (retried, retried_path) = new_reply("root to reply, retried").await?;
    let (deleted, deleted_path) = new_reply("root to reply, deleted").await?;
    let (to_root, to_root_path) = new_reply("reply to root").await?;
    posts.push(retried_path.clone());
    posts.push(to_root_path.clone());

    // Root to reply: each replay stops after the edge write, with Redis still a root post's
    for id in [&retried, &deleted] {
        as_indexed_before_references(&author, id).await?;
    }
    fail_references(ReferenceFault::AfterSync, &author, true);
    for path in [&retried_path, &deleted_path] {
        assert!(handle(format!("PUT pubky://{author}{path}")).await.is_err());
    }
    fail_references(ReferenceFault::AfterSync, &author, false);
    for id in [&retried, &deleted] {
        assert!(
            PostDetails::is_reply_in_graph(&author, id).await?,
            "the edge"
        );
        assert_eq!(root_lists_and_replies(&author, id).await?, ROOT, "stale");
    }
    // The retry puts it in the reply lists
    handle(format!("PUT pubky://{author}{retried_path}")).await?;
    assert_eq!(root_lists_and_replies(&author, &retried).await?, REPLY);
    // The DEL before any retry leaves nothing behind
    handle(format!("DEL pubky://{author}{deleted_path}")).await?;
    assert!(PostDetails::get_from_graph(&author, &deleted)
        .await?
        .is_none());
    assert_eq!(
        root_lists_and_replies(&author, &deleted).await?,
        (false, false, false, false),
        "the delete takes the post out of the ranking it was still in"
    );

    // Reply to root: a Resource reply edge without a reconciled revision accepts an edit that
    // drops the parent; the edit stops after its edge write, and its retry finds no edge left
    get_neo4j_graph()?
        .run(
            nexus_common::db::graph::Query::new(
                "test_forget_revision",
                "MATCH (:User {id: $author})-[:AUTHORED]->(p:Post {id: $id})
                 REMOVE p.resource_ref_revision",
            )
            .param("author", author.as_str())
            .param("id", to_root.as_str()),
        )
        .await?;
    let edit: ResourcePath =
        format!("/pub/social/v1/posts/{to_root}/{}.json", generate_post_id()).parse()?;
    test.put(
        &key,
        &edit,
        &json!({"kind":"note","content":"a root post now"}),
    )
    .await?;
    fail_references(ReferenceFault::AfterSync, &author, true);
    assert!(handle(format!("PUT pubky://{author}{edit}")).await.is_err());
    fail_references(ReferenceFault::AfterSync, &author, false);
    assert!(!PostDetails::is_reply_in_graph(&author, &to_root).await?);
    assert_eq!(
        root_lists_and_replies(&author, &to_root).await?,
        REPLY,
        "stale"
    );
    handle(format!("PUT pubky://{author}{edit}")).await?;
    assert_eq!(
        root_lists_and_replies(&author, &to_root).await?,
        ROOT,
        "the retry lists and ranks the root post"
    );

    for path in posts.iter().chain([&profile]) {
        handle(format!("DEL pubky://{author}{path}")).await?;
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn graph_commit_before_append_replays_put_and_a_delete_retry_replays_del() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference crash replay", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/crash/{author}");
    let rid = resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let envelope = json!({"kind":"note","content":"first","parent":target});
    test.put(&key, &path, &envelope).await?;
    test = test.remove_event_processing().await;
    let edit = generate_post_id();
    let version: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(&key, &version, &envelope).await?;
    let line = references::reference(
        &author,
        &format!("pubky://{author}{version}"),
        &target,
        ResourceChangeKind::Reply,
    )?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    references::stage(&post, vec![line]).await?; // committed graph and outbox, no append
    assert!(ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .all(|line| line.author != author));
    let handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&format!("PUT pubky://{author}{version}"), handler.clone())
        .await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let puts: Vec<_> = lines
        .iter()
        .filter(|line| {
            line.author == author
                && line.op == ResourceChangeOp::Put
                && line.object_path == version.to_string()
        })
        .collect();
    assert_eq!(
        puts.len(),
        1,
        "the handler's retry delivers the outbox, and its own write, which finds the edge, adds nothing"
    );
    assert_eq!(
        (puts[0].resource_id.as_str(), puts[0].kind),
        (rid.as_str(), ResourceChangeKind::Reply)
    );
    // A DEL that appended its dels and stopped before the graph delete: the post stays
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    references::retract(&author, &id).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    // Its retry appends them again and deletes the post; a replay finds nothing to retract
    retrieve_and_handle_event_line(&format!("DEL pubky://{author}{version}"), handler.clone())
        .await?;
    retrieve_and_handle_event_line(&format!("DEL pubky://{author}{version}"), handler).await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let deletes: Vec<_> = lines
        .iter()
        .filter(|line| line.author == author && line.op == ResourceChangeOp::Del)
        .collect();
    assert_eq!(deletes.len(), 2, "the retry's duplicate, and no more");
    assert!(deletes
        .iter()
        .all(|line| line.object_path == version.to_string()));
    assert!(ResourceView::get_by_id(
        &resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0),
        None,
        None,
        None,
        None
    )
    .await?
    .is_none());
    test.del(&key, &profile).await?;
    Ok(())
}

/// A `social/v1` post DEL that finds no post (one never indexed, or one another version's DEL
/// already deleted) is acknowledged and, as for a `pubky.app` post, its line is not stored in
/// `Events`. A DEL that deletes a post stores its line.
#[tokio_shared_rt::test(shared)]
async fn a_social_post_del_that_finds_no_post_is_acknowledged_and_not_stored() -> Result<()> {
    use nexus_common::models::event::EventLine;

    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Unknown post DEL", ""))
        .await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let stored_since = |since: u64| async move {
        anyhow::Ok(EventLine::get_from_index(Some(since), usize::MAX).await?.0)
    };
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
    let del = |path: &ResourcePath| format!("DEL pubky://{author}{path}");

    // Never indexed
    let (_, since) = EventLine::get_from_index(None, usize::MAX).await?;
    retrieve_and_handle_event_line(&del(&path), handler.clone()).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(!stored_since(since).await?.contains(&del(&path)));

    // Indexed: the first version DEL deletes the post and is stored, another one finds nothing
    test.put(&key, &path, &json!({"kind":"note","content":"to delete"}))
        .await?;
    retrieve_and_handle_event_line(&format!("PUT pubky://{author}{path}"), handler.clone()).await?;
    let (_, since) = EventLine::get_from_index(None, usize::MAX).await?;
    retrieve_and_handle_event_line(&del(&path), handler.clone()).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    retrieve_and_handle_event_line(&del(&edit), handler.clone()).await?;
    let stored = stored_since(since).await?;
    assert!(stored.contains(&del(&path)), "the deleting DEL is stored");
    assert!(
        !stored.contains(&del(&edit)),
        "the DEL that found nothing is not"
    );

    retrieve_and_handle_event_line(&format!("DEL pubky://{author}{profile}"), handler).await?;
    Ok(())
}

/// A DEL that appended its references' dels and stopped before the graph delete leaves the
/// post. A PUT of the post before the DEL's retry publishes its references again, so the
/// consumer ends with the reference live, as the graph holds it.
#[tokio_shared_rt::test(shared)]
async fn put_after_an_interrupted_delete_publishes_its_references_again() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference retraction on put", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/retraction-on-put/{author}");
    let rid = resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"retracted","parent":target}),
    )
    .await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    // The DEL's append, without its graph delete
    references::retract(&author, &id).await?;

    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    let ops: Vec<_> = ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .filter(|line| line.author == author && line.resource_id == rid)
        .map(|line| line.op)
        .collect();
    assert_eq!(ops, [ResourceChangeOp::Del, ResourceChangeOp::Put]);
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    test.del(&key, &path).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// Handles `line` on its own task, as the live runners and the retry processor do.
fn spawn_event_line(
    line: String,
    handler: std::sync::Arc<dyn nexus_watcher::events::EventHandler>,
) -> tokio::task::JoinHandle<Result<(), nexus_watcher::EventProcessorError>> {
    tokio::spawn(async move { retrieve_and_handle_event_line(&line, handler).await })
}

/// Live events and retries of one post run in parallel. A PUT can read the outbox a lost append
/// left and pause before appending it, while a DEL of the post publishes that outbox, deletes the
/// post and retracts its references: the PUT's append after that would bring the deleted
/// reference back for feed readers. The post lock orders the two, so the feed ends with the
/// retraction. The held PUT is a newer version that also embeds another target, so the DEL must
/// read the post's references under the lock too: one that read them before taking it would
/// retract the reply and leave the newer version's embed live.
#[tokio_shared_rt::test(shared)]
async fn concurrent_delete_is_not_overtaken_by_an_outbox_append() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference delete race", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/delete-race/{author}");
    let rid = resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0);
    let new_target = format!("https://example.com/delete-race-newer/{author}");
    let new_rid = resource_id(&normalize_uri(&new_target).map_err(anyhow::Error::msg)?.0);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"raced","parent":target}),
    )
    .await?;
    // A newer version, on the homeserver but not handled yet
    test = test.remove_event_processing().await;
    let edit = generate_post_id();
    let newer: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(
        &key,
        &newer,
        &json!({"kind":"note","content":"raced again","parent":target,"embed":new_target}),
    )
    .await?;
    // A committed graph write whose feed append was lost leaves its outbox on the post: here
    // the reply moved to the newer version's file.
    let line = references::reference(
        &author,
        &format!("pubky://{author}{newer}"),
        &target,
        ResourceChangeKind::Reply,
    )?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    references::stage(&post, vec![line]).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Flush, &author);
    let put = spawn_event_line(format!("PUT pubky://{author}{newer}"), handler.clone());
    // The PUT read the outbox, and has not appended it nor staged its own references.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the PUT never reached the outbox append"))??;
    let mut del = spawn_event_line(format!("DEL pubky://{author}{newer}"), handler);
    // Unordered, the DEL runs to its end while the PUT is held.
    let early = tokio::time::timeout(Duration::from_secs(3), &mut del).await;
    // The DEL waits for the lock before it reads the post's references: it appends nothing
    let appended_early = ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .any(|line| line.author == author && line.op == ResourceChangeOp::Del);
    let _ = release.send(());
    assert!(
        early.is_err(),
        "the DEL must wait while the PUT holds the lock"
    );
    assert!(!appended_early, "the waiting DEL appended its dels");
    put.await??;
    del.await??;

    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    for (rid, name) in [
        (&rid, "the reply's target"),
        (&new_rid, "the newer embed's target"),
    ] {
        let ops: Vec<_> = lines
            .iter()
            .filter(|line| line.author == author && &line.resource_id == rid)
            .map(|line| line.op == ResourceChangeOp::Put)
            .collect();
        assert_eq!(
            ops.last(),
            Some(&false),
            "a retraction is {name}'s last change (true = put): {ops:?}"
        );
    }
    for rid in [&rid, &new_rid] {
        assert!(ResourceView::get_by_id(rid, None, None, None, None)
            .await?
            .is_none());
    }
    // Event processing is off: handle the profile's DEL here, so the user does not stay behind
    test.del(&key, &profile).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{profile}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    Ok(())
}

/// An older version's PUT that wrote the post and paused before its references must not, once
/// a newer version's PUT finished, replace the newer version's references with its own: both
/// versions have the same content and kind, so the post write tells them apart. The post lock
/// orders the two handlers, and the post keeps the references of the version handled last. The
/// versions embed different targets, as an edit may change an embed (not a reply parent).
#[tokio_shared_rt::test(shared)]
async fn older_version_does_not_replace_a_newer_versions_references() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference edit race", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let old_target = format!("https://example.com/older/{author}");
    let new_target = format!("https://example.com/newer/{author}");
    let id = generate_post_id();
    let older: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &older,
        &json!({"kind":"note","content":"same words","embed":old_target}),
    )
    .await?;
    let edit = generate_post_id();
    let newer: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(
        &key,
        &newer,
        &json!({"kind":"note","content":"same words","embed":new_target}),
    )
    .await?;

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Stage, &author);
    let old_put = spawn_event_line(format!("PUT pubky://{author}{older}"), handler.clone());
    // The older version's post write is done, its references are not.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the older PUT never reached its references"))??;
    let mut new_put = spawn_event_line(format!("PUT pubky://{author}{newer}"), handler);
    // Unordered, the newer PUT runs to its end while the older one is held.
    let early = tokio::time::timeout(Duration::from_secs(3), &mut new_put).await;
    let _ = release.send(());
    old_put.await??;
    match early {
        Ok(joined) => joined??,
        Err(_) => new_put.await??,
    }

    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .embedded
            .as_deref(),
        Some(new_target.as_str()),
        "the version handled last keeps its references"
    );
    assert!(
        references::posts(&resource_id(&old_target), ResourceChangeKind::Embed, 0, 10)
            .await?
            .is_empty()
    );
    test.del(&key, &newer).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// A newer version's PUT waits for the post's lock before it reads or writes anything: while an
/// older version's PUT holds it, paused after its post write and before its references, the
/// post keeps the older content. Released, the older PUT finishes and the newer one runs last:
/// the post has the newer content and the newer version's (no) embed, and stays a ranked root
/// post.
#[tokio_shared_rt::test(shared)]
async fn newer_version_waits_for_the_post_lock_before_its_post_write() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference write order", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let old_target = format!("https://example.com/older-write/{author}");
    let id = generate_post_id();
    let older: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &older,
        &json!({"kind":"note","content":"older words","embed":old_target}),
    )
    .await?;
    let edit = generate_post_id();
    let newer: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(
        &key,
        &newer,
        &json!({"kind":"note","content":"newer words"}),
    )
    .await?;
    let ranked = || async {
        super::posts::utils::check_member_total_engagement_user_posts(&[&author, &id]).await
    };
    assert!(ranked().await?.is_some(), "a root post is ranked");
    let content = || async {
        anyhow::Ok(
            PostDetails::get_from_graph(&author, &id)
                .await?
                .expect("the post")
                .0
                .content,
        )
    };

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Stage, &author);
    let old_put = spawn_event_line(format!("PUT pubky://{author}{older}"), handler.clone());
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the older PUT never reached its references"))??;
    assert_eq!(content().await?, "older words");
    let mut new_put = spawn_event_line(format!("PUT pubky://{author}{newer}"), handler);
    let early = tokio::time::timeout(Duration::from_secs(3), &mut new_put).await;
    let held = content().await?;
    let _ = release.send(());
    assert!(
        early.is_err(),
        "the newer PUT must wait while the older holds the lock"
    );
    assert_eq!(held, "older words", "the waiting PUT wrote the post");
    old_put.await??;
    new_put.await??;

    assert_eq!(content().await?, "newer words");
    assert_eq!(
        PostRelationships::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .embedded,
        None
    );
    assert!(ranked().await?.is_some(), "the root post stays ranked");
    assert!(
        references::posts(&resource_id(&old_target), ResourceChangeKind::Embed, 0, 10)
            .await?
            .is_empty()
    );
    test.del(&key, &newer).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// A tombstoned external reply keeps its Resource reply edge, as every tombstone keeps its
/// edges, and its reference is retracted: one del, when it becomes a tombstone. It stays out of
/// the root timelines and in the author's replies and reply count, also after its cache is
/// lost, and the Resource neither lists nor counts it. Its later hard delete publishes no second
/// del, and removes the Resource with the last edge.
#[tokio_shared_rt::test(shared)]
async fn tombstones_keep_external_edges_and_retract_them_once() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference tombstone", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let target = format!("https://example.com/tombstone/{author}");
    let (normalized, _) = normalize_uri(&target).map_err(anyhow::Error::msg)?;
    let rid = resource_id(&normalized);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"resource reply","parent":target}),
    )
    .await?;
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    let ours = |lines: Vec<ResourceChange>| -> Vec<ResourceChange> {
        lines
            .into_iter()
            .filter(|line| line.author == author && line.resource_id == rid)
            .collect()
    };

    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    let lines = ours(ResourceChange::get_from_index(cursor, 1000).await?.0.lines);
    assert_eq!(
        lines.iter().map(|line| line.op).collect::<Vec<_>>(),
        [ResourceChangeOp::Del],
        "the tombstone retracts its reference once"
    );
    // The edge stays, off the Resource's lists and counts
    assert!(PostDetails::is_reply_in_graph(&author, &id).await?);
    assert_eq!(
        PostRelationships::get_by_id(&author, &id)
            .await?
            .unwrap()
            .replied,
        Some(normalized.clone())
    );
    let view = ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .expect("the tombstone's edge keeps the Resource");
    assert_eq!(view.replies_count, 0);
    assert!(references::posts(&rid, ResourceChangeKind::Reply, 0, 10)
        .await?
        .is_empty());
    // Out of the root timelines and in the author's replies, after cache loss too
    PostDetails::remove_from_index_multiple_json(&[&[author.as_str(), id.as_str()]]).await?;
    PostDetails::reindex(&author, &id).await?;
    assert!(
        super::posts::utils::check_member_global_timeline_user_post(&author, &id)
            .await?
            .is_none()
    );
    assert!(
        super::posts::utils::check_member_user_post_timeline(&author, &id)
            .await?
            .is_none()
    );
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some()
    );
    assert_eq!(
        UserCounts::get_by_id(&author).await?.unwrap().replies,
        2,
        "the tombstone and the child are both replies"
    );

    // The child goes, and the replayed DEL hard-deletes the tombstone
    test.del(&key, &child).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    let lines = ours(ResourceChange::get_from_index(cursor, 1000).await?.0.lines);
    assert_eq!(
        lines.iter().map(|line| line.op).collect::<Vec<_>>(),
        [ResourceChangeOp::Del],
        "the hard delete of a tombstone publishes no second del"
    );
    assert!(
        ResourceView::get_by_id(&rid, None, None, None, None)
            .await?
            .is_none(),
        "the Resource goes with its last edge"
    );
    test.del(&key, &profile).await?;
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn hard_deleted_external_reply_leaves_author_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Hard delete external reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/hard-delete/{author}")})).await?;
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some()
    );
    test.del(&key, &path).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_none()
    );
    test.del(&key, &profile).await?;
    Ok(())
}

/// A tombstoned external reply keeps its place in the author's replies. The DEL that later
/// hard-deletes it, once the child that kept the tombstone is gone (a replayed DEL, or the DEL
/// of another of its version files), must take it out of the author's replies too.
#[tokio_shared_rt::test(shared)]
async fn hard_delete_of_a_tombstoned_external_reply_leaves_author_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstoned external reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/tombstoned-reply/{author}")})).await?;
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_some(),
        "a tombstone stays in the author's replies"
    );

    test.del(&key, &child).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(
        super::posts::utils::check_member_user_replies_timeline(&author, &id)
            .await?
            .is_none(),
        "a hard-deleted tombstone stays in the author's replies"
    );
    test.del(&key, &profile).await?;
    Ok(())
}

/// A tombstoned external reply keeps its Resource reply edge and is still a reply, as the
/// author's replies set keeps it. The graph reads must classify it so too: a kind-filtered author
/// stream lists root posts only, the ranking refresh's entries and the post's counts call it a
/// reply, and the author's reply count keeps it.
#[tokio_shared_rt::test(shared)]
async fn tombstoned_external_reply_stays_out_of_root_graph_reads() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstone root reads", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let root_id = generate_post_id();
    let root: ResourcePath = format!("/pub/social/v1/posts/{root_id}/{root_id}.json").parse()?;
    test.put(&key, &root, &json!({"kind":"note","content":"a root post"}))
        .await?;
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note", "content":"reply", "parent":format!("https://example.com/tombstone-root-reads/{author}")})).await?;
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the parent tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;
    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);

    let stream = queries::get::post_stream(
        StreamSource::Author {
            author_id: author.clone(),
        },
        StreamSorting::Timeline,
        SortOrder::Descending,
        &None,
        Pagination::default(),
        Some(KindFilter::Kind(PostKind::Short)),
        None,
    )?;
    let mut rows = get_neo4j_graph()?.execute(stream).await?;
    let mut streamed = Vec::new();
    while let Some(row) = rows.try_next().await? {
        streamed.push(row.get::<String>("post_id")?);
    }
    assert_eq!(streamed, std::slice::from_ref(&root_id), "root posts only");

    let mut rows = get_neo4j_graph()?
        .execute(queries::get::author_post_entries(
            &author,
            (i64::MIN, ""),
            100,
        ))
        .await?;
    let mut entries = std::collections::BTreeMap::new();
    while let Some(row) = rows.try_next().await? {
        entries.insert(row.get::<String>("post_id")?, row.get::<bool>("is_reply")?);
    }
    assert_eq!(
        entries,
        [(root_id, false), (id.clone(), true), (child_id, true)].into(),
        "the ranking refresh keeps the tombstone out of the root sets"
    );
    assert!(PostCounts::get_from_graph(&author, &id).await?.unwrap().1);
    assert_eq!(
        UserCounts::get_by_id(&author).await?.unwrap().replies,
        2,
        "the tombstone and the child are both replies"
    );

    // The reply file is already deleted: the tombstone stays, as in the tests above.
    test.del(&key, &child).await?;
    test.del(&key, &root).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// A native reply that also embeds an external URI, tombstoned because a child replies to it,
/// keeps its cached relationships: they are the gate its later hard delete reads before it takes
/// the reply's engagement off the parent, and the hard delete must bring the parent's engagement
/// back down.
#[tokio_shared_rt::test(shared)]
async fn tombstoned_reply_with_an_external_embed_keeps_its_hard_delete_gate() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstoned embedding reply", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let root_id = generate_post_id();
    let root: ResourcePath = format!("/pub/social/v1/posts/{root_id}/{root_id}.json").parse()?;
    test.put(&key, &root, &json!({"kind":"note","content":"root"}))
        .await?;
    let root_key = [author.as_str(), root_id.as_str()];
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &json!({"kind":"note","content":"reply with an embed","parent":format!("pubky://{author}/pub/social/v1/posts/{root_id}"),"embed":format!("https://example.com/embedding-reply/{author}")})).await?;
    assert_eq!(
        super::posts::utils::check_member_total_engagement_user_posts(&root_key).await?,
        Some(1),
        "parent engagement = 1 after reply creation"
    );
    let child_id = generate_post_id();
    let child: ResourcePath = format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
    test.put(&key, &child, &json!({"kind":"note","content":"keep the reply tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{id}")})).await?;

    test.del(&key, &path).await?;
    assert!(PostDetails::get_by_id(&author, &id).await?.unwrap().deleted);
    assert!(
        PostRelationships::get_from_index(&author, &id)
            .await?
            .is_some(),
        "the tombstone's cached relationships are its hard delete's gate"
    );

    test.del(&key, &child).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert_eq!(
        super::posts::utils::check_member_total_engagement_user_posts(&root_key).await?,
        Some(0),
        "the hard delete takes the reply's engagement off its parent"
    );
    test.del(&key, &root).await?;
    test.del(&key, &profile).await?;
    Ok(())
}

/// Only external targets are Resources. A reply, an embed or a collection item naming a
/// bookmark, a private path or another object Nexus never references is indexed, but makes no
/// Resource, no reference edge, no feed line and no count.
#[tokio_shared_rt::test(shared)]
async fn bookmarks_and_private_paths_never_become_resources() -> Result<()> {
    use super::posts::utils::test_user;
    use pubky_social_specs::legacy_v0::{PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind};

    async fn authored_since(authors: &[&str], mut cursor: u64) -> Result<Vec<ResourceChange>> {
        let mut lines = Vec::new();
        loop {
            let (page, _, _) = ResourceChange::get_from_index(cursor, 1000).await?;
            if page.lines.is_empty() {
                return Ok(lines);
            }
            cursor = page.next_cursor;
            lines.extend(
                page.lines
                    .into_iter()
                    .filter(|line| authors.contains(&line.author.as_str())),
            );
        }
    }

    let mut test = WatcherTest::setup(None).await?;
    let (_, _, start) = ResourceChange::get_from_index(0, 0).await?;

    // A legacy author: a v0 post takes any URL as its parent or embed
    let legacy = Keypair::random();
    let legacy_id = test
        .create_user(&legacy, &test_user("Private targets", "v0"))
        .await?;
    let legacy_targets = [
        format!("pubky://{legacy_id}/pub/pubky.app/bookmarks/8Z8CWH8NVYQY39ZEBFGKQWWEKG"),
        format!("pubky://{legacy_id}/priv/social/v1/bookmarks/aHR0cHM6Ly9leGFtcGxlLmNvbS8.json"),
        format!("pubky://{legacy_id}/priv/notes.app/drafts/0033SSE3B1FQ0"),
    ];
    let mut legacy_posts = Vec::new();
    for target in &legacy_targets {
        for (parent, embed) in [(Some(target.clone()), None), (None, Some(target.clone()))] {
            let post = PubkyAppPost {
                content: format!("references {target}"),
                kind: PubkyAppPostKind::Short,
                parent,
                embed: embed.map(|uri| PubkyAppPostEmbed {
                    kind: PubkyAppPostKind::Short,
                    uri,
                }),
                attachments: None,
                lock: None,
            };
            legacy_posts.push(test.create_post(&legacy, &post).await?);
        }
    }

    // A social v1 author: a public post references public objects only, so a legacy bookmark
    let social = Keypair::random();
    test.register_user(&social).await?;
    let social_id = social.public_key().to_z32();
    let profile = test
        .put_v1_profile(&social, &v1_test_user("Bookmark targets", ""))
        .await?;
    super::utils::watcher::rank_user(&social_id).await?;
    let bookmark =
        format!("pubky://{social_id}/pub/pubky.app/bookmarks/8Z8CWH8NVYQY39ZEBFGKQWWEKG");
    let mut social_posts = Vec::new();
    for body in [
        json!({"kind":"note","content":"reply to a bookmark","parent":bookmark}),
        json!({"kind":"note","content":"embed of a bookmark","embed":bookmark}),
        json!({"kind":"collection","content":json!({"name":"Bookmarks","items":[{"uri":bookmark}]}).to_string()}),
    ] {
        let id = generate_post_id();
        let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
        test.put(&social, &path, &body).await?;
        social_posts.push((id, path));
    }

    for (id, _) in &legacy_posts {
        assert!(
            PostDetails::get_from_graph(&legacy_id, id).await?.is_some(),
            "{id} must be indexed"
        );
    }
    for (id, _) in &social_posts {
        assert!(
            PostDetails::get_from_graph(&social_id, id).await?.is_some(),
            "{id} must be indexed"
        );
    }
    // A post whose parent makes no Resource is a root post everywhere: in the engagement
    // ranking too, as a graph rebuild puts it
    let posts = legacy_posts
        .iter()
        .map(|(id, _)| (&legacy_id, id))
        .chain(social_posts.iter().map(|(id, _)| (&social_id, id)));
    for (author, id) in posts {
        assert!(
            super::posts::utils::check_member_total_engagement_user_posts(&[author, id])
                .await?
                .is_some(),
            "{id} must be ranked as a root post"
        );
    }
    for target in legacy_targets.iter().chain([&bookmark]) {
        let (uri, _) = normalize_uri(target).map_err(anyhow::Error::msg)?;
        assert!(
            ResourceView::get_by_id(&resource_id(&uri), None, None, None, None)
                .await?
                .is_none(),
            "{target} must not be a Resource"
        );
    }
    let lines = authored_since(&[&legacy_id, &social_id], start).await?;
    assert!(lines.is_empty(), "no reference reaches the feed: {lines:?}");

    for (_, path) in &social_posts {
        test.del(&social, path).await?;
    }
    test.del(&social, &profile).await?;
    for (_, path) in &legacy_posts {
        test.cleanup_post(&legacy, path).await?;
    }
    test.cleanup_user(&legacy).await?;
    Ok(())
}
