use super::posts::utils::v1_test_user;
use super::utils::watcher::{generate_post_id, retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use futures::TryStreamExt;
use nexus_common::db::kv::SortOrder;
use nexus_common::db::{fetch_row_from_graph, get_neo4j_graph, queries, RedisOps};
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
    fail_feed_dels_after_append, fail_references, pause_references, ReferenceFault, ReferencePause,
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
        &test.homeserver_id,
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    assert_eq!(
        root_lists_and_replies(&author, &id).await?,
        REPLY,
        "the recovered reply stays out of the root timelines and the ranking"
    );
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
            &test.homeserver_id,
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
    // A line names the post's address, whichever version file made it
    assert!(own.iter().any(|line| line.kind == ResourceChangeKind::Reply
        && line.op == ResourceChangeOp::Put
        && line.object_path == format!("/pub/social/v1/posts/{id}")));
    let first_collection: Vec<_> = own
        .iter()
        .filter(|line| {
            line.kind == ResourceChangeKind::Collection
                && line.op == ResourceChangeOp::Put
                && line.object_path == format!("/pub/social/v1/posts/{collection_id}")
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
    let err = retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{version}"),
        &test.homeserver_id,
        handler.clone(),
    )
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
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
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
    let err = retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{moved}"),
        &test.homeserver_id,
        handler.clone(),
    )
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
    let failed = retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await;
    fail_references(ReferenceFault::BeforeStage, &author, false);
    assert!(failed.is_err(), "the injected failure");
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    assert!(
        !PostDetails::is_reply_in_graph(&author, &id).await?,
        "no edge yet"
    );
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);

    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    assert_eq!(
        references::posts(&rid, ResourceChangeKind::Reply, 0, 10).await?,
        [(author.clone(), id.clone())]
    );
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);

    for line in [
        format!("DEL pubky://{author}{path}"),
        format!("DEL pubky://{author}{profile}"),
    ] {
        retrieve_and_handle_event_line(&line, &test.homeserver_id, handler.clone()).await?;
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
    let homeserver_id = test.homeserver_id.clone();
    let handle = |line: String| {
        let handler = handler.clone();
        let homeserver_id = homeserver_id.clone();
        async move { retrieve_and_handle_event_line(&line, &homeserver_id, handler).await }
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
            &test.homeserver_id,
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
    let embedded = format!("https://example.com/crash-embed/{author}");
    let rid = resource_id(&normalize_uri(&embedded).map_err(anyhow::Error::msg)?.0);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"first","parent":target}),
    )
    .await?;
    test = test.remove_event_processing().await;
    // A later version that adds an embed
    let edit = generate_post_id();
    let version: ResourcePath = format!("/pub/social/v1/posts/{id}/{edit}.json").parse()?;
    test.put(
        &key,
        &version,
        &json!({"kind":"note","content":"first","parent":target,"embed":embedded}),
    )
    .await?;
    let post_uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
    let post_path = format!("/pub/social/v1/posts/{id}");
    let lines = [
        (&target, ResourceChangeKind::Reply),
        (&embedded, ResourceChangeKind::Embed),
    ]
    .into_iter()
    .map(|(target, kind)| references::reference(&author, &post_uri, target, kind))
    .collect::<Result<Vec<_>, _>>()?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    references::stage(&post, lines).await?; // committed graph and outbox, no append
    assert!(ResourceChange::get_from_index(cursor, 1000)
        .await?
        .0
        .lines
        .iter()
        .all(|line| line.author != author));
    let handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{version}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let puts: Vec<_> = lines
        .iter()
        .filter(|line| {
            line.author == author
                && line.op == ResourceChangeOp::Put
                && line.object_path == post_path
        })
        .collect();
    assert_eq!(
        puts.len(),
        1,
        "the handler's retry delivers the outbox, and its own write, which finds the edges, adds nothing"
    );
    assert_eq!(
        (puts[0].resource_id.as_str(), puts[0].kind),
        (rid.as_str(), ResourceChangeKind::Embed)
    );
    // A DEL that appended its dels and stopped before the graph delete: the post stays
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    references::retract(&author, &id).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    // Its retry appends them again and deletes the post; a replay finds nothing to retract
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{version}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{version}"),
        &test.homeserver_id,
        handler,
    )
    .await?;
    let lines = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
    let deletes: Vec<_> = lines
        .iter()
        .filter(|line| line.author == author && line.op == ResourceChangeOp::Del)
        .collect();
    assert_eq!(
        deletes.len(),
        4,
        "each edge's del and the retry's duplicate, and no more"
    );
    assert!(deletes.iter().all(|line| line.object_path == post_path));
    for target in [&target, &embedded] {
        assert!(ResourceView::get_by_id(
            &resource_id(&normalize_uri(target).map_err(anyhow::Error::msg)?.0),
            None,
            None,
            None,
            None
        )
        .await?
        .is_none());
    }
    test.del(&key, &profile).await?;
    Ok(())
}

/// `id`'s Resource edges as the graph holds them: kind, Resource id, path and time.
async fn reference_edges(
    author: &str,
    id: &str,
) -> Result<Vec<(ResourceChangeKind, String, String, i64)>> {
    let row = fetch_row_from_graph(queries::get::resource_references_state(author, id))
        .await?
        .expect("the post");
    let lines: Vec<ResourceChange> = row.get("lines")?;
    Ok(lines
        .into_iter()
        .map(|line| (line.kind, line.resource_id, line.object_path, line.at))
        .collect())
}

/// Two version files of one post that name the same targets, handled in turn and then again
/// (A, B, A, B: a batch that runs again, or a homeserver that repeats both lines), append nothing
/// after the first PUT, although their content differs: a reference is keyed by the post's
/// address, which every version file shares. The edges keep their path and the time they were
/// first indexed. One post is a collection of several items, the other a reply to a URL with
/// an external embed (a collection has neither). A version that drops one item appends exactly
/// that item's del.
#[tokio_shared_rt::test(shared)]
async fn version_files_of_one_post_naming_the_same_targets_append_nothing() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Reference versions", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let parent = format!("https://example.com/versions/parent/{author}");
    let embed = format!("https://example.com/versions/embed/{author}");
    let items: Vec<String> = (0..4)
        .map(|n| format!("https://example.com/versions/item/{n}/{author}"))
        .collect();
    let collection = |name: &str, items: &[String]| {
        let items: Vec<_> = items.iter().map(|uri| json!({ "uri": uri })).collect();
        json!({
            "kind": "collection",
            "content": json!({ "name": name, "items": items }).to_string(),
        })
    };
    let note =
        |content: &str| json!({"kind":"note","content":content,"parent":parent,"embed":embed});
    let (collection_id, note_id) = (generate_post_id(), generate_post_id());
    let handler = test.event_processor_runner.event_handler.clone();
    let mut last = Vec::new();
    for (id, a_body, b_body, references) in [
        (
            &collection_id,
            collection("A", &items),
            collection("B", &items),
            items.len(),
        ),
        (&note_id, note("version A"), note("version B"), 2),
    ] {
        let post_path = format!("/pub/social/v1/posts/{id}");
        let a: ResourcePath = format!("{post_path}/{id}.json").parse()?;
        let b: ResourcePath = format!("{post_path}/{}.json", generate_post_id()).parse()?;
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        test.put(&key, &a, &a_body).await?;
        let first = own_ops_since(&author, cursor).await?;
        assert_eq!(first.len(), references, "{id}: {first:?}");
        assert!(first.iter().all(|(op, _)| *op == ResourceChangeOp::Put));
        let first_edges = reference_edges(&author, id).await?;
        assert_eq!(first_edges.len(), references, "{id}");
        assert!(first_edges.iter().all(|(_, _, path, _)| *path == post_path));

        // B is an edit (its content differs) with the same targets; then A and B again
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        test.put(&key, &b, &b_body).await?;
        for path in [&a, &b] {
            retrieve_and_handle_event_line(
                &format!("PUT pubky://{author}{path}"),
                &test.homeserver_id,
                handler.clone(),
            )
            .await?;
        }
        assert_eq!(
            own_ops_since(&author, cursor).await?,
            [],
            "{id}: no version file after the first PUT appends a line"
        );
        let content = PostDetails::get_from_graph(&author, id)
            .await?
            .unwrap()
            .0
            .content;
        assert!(
            content == "version B" || content.contains(r#""name":"B""#),
            "the version handled last is the post: {content}"
        );
        assert_eq!(
            reference_edges(&author, id).await?,
            first_edges,
            "{id}: the edges keep their path and first time"
        );
        last.push(b);
    }

    // A version that drops one item appends that item's del, and nothing else
    let c: ResourcePath = format!(
        "/pub/social/v1/posts/{collection_id}/{}.json",
        generate_post_id()
    )
    .parse()?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    test.put(&key, &c, &collection("C", &items[1..])).await?;
    assert_eq!(
        own_ops_since(&author, cursor).await?,
        [(
            ResourceChangeOp::Del,
            resource_id(&normalize_uri(&items[0]).map_err(anyhow::Error::msg)?.0)
        )]
    );

    test.del(&key, &c).await?;
    test.del(&key, &last[1]).await?;
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
    // The lines stored since `since`, as the homeserver sent them: without their key
    let stored_since = |since: u64| async move {
        let (lines, _) = EventLine::get_from_index(Some(since), usize::MAX).await?;
        anyhow::Ok(
            lines
                .iter()
                .map(|line| EventLine::split(line).0.to_string())
                .collect::<Vec<_>>(),
        )
    };
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
    let del = |path: &ResourcePath| format!("DEL pubky://{author}{path}");

    // Never indexed
    let (_, since) = EventLine::get_from_index(None, usize::MAX).await?;
    retrieve_and_handle_event_line(&del(&path), &test.homeserver_id, handler.clone()).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert!(!stored_since(since).await?.contains(&del(&path)));

    // Indexed: the first version DEL deletes the post and is stored, another one finds nothing
    test.put(&key, &path, &json!({"kind":"note","content":"to delete"}))
        .await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    let (_, since) = EventLine::get_from_index(None, usize::MAX).await?;
    retrieve_and_handle_event_line(&del(&path), &test.homeserver_id, handler.clone()).await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    retrieve_and_handle_event_line(&del(&edit), &test.homeserver_id, handler.clone()).await?;
    let stored = stored_since(since).await?;
    assert!(stored.contains(&del(&path)), "the deleting DEL is stored");
    assert!(
        !stored.contains(&del(&edit)),
        "the DEL that found nothing is not"
    );

    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{profile}"),
        &test.homeserver_id,
        handler,
    )
    .await?;
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
        &test.homeserver_id,
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

/// Handles `line`, read from `homeserver_id`, on its own task, as the live runners and the retry
/// processor do.
fn spawn_event_line(
    line: String,
    homeserver_id: pubky_social_specs::PubkyId,
    handler: std::sync::Arc<dyn nexus_watcher::events::EventHandler>,
) -> tokio::task::JoinHandle<Result<(), nexus_watcher::EventProcessorError>> {
    tokio::spawn(
        async move { retrieve_and_handle_event_line(&line, &homeserver_id, handler).await },
    )
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
    let lost_target = format!("https://example.com/delete-race-lost/{author}");
    let lost_rid = resource_id(&normalize_uri(&lost_target).map_err(anyhow::Error::msg)?.0);
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
    // it added an embed the newer version replaces.
    let post_uri = format!("pubky://{author}/pub/social/v1/posts/{id}");
    let lines = [
        (&target, ResourceChangeKind::Reply),
        (&lost_target, ResourceChangeKind::Embed),
    ]
    .into_iter()
    .map(|(target, kind)| references::reference(&author, &post_uri, target, kind))
    .collect::<Result<Vec<_>, _>>()?;
    let post = PostDetails::get_by_id(&author, &id).await?.unwrap();
    references::stage(&post, lines).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    let handler = test.event_processor_runner.event_handler.clone();
    let (reached, release) = pause_references(ReferencePause::Flush, &author);
    let put = spawn_event_line(
        format!("PUT pubky://{author}{newer}"),
        test.homeserver_id.clone(),
        handler.clone(),
    );
    // The PUT read the outbox, and has not appended it nor staged its own references.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the PUT never reached the outbox append"))??;
    let mut del = spawn_event_line(
        format!("DEL pubky://{author}{newer}"),
        test.homeserver_id.clone(),
        handler,
    );
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
        (&lost_rid, "the lost append's embed"),
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
    for rid in [&rid, &new_rid, &lost_rid] {
        assert!(ResourceView::get_by_id(rid, None, None, None, None)
            .await?
            .is_none());
    }
    // Event processing is off: handle the profile's DEL here, so the user does not stay behind
    test.del(&key, &profile).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{profile}"),
        &test.homeserver_id,
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
    let old_put = spawn_event_line(
        format!("PUT pubky://{author}{older}"),
        test.homeserver_id.clone(),
        handler.clone(),
    );
    // The older version's post write is done, its references are not.
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the older PUT never reached its references"))??;
    let mut new_put = spawn_event_line(
        format!("PUT pubky://{author}{newer}"),
        test.homeserver_id.clone(),
        handler,
    );
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

/// A newer version's PUT waits for the post's lock before its post write: while an older
/// version's PUT holds it, paused after its post write and before its references, the post
/// keeps the older content. Released, the older PUT finishes and the newer one runs last: the
/// post has the newer content, the newer version's (no) embed, so the Resource lists no embed,
/// and it stays a ranked root post. Both versions are root posts: an edit cannot change an
/// external reply parent, so no version here changes the post's reply state under the lock.
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
    let old_put = spawn_event_line(
        format!("PUT pubky://{author}{older}"),
        test.homeserver_id.clone(),
        handler.clone(),
    );
    tokio::time::timeout(Duration::from_secs(30), reached)
        .await
        .map_err(|_| anyhow::anyhow!("the older PUT never reached its references"))??;
    assert_eq!(content().await?, "older words");
    let mut new_put = spawn_event_line(
        format!("PUT pubky://{author}{newer}"),
        test.homeserver_id.clone(),
        handler,
    );
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
        &test.homeserver_id,
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
        &test.homeserver_id,
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
        &test.homeserver_id,
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

/// `author`'s lines since `cursor`, as (op, resource id).
async fn own_ops_since(author: &str, cursor: u64) -> Result<Vec<(ResourceChangeOp, String)>> {
    Ok(ResourceChange::get_from_index(cursor, 10_000)
        .await?
        .0
        .lines
        .into_iter()
        .filter(|line| line.author == author)
        .map(|line| (line.op, line.resource_id))
        .collect())
}

/// The reply, embed and collection counts of the Resource `id`.
async fn reference_counts(id: &str) -> Result<(usize, usize, usize)> {
    let view = ResourceView::get_by_id(id, None, None, None, None)
        .await?
        .expect("the Resource");
    Ok((
        view.replies_count,
        view.embeds_count,
        view.collections_count,
    ))
}

/// Two authors reference one URL, and the second author's post even has the same id as the
/// first's. One author's delete, or an edit that drops an embed, leaves the other author's
/// edges, counts and lists, and publishes no line naming the other author. A Resource that has
/// a tag and loses its only post stays, with its tag.
#[tokio_shared_rt::test(shared)]
async fn one_authors_delete_or_edit_leaves_another_authors_references() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (a_key, b_key) = (Keypair::random(), Keypair::random());
    let mut profiles = Vec::new();
    for (key, name) in [
        (&a_key, "Shared references A"),
        (&b_key, "Shared references B"),
    ] {
        test.register_user(key).await?;
        profiles.push(test.put_v1_profile(key, &v1_test_user(name, "")).await?);
        super::utils::watcher::rank_user(&key.public_key().to_z32()).await?;
    }
    let (a, b) = (a_key.public_key().to_z32(), b_key.public_key().to_z32());
    let replied = format!("https://example.com/shared-reply/{a}");
    let embedded = format!("https://example.com/shared-embed/{a}");
    let tagged = format!("https://example.com/shared-tagged/{a}");
    let (replied_id, embedded_id, tagged_id) = (
        resource_id(&replied),
        resource_id(&embedded),
        resource_id(&tagged),
    );
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    let both = json!({"kind":"note","content":"shared","parent":replied,"embed":embedded});
    test.put(&a_key, &path, &both).await?;
    test.put(&b_key, &path, &both).await?;
    let embed_id = generate_post_id();
    let embed_path: ResourcePath =
        format!("/pub/social/v1/posts/{embed_id}/{embed_id}.json").parse()?;
    test.put(
        &a_key,
        &embed_path,
        &json!({"kind":"note","content":"an embed","embed":embedded}),
    )
    .await?;
    let tag = PubkySocialTag::new(tagged.clone(), "kept".into());
    let tag_path: ResourcePath = PubkySocialTag::create_path(&tag.create_id()).parse()?;
    test.put(&b_key, &tag_path, &tag).await?;
    let only_id = generate_post_id();
    let only_path: ResourcePath =
        format!("/pub/social/v1/posts/{only_id}/{only_id}.json").parse()?;
    test.put(
        &a_key,
        &only_path,
        &json!({"kind":"note","content":"the only post","embed":tagged}),
    )
    .await?;
    assert_eq!(reference_counts(&replied_id).await?, (2, 0, 0));
    assert_eq!(reference_counts(&embedded_id).await?, (0, 3, 0));
    assert_eq!(reference_counts(&tagged_id).await?, (0, 1, 0));
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    // A deletes its post with B's id
    test.del(&a_key, &path).await?;
    assert_eq!(
        references::posts(&replied_id, ResourceChangeKind::Reply, 0, 10).await?,
        [(b.clone(), id.clone())]
    );
    assert_eq!(reference_counts(&replied_id).await?, (1, 0, 0));
    assert_eq!(reference_counts(&embedded_id).await?, (0, 2, 0));
    assert!(PostDetails::get_from_graph(&b, &id).await?.is_some());
    assert_eq!(root_lists_and_replies(&b, &id).await?, REPLY);
    // A edits its embed away
    let edit: ResourcePath = format!(
        "/pub/social/v1/posts/{embed_id}/{}.json",
        generate_post_id()
    )
    .parse()?;
    test.put(&a_key, &edit, &json!({"kind":"note","content":"no embed"}))
        .await?;
    assert_eq!(
        references::posts(&embedded_id, ResourceChangeKind::Embed, 0, 10).await?,
        [(b.clone(), id.clone())]
    );
    assert_eq!(reference_counts(&embedded_id).await?, (0, 1, 0));
    // The tagged Resource loses its only post and keeps its tag
    test.del(&a_key, &only_path).await?;
    let view = ResourceView::get_by_id(&tagged_id, None, None, None, None)
        .await?
        .expect("the tag keeps the Resource");
    assert_eq!(
        (view.taggers_count, view.embeds_count),
        (1, 0),
        "the tag stays, the embed goes"
    );
    assert_eq!(own_ops_since(&b, cursor).await?, [], "no line names B");
    let a_ops = own_ops_since(&a, cursor).await?;
    for rid in [&replied_id, &embedded_id, &tagged_id] {
        assert!(
            a_ops.contains(&(ResourceChangeOp::Del, rid.clone())),
            "A's references are retracted: {a_ops:?}"
        );
    }

    test.del(&a_key, &edit).await?;
    test.del(&b_key, &path).await?;
    test.del(&b_key, &tag_path).await?;
    for rid in [&replied_id, &embedded_id, &tagged_id] {
        assert!(
            ResourceView::get_by_id(rid, None, None, None, None)
                .await?
                .is_none(),
            "the Resource goes with its last relationship"
        );
    }
    for (key, profile) in [&a_key, &b_key].into_iter().zip(&profiles) {
        test.del(key, profile).await?;
    }
    Ok(())
}

/// A tombstone keeps its Resource edges and is off every Resource-side read: no reply, embed
/// or collection count and no list names it. A PUT that brings it back puts its references
/// again, after the dels its delete published.
#[tokio_shared_rt::test(shared)]
async fn a_tombstone_is_off_every_count_and_a_put_brings_its_references_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Tombstone counts", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    let replied = format!("https://example.com/tombstone-counts/reply/{author}");
    let embedded = format!("https://example.com/tombstone-counts/embed/{author}");
    let collected = format!("https://example.com/tombstone-counts/item/{author}");
    let (replied_id, embedded_id, collected_id) = (
        resource_id(&replied),
        resource_id(&embedded),
        resource_id(&collected),
    );
    let post =
        |content: &str| json!({"kind":"note","content":content,"parent":replied,"embed":embedded});
    let child = |parent: &str| json!({"kind":"note","content":"keeps the tombstone","parent":format!("pubky://{author}/pub/social/v1/posts/{parent}")});
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(&key, &path, &post("a reply with an embed"))
        .await?;
    let collection_id = generate_post_id();
    let collection: ResourcePath =
        format!("/pub/social/v1/posts/{collection_id}/{collection_id}.json").parse()?;
    test.put(&key, &collection, &json!({"kind":"collection", "content":json!({"name":"Items","items":[{"uri":collected}]}).to_string()})).await?;
    let mut children = Vec::new();
    for parent in [&id, &collection_id] {
        let child_id = generate_post_id();
        let child_path: ResourcePath =
            format!("/pub/social/v1/posts/{child_id}/{child_id}.json").parse()?;
        test.put(&key, &child_path, &child(parent)).await?;
        children.push(child_path);
    }
    assert_eq!(reference_counts(&replied_id).await?, (1, 0, 0));
    assert_eq!(reference_counts(&embedded_id).await?, (0, 1, 0));
    assert_eq!(reference_counts(&collected_id).await?, (0, 0, 1));

    test.del(&key, &path).await?;
    test.del(&key, &collection).await?;
    for (rid, kind) in [
        (&replied_id, ResourceChangeKind::Reply),
        (&embedded_id, ResourceChangeKind::Embed),
        (&collected_id, ResourceChangeKind::Collection),
    ] {
        assert_eq!(reference_counts(rid).await?, (0, 0, 0), "{kind:?}");
        assert!(
            references::posts(rid, kind, 0, 10).await?.is_empty(),
            "{kind:?}"
        );
    }
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    // The PUT that brings the reply back
    test.put(&key, &path, &post("back again")).await?;
    assert!(
        !PostDetails::get_from_graph(&author, &id)
            .await?
            .unwrap()
            .0
            .deleted
    );
    let ops = own_ops_since(&author, cursor).await?;
    assert_eq!(ops.len(), 2, "{ops:?}");
    for rid in [&replied_id, &embedded_id] {
        assert!(
            ops.contains(&(ResourceChangeOp::Put, rid.clone())),
            "its references are put again: {ops:?}"
        );
    }
    assert_eq!(reference_counts(&replied_id).await?, (1, 0, 0));
    assert_eq!(reference_counts(&embedded_id).await?, (0, 1, 0));

    for child_path in &children {
        test.del(&key, child_path).await?;
    }
    test.del(&key, &path).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{collection}"),
        &test.homeserver_id,
        test.event_processor_runner.event_handler.clone(),
    )
    .await?;
    for rid in [&replied_id, &embedded_id, &collected_id] {
        assert!(ResourceView::get_by_id(rid, None, None, None, None)
            .await?
            .is_none());
    }
    test.del(&key, &profile).await?;
    Ok(())
}

/// A DEL marks the post's references retracted before it appends their dels: after one that
/// stops right after its append, with the post still in the graph, a PUT puts them again.
#[tokio_shared_rt::test(shared)]
async fn a_put_after_a_delete_that_stopped_after_its_append_puts_the_references_again() -> Result<()>
{
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Stopped after the del append", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let target = format!("https://example.com/stopped-after-append/{author}");
    let rid = resource_id(&target);
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"a reply","parent":target}),
    )
    .await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;

    fail_feed_dels_after_append(&author, true);
    let failed = retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await;
    fail_feed_dels_after_append(&author, false);
    assert!(failed.is_err(), "the injected failure after the append");
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    assert_eq!(
        own_ops_since(&author, cursor).await?,
        [
            (ResourceChangeOp::Del, rid.clone()),
            (ResourceChangeOp::Put, rid.clone())
        ]
    );

    for line in [
        format!("DEL pubky://{author}{path}"),
        format!("DEL pubky://{author}{profile}"),
    ] {
        retrieve_and_handle_event_line(&line, &test.homeserver_id, handler.clone()).await?;
    }
    Ok(())
}

/// An edit that drops a post's only external embed and stops after its reference write leaves
/// the cached relationships with the embed; its retry, which finds no edge left, still reads
/// them back.
#[tokio_shared_rt::test(shared)]
async fn a_retried_edit_that_dropped_the_last_embed_reads_the_relationships_back() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Dropped embed retried", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let embedded = format!("https://example.com/dropped-embed/{author}");
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"an embed","embed":embedded}),
    )
    .await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    let cached = || async {
        anyhow::Ok(
            PostRelationships::get_from_index(&author, &id)
                .await?
                .expect("cached relationships")
                .embedded,
        )
    };
    assert_eq!(cached().await?.as_deref(), Some(embedded.as_str()));

    let edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
    test.put(&key, &edit, &json!({"kind":"note","content":"no embed"}))
        .await?;
    fail_references(ReferenceFault::AfterSync, &author, true);
    let failed = retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{edit}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await;
    fail_references(ReferenceFault::AfterSync, &author, false);
    assert!(failed.is_err(), "the injected failure");
    assert_eq!(cached().await?.as_deref(), Some(embedded.as_str()), "stale");
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{edit}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    assert_eq!(
        cached().await?,
        None,
        "the retry reads the relationships back"
    );

    for line in [
        format!("DEL pubky://{author}{edit}"),
        format!("DEL pubky://{author}{profile}"),
    ] {
        retrieve_and_handle_event_line(&line, &test.homeserver_id, handler.clone()).await?;
    }
    Ok(())
}

/// A PUT that makes a reply a root post and stops between its root list writes leaves the post
/// in its author's replies as well as in its timelines: its retry sees the disagreement and
/// finishes the lists.
#[tokio_shared_rt::test(shared)]
async fn a_put_that_stops_between_the_root_list_writes_is_finished_by_its_retry() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let key = Keypair::random();
    test.register_user(&key).await?;
    let author = key.public_key().to_z32();
    let profile = test
        .put_v1_profile(&key, &v1_test_user("Stopped between list writes", ""))
        .await?;
    super::utils::watcher::rank_user(&author).await?;
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let homeserver_id = test.homeserver_id.clone();
    let handle = |line: String| {
        let handler = handler.clone();
        let homeserver_id = homeserver_id.clone();
        async move { retrieve_and_handle_event_line(&line, &homeserver_id, handler).await }
    };
    let id = generate_post_id();
    let path: ResourcePath = format!("/pub/social/v1/posts/{id}/{id}.json").parse()?;
    test.put(
        &key,
        &path,
        &json!({"kind":"note","content":"a reply","parent":format!("https://example.com/between-lists/{author}")}),
    )
    .await?;
    handle(format!("PUT pubky://{author}{path}")).await?;
    assert_eq!(root_lists_and_replies(&author, &id).await?, REPLY);
    // A Resource reply edge without a reconciled revision accepts an edit that drops the parent
    get_neo4j_graph()?
        .run(
            nexus_common::db::graph::Query::new(
                "test_forget_revision",
                "MATCH (:User {id: $author})-[:AUTHORED]->(p:Post {id: $id})
                 REMOVE p.resource_ref_revision",
            )
            .param("author", author.as_str())
            .param("id", id.as_str()),
        )
        .await?;
    let edit: ResourcePath =
        format!("/pub/social/v1/posts/{id}/{}.json", generate_post_id()).parse()?;
    test.put(
        &key,
        &edit,
        &json!({"kind":"note","content":"a root post now"}),
    )
    .await?;

    fail_references(ReferenceFault::BeforeRepliesRemoval, &author, true);
    let failed = handle(format!("PUT pubky://{author}{edit}")).await;
    fail_references(ReferenceFault::BeforeRepliesRemoval, &author, false);
    let err = failed.expect_err("the injected failure");
    assert!(err.to_string().contains("BeforeRepliesRemoval"), "{err}");
    assert!(!PostDetails::is_reply_in_graph(&author, &id).await?);
    assert_eq!(
        root_lists_and_replies(&author, &id).await?,
        (true, true, true, true),
        "stopped with the post still in its author's replies"
    );
    handle(format!("PUT pubky://{author}{edit}")).await?;
    assert_eq!(
        root_lists_and_replies(&author, &id).await?,
        ROOT,
        "the retry finishes the root post's lists"
    );

    for line in [
        format!("DEL pubky://{author}{edit}"),
        format!("DEL pubky://{author}{profile}"),
    ] {
        handle(line).await?;
    }
    Ok(())
}

/// The reference outbox `id` holds, raw.
async fn reference_outbox(author: &str, id: &str) -> Result<Option<String>> {
    let row = nexus_common::db::fetch_row_from_graph(
        nexus_common::db::graph::Query::new(
            "test_reference_outbox",
            "MATCH (:User {id: $author})-[:AUTHORED]->(p:Post {id: $id})
             RETURN p.resource_ref_outbox AS outbox",
        )
        .param("author", author)
        .param("id", id),
    )
    .await?
    .expect("the post");
    Ok(row.get("outbox")?)
}

/// Gives `id` an outbox no reader can parse, as a lost append in an older format leaves it.
async fn set_unreadable_outbox(author: &str, id: &str) -> Result<()> {
    get_neo4j_graph()?
        .run(
            nexus_common::db::graph::Query::new(
                "test_unreadable_outbox",
                "MATCH (:User {id: $author})-[:AUTHORED]->(p:Post {id: $id})
                 SET p.resource_ref_outbox = $raw",
            )
            .param("author", author)
            .param("id", id)
            .param(
                "raw",
                r#"[{"op":"put","from":"a format this release cannot read""#,
            ),
        )
        .await?;
    Ok(())
}

/// An outbox nothing can parse blocks nothing: a PUT, a DEL and a moderator's takedown of its
/// post each succeed, clear it and do their own work, as if no outbox was pending.
#[tokio_shared_rt::test(shared)]
async fn an_unreadable_outbox_blocks_no_put_delete_or_takedown_of_its_post() -> Result<()> {
    use super::posts::utils::{find_post_details, test_user};
    use super::utils::watcher::HomeserverHashIdPath;
    use pubky_social_specs::legacy_v0::{
        post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppTag,
    };

    let mut test = WatcherTest::setup(None).await?;
    let author_kp = Keypair::random();
    let author = test
        .create_user(
            &author_kp,
            &test_user("Watcher:UnreadableOutbox:Author", "author"),
        )
        .await?;
    let moderator_recovery_file =
        tokio::fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp =
        pubky::recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password")?;
    let moderator = test
        .create_user(
            &moderator_kp,
            &test_user("Watcher:UnreadableOutbox:Moderator", "moderator"),
        )
        .await?;
    let target = format!("https://example.com/unreadable-outbox/{author}");
    let rid = resource_id(&normalize_uri(&target).map_err(anyhow::Error::msg)?.0);
    let reply = |content: &str| PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(target.clone()),
        embed: None,
        attachments: None,
        lock: None,
    };
    let (deleted, deleted_path) = test
        .create_post(&author_kp, &reply("deleted by its author"))
        .await?;
    let (taken_down, _) = test.create_post(&author_kp, &reply("taken down")).await?;
    assert_eq!(reference_counts(&rid).await?, (2, 0, 0));
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();
    let homeserver_id = test.homeserver_id.clone();
    let handle = |line: String| {
        let handler = handler.clone();
        let homeserver_id = homeserver_id.clone();
        async move { retrieve_and_handle_event_line(&line, &homeserver_id, handler).await }
    };

    // A PUT (here a replay) clears it and keeps the post's references
    set_unreadable_outbox(&author, &deleted).await?;
    handle(format!("PUT pubky://{author}{deleted_path}")).await?;
    assert_eq!(reference_outbox(&author, &deleted).await?, None);
    assert_eq!(reference_counts(&rid).await?, (2, 0, 0));

    // A DEL clears it and retracts the post's reference
    set_unreadable_outbox(&author, &deleted).await?;
    let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
    handle(format!("DEL pubky://{author}{deleted_path}")).await?;
    assert!(find_post_details(&author, &deleted).await.is_err());
    assert_eq!(
        own_ops_since(&author, cursor).await?,
        [(ResourceChangeOp::Del, rid.clone())]
    );
    assert_eq!(reference_counts(&rid).await?, (1, 0, 0));

    // A takedown clears it and retracts the post's reference
    set_unreadable_outbox(&author, &taken_down).await?;
    let tag = PubkyAppTag {
        uri: post_uri_builder(author.clone(), taken_down.clone()),
        label: "label_to_moderate".to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&moderator_kp, &tag_path, &tag).await?;
    handle(format!("PUT pubky://{moderator}{tag_path}")).await?;
    assert!(find_post_details(&author, &taken_down).await.is_err());
    assert_eq!(
        own_ops_since(&author, cursor).await?,
        [
            (ResourceChangeOp::Del, rid.clone()),
            (ResourceChangeOp::Del, rid.clone())
        ]
    );
    assert!(ResourceView::get_by_id(&rid, None, None, None, None)
        .await?
        .is_none());

    test.cleanup_user(&author_kp).await?;
    handle(format!("DEL pubky://{author}/pub/pubky.app/profile.json")).await?;
    Ok(())
}

/// A deleted post's view names nothing it relates to, whether its author deleted it or a
/// moderator took it down, and whatever keeps its tombstone (a reply, a tag, a mention): no parent
/// (a URL here), no embed, no repost and no mention, from the cached relationships or the graph. The
/// tombstone keeps its edges and its cached relationships, the gate of its later hard delete.
/// A live post's view is unchanged.
#[tokio_shared_rt::test(shared)]
async fn a_deleted_posts_view_names_no_relationships() -> Result<()> {
    use super::posts::utils::test_user;
    use super::utils::watcher::HomeserverHashIdPath;
    use nexus_common::models::post::PostView;
    use pubky_social_specs::legacy_v0::{
        post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag,
    };

    let mut test = WatcherTest::setup(None).await?;
    let author_kp = Keypair::random();
    let author = test
        .create_user(
            &author_kp,
            &test_user("Watcher:DeletedRelationships:Author", "author"),
        )
        .await?;
    let other_kp = Keypair::random();
    let other = test
        .create_user(
            &other_kp,
            &test_user("Watcher:DeletedRelationships:Other", "other"),
        )
        .await?;
    let moderator_recovery_file =
        tokio::fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp =
        pubky::recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password")?;
    test.create_user(
        &moderator_kp,
        &test_user("Watcher:DeletedRelationships:Moderator", "moderator"),
    )
    .await?;
    let reply_target = format!("https://example.com/deleted-relationships/reply/{author}");
    let embed_target = format!("https://example.com/deleted-relationships/embed/{author}");
    let replied = normalize_uri(&reply_target).map_err(anyhow::Error::msg)?.0;
    let embedded = normalize_uri(&embed_target).map_err(anyhow::Error::msg)?.0;
    let mention = format!("mentions pubky{other}");
    let post = |content: &str, parent: Option<&str>, embed: Option<&str>| PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent: parent.map(str::to_string),
        embed: embed.map(|uri| PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: uri.to_string(),
        }),
        attachments: None,
        lock: None,
    };
    let tag = |id: &str, label: &str| PubkyAppTag {
        uri: post_uri_builder(author.clone(), id.to_string()),
        label: label.to_string(),
        created_at: chrono::Utc::now().timestamp_millis(),
    };

    // A live post replying to a URL, embedding another and mentioning a user: its view names all
    let (live, live_path) = test
        .create_post(
            &author_kp,
            &post(&mention, Some(&reply_target), Some(&embed_target)),
        )
        .await?;
    let view = PostView::get_by_id(&author, &live, None, None, None)
        .await?
        .expect("the live post");
    assert_eq!(
        view.relationships.replied.as_deref(),
        Some(replied.as_str())
    );
    assert!(view.relationships.replied_external);
    assert_eq!(
        view.relationships.embedded.as_deref(),
        Some(embedded.as_str())
    );
    assert_eq!(
        view.relationships
            .mentioned
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [other.as_str()]
    );

    // Posts the author or a moderator deletes, each kept as a tombstone by a reply or a tag, and
    // one kept by its mention (a post that mentions a user is never hard-deleted)
    #[derive(Clone, Copy, PartialEq)]
    enum Keeper {
        Reply,
        Tag,
        Mention,
    }
    let mut deleted = Vec::new();
    let mut keepers = Vec::new();
    for (case, parent, embed, keeper, taken_down) in [
        (
            "deleted reply to a URL",
            Some(&reply_target),
            None,
            Keeper::Reply,
            false,
        ),
        (
            "deleted post with an embed",
            None,
            Some(&embed_target),
            Keeper::Tag,
            false,
        ),
        (
            "taken down reply to a URL",
            Some(&reply_target),
            None,
            Keeper::Reply,
            true,
        ),
        (
            "taken down post with an embed",
            None,
            Some(&embed_target),
            Keeper::Tag,
            true,
        ),
        (
            "deleted post with a mention",
            None,
            None,
            Keeper::Mention,
            false,
        ),
    ] {
        let content = match keeper {
            Keeper::Mention => mention.as_str(),
            _ => "kept by a reply or a tag",
        };
        let (id, path) = test
            .create_post(
                &author_kp,
                &post(
                    content,
                    parent.map(String::as_str),
                    embed.map(String::as_str),
                ),
            )
            .await?;
        match keeper {
            Keeper::Reply => {
                let child = PubkyAppPost {
                    parent: Some(post_uri_builder(author.clone(), id.clone())),
                    ..super::posts::utils::short_post("keeps the tombstone")
                };
                let (_, child_path) = test.create_post(&author_kp, &child).await?;
                keepers.push((author_kp.clone(), child_path));
            }
            Keeper::Tag => {
                let keeper = tag(&id, "keeps_the_tombstone");
                let tag_path = keeper.hs_path();
                test.put(&other_kp, &tag_path, &keeper).await?;
                keepers.push((other_kp.clone(), tag_path));
            }
            Keeper::Mention => {}
        }
        if taken_down {
            let takedown = tag(&id, "label_to_moderate");
            test.put(&moderator_kp, &takedown.hs_path(), &takedown)
                .await?;
        } else {
            test.del(&author_kp, &path).await?;
        }
        deleted.push((case, id, path, keeper));
    }

    for (case, id, _, keeper) in &deleted {
        let details = PostDetails::get_by_id(&author, id)
            .await?
            .unwrap_or_else(|| panic!("{case}: kept as a tombstone"));
        assert!(details.deleted, "{case}");
        let cached = PostRelationships::get_from_index(&author, id)
            .await?
            .unwrap_or_else(|| panic!("{case}: the hard delete's gate stays"));
        assert!(
            cached.replied.is_some() || cached.embedded.is_some() || !cached.mentioned.is_empty(),
            "{case}: the tombstone keeps its edges"
        );
        assert_eq!(
            *keeper == Keeper::Mention,
            !cached.mentioned.is_empty(),
            "{case}"
        );
        let none = |view: PostView| {
            let relationships = view.relationships;
            assert_eq!(relationships.replied, None, "{case}");
            assert!(!relationships.replied_external, "{case}");
            assert_eq!(relationships.reposted, None, "{case}");
            assert_eq!(relationships.embedded, None, "{case}");
            assert!(relationships.mentioned.is_empty(), "{case}");
        };
        none(
            PostView::get_by_id(&author, id, None, None, None)
                .await?
                .unwrap_or_else(|| panic!("{case}")),
        );
        // From the graph too, as after a cache loss; then the gate is put back
        PostRelationships::delete(&author, id).await?;
        none(
            PostView::get_by_id(&author, id, None, None, None)
                .await?
                .unwrap_or_else(|| panic!("{case}")),
        );
        PostRelationships::reindex(&author, id).await?;
    }

    // Cleanup: the keepers go, then the tombstones' DELs replayed hard-delete them
    for (kp, path) in &keepers {
        test.del(kp, path).await?;
    }
    for (case, id, path, _) in deleted.iter().filter(|post| post.3 != Keeper::Mention) {
        retrieve_and_handle_event_line(
            &format!("DEL pubky://{author}{path}"),
            &test.homeserver_id,
            test.event_processor_runner.event_handler.clone(),
        )
        .await?;
        assert!(
            PostDetails::get_from_graph(&author, id).await?.is_none(),
            "{case}: hard-deleted"
        );
    }
    test.del(&author_kp, &live_path).await?;
    test.cleanup_user(&author_kp).await?;
    test.cleanup_user(&other_kp).await?;
    Ok(())
}

/// A reply with an external embed whose hard delete stops after its steps that run once (the
/// parent's score, the "a reply was deleted" notification), then a PUT of the same post handled
/// before the delete's retry: the PUT reads the post's cached relationships back from the graph,
/// and the retry still lowers the parent's score once and notifies its author once.
#[tokio_shared_rt::test(shared)]
async fn a_put_between_a_half_done_delete_and_its_retry_runs_no_step_twice() -> Result<()> {
    use super::posts::utils::{check_member_total_engagement_user_posts, short_post, test_user};
    use nexus_common::models::notification::{Notification, NotificationBody};
    use pubky_social_specs::legacy_v0::{
        post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
    };

    let mut test = WatcherTest::setup(None).await?;
    let parent_kp = Keypair::random();
    let parent_author = test
        .create_user(
            &parent_kp,
            &test_user("Watcher:HalfDoneDelete:Parent", "parent"),
        )
        .await?;
    let author_kp = Keypair::random();
    let author = test
        .create_user(
            &author_kp,
            &test_user("Watcher:HalfDoneDelete:Author", "author"),
        )
        .await?;
    let (root, root_path) = test
        .create_post(&parent_kp, &short_post("the parent"))
        .await?;
    let root_key = [parent_author.as_str(), root.as_str()];
    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(parent_author.clone(), root.clone())),
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: format!("https://example.com/half-done-delete/{author}"),
        }),
        ..short_post("a reply with an external embed")
    };
    let (id, path) = test.create_post(&author_kp, &reply).await?;
    assert_eq!(
        check_member_total_engagement_user_posts(&root_key).await?,
        Some(1)
    );
    // The "a reply was deleted" notices, by time: a second send of the same notice keeps one
    // entry under a new time.
    let deleted_notices = || async {
        anyhow::Ok(
            Notification::get_by_id(&parent_author, Pagination::default())
                .await?
                .into_iter()
                .filter(|n| matches!(n.body, NotificationBody::PostDeleted { .. }))
                .map(|n| n.timestamp)
                .collect::<Vec<_>>(),
        )
    };
    test = test.remove_event_processing().await;
    let handler = test.event_processor_runner.event_handler.clone();

    // The delete stops after its steps that run once, before the post leaves Redis and the graph
    test.del(&author_kp, &path).await?;
    fail_references(ReferenceFault::BeforeDetailsDelete, &author, true);
    let stopped = retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await;
    fail_references(ReferenceFault::BeforeDetailsDelete, &author, false);
    assert!(stopped.is_err(), "{stopped:?}");
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_some());
    assert!(PostRelationships::get_from_index(&author, &id)
        .await?
        .is_none());
    assert_eq!(
        check_member_total_engagement_user_posts(&root_key).await?,
        Some(0)
    );
    let notices = deleted_notices().await?;
    assert_eq!(notices.len(), 1);

    // A PUT of the post before the retry writes its cached relationships again
    test.put(&author_kp, &path, &reply).await?;
    retrieve_and_handle_event_line(
        &format!("PUT pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    assert!(PostRelationships::get_from_index(&author, &id)
        .await?
        .is_some());

    // The retry finishes the delete and runs none of those steps again
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{author}{path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    assert!(PostDetails::get_from_graph(&author, &id).await?.is_none());
    assert_eq!(
        check_member_total_engagement_user_posts(&root_key).await?,
        Some(0),
        "the parent's score is lowered once"
    );
    assert_eq!(deleted_notices().await?, notices, "its author is told once");

    test.del(&author_kp, &path).await?;
    test.del(&parent_kp, &root_path).await?;
    retrieve_and_handle_event_line(
        &format!("DEL pubky://{parent_author}{root_path}"),
        &test.homeserver_id,
        handler.clone(),
    )
    .await?;
    for (kp, user) in [(&author_kp, &author), (&parent_kp, &parent_author)] {
        test.cleanup_user(kp).await?;
        retrieve_and_handle_event_line(
            &format!("DEL pubky://{user}/pub/pubky.app/profile.json"),
            &test.homeserver_id,
            handler.clone(),
        )
        .await?;
    }
    Ok(())
}
