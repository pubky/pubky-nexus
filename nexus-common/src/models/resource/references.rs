//! References from social posts to universal Resources. Content stays on the Post.

use crate::db::graph::{GraphError, Query};
use crate::db::{fetch_row_from_graph, get_neo4j_graph};
use crate::models::error::{ModelError, ModelResult};
use crate::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use crate::models::post::PostDetails;
use crate::universal_tag::normalize::{normalize_uri, resource_id};
use futures::TryStreamExt;
use std::collections::HashMap;

pub fn reference(
    author: &str,
    source_uri: &str,
    target: &str,
    kind: ResourceChangeKind,
) -> ModelResult<ResourceChange> {
    let path = source_uri
        .strip_prefix(&format!("pubky://{author}"))
        .filter(|path| path.starts_with("/pub/"))
        .ok_or_else(|| ModelError::InvalidInput("Invalid reference source URI".into()))?;
    let (uri, _) = normalize_uri(target).map_err(ModelError::InvalidInput)?;
    Ok(ResourceChange {
        op: ResourceChangeOp::Put,
        resource_id: resource_id(&uri),
        uri,
        kind,
        author: author.into(),
        label: None,
        object_path: path.into(),
        at: chrono::Utc::now().timestamp_millis(),
    })
}

#[derive(Default)]
struct State {
    revision: String,
    /// A stage wrote the post's revision: its references were reconciled at least once.
    reconciled: bool,
    /// The post is a tombstone: it keeps its Resource edges, whose dels were published when it
    /// became one.
    deleted: bool,
    /// [`retract`] published the dels of the post's edges, and no [`stage`] has published them
    /// again since: the next one puts every edge, changed or not.
    retracted: bool,
    outbox: Option<String>,
    lines: Vec<ResourceChange>,
}

async fn state(author: &str, id: &str) -> ModelResult<State> {
    let row = fetch_row_from_graph(Query::new("resource_references_state", r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        OPTIONAL MATCH (p)-[e:REPLIED|EMBEDS|COLLECTED]->(r:Resource)
        WITH p, e, r ORDER BY type(e), r.id
        RETURN coalesce(p.resource_ref_revision, '') AS revision,
               p.resource_ref_revision IS NOT NULL AS reconciled,
               coalesce(p.deleted, false) AS deleted,
               coalesce(p.resource_ref_retracted, false) AS retracted,
               p.resource_ref_outbox AS outbox,
               collect(CASE WHEN e IS NOT NULL THEN {
                 op: 'put', resource_id: r.id, uri: r.uri,
                 kind: CASE type(e) WHEN 'REPLIED' THEN 'reply' WHEN 'EMBEDS' THEN 'embed' ELSE 'collection' END,
                 author: $author, object_path: e.object_path, at: e.indexed_at
               } END) AS lines
    ").param("author", author).param("id", id)).await?;
    match row {
        Some(row) => Ok(State {
            revision: row.get("revision")?,
            reconciled: row.get("reconciled")?,
            deleted: row.get("deleted")?,
            retracted: row.get("retracted")?,
            outbox: row.get("outbox")?,
            lines: row.get("lines")?,
        }),
        None => Ok(State::default()),
    }
}

fn same_reference(a: &ResourceChange, b: &ResourceChange) -> bool {
    a.kind == b.kind && a.resource_id == b.resource_id && a.object_path == b.object_path
}

/// Finish an append whose graph transaction committed but whose acknowledgement
/// was lost. Clearing the exact outbox value cannot erase a later graph write.
pub async fn flush(author: &str, id: &str) -> ModelResult<()> {
    match state(author, id).await?.outbox {
        Some(raw) => publish(author, id, raw).await,
        None => Ok(()),
    }
}

/// Appends the outbox `raw` the post holds to the feed, then clears it if it is still the one.
async fn publish(author: &str, id: &str, raw: String) -> ModelResult<()> {
    #[cfg(any(test, feature = "test-utils"))]
    crate::utils::test_utils::reference_pause(
        crate::utils::test_utils::ReferencePause::Flush,
        author,
    )
    .await;
    let lines: Vec<ResourceChange> =
        serde_json::from_str(&raw).map_err(ModelError::from_generic)?;
    ResourceChange::store_batch(&lines).await?;
    get_neo4j_graph()?
        .run(
            Query::new(
                "resource_references_ack",
                r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
          AND p.resource_ref_outbox = $outbox
        REMOVE p.resource_ref_outbox
    ",
            )
            .param("author", author)
            .param("id", id)
            .param("outbox", raw),
        )
        .await
        .map_err(GraphError::from)?;
    Ok(())
}

/// Atomically reconcile the external edges and save the append outbox on their
/// source Post, then publish it. A stale recovery cannot replace a newer post's references.
///
/// Returns whether the post has or had any Resource edge: only then can its cached
/// relationships and lists differ from what its post write left. It reads the post's
/// references once.
pub async fn sync(post: &PostDetails, lines: Vec<ResourceChange>) -> ModelResult<bool> {
    #[cfg(any(test, feature = "test-utils"))]
    crate::utils::test_utils::reference_fault(
        crate::utils::test_utils::ReferenceFault::BeforeStage,
        &post.author,
    )?;
    let Staged { touched, outbox } = stage_outbox(post, lines).await?;
    if let Some(raw) = outbox {
        publish(&post.author, &post.id, raw).await?;
    }
    #[cfg(any(test, feature = "test-utils"))]
    crate::utils::test_utils::reference_fault(
        crate::utils::test_utils::ReferenceFault::AfterSync,
        &post.author,
    )?;
    Ok(touched)
}

/// What [`stage_outbox`] committed.
struct Staged {
    /// The post has or had a Resource edge.
    touched: bool,
    /// The outbox it saved, to publish.
    outbox: Option<String>,
}

/// Commit edges and recovery metadata, without acknowledging the feed append. The outbox holds
/// the dels of the edges the post loses and the puts of the ones it gains, or of all its edges
/// after a [`retract`]; with neither, nothing is written and nothing is published. A new edge
/// takes this write's time, an existing one keeps the time it was first indexed.
///
/// Like [`flush`] and [`retract`], it reads and then writes the post's
/// references and outbox: its callers serialize every handler of one post (the watcher's post
/// lock), so another one's write cannot land between its read and its write.
pub async fn stage(post: &PostDetails, lines: Vec<ResourceChange>) -> ModelResult<()> {
    stage_outbox(post, lines).await.map(|_| ())
}

/// [`stage`], returning what it committed. An outbox a lost append left is published first,
/// from the same read: the edges were written with it.
async fn stage_outbox(post: &PostDetails, mut lines: Vec<ResourceChange>) -> ModelResult<Staged> {
    #[cfg(any(test, feature = "test-utils"))]
    crate::utils::test_utils::reference_pause(
        crate::utils::test_utils::ReferencePause::Stage,
        &post.author,
    )
    .await;
    let mut old = state(&post.author, &post.id).await?;
    if let Some(raw) = old.outbox.take() {
        publish(&post.author, &post.id, raw).await?;
    }
    let at = chrono::Utc::now().timestamp_millis();
    for line in &mut lines {
        line.at = at;
    }
    lines.sort_by_key(|line| (format!("{:?}", line.kind), line.resource_id.clone()));
    lines.dedup_by(|a, b| same_reference(a, b));
    // An edge keeps the time it was first indexed, whichever version file names it now.
    for line in &mut lines {
        if let Some(edge) = old
            .lines
            .iter()
            .find(|old| old.kind == line.kind && old.resource_id == line.resource_id)
        {
            line.at = edge.at;
        }
    }
    let mut changes: Vec<_> = old
        .lines
        .iter()
        .filter(|old| !lines.iter().any(|line| same_reference(old, line)))
        .cloned()
        .map(|mut line| {
            line.op = ResourceChangeOp::Del;
            line.at = at;
            line
        })
        .collect();
    // Only the references the edges gain are published, so a replay appends nothing. After a
    // retraction every one is, as the dels it published said they were gone.
    changes.extend(
        lines
            .iter()
            .filter(|line| old.retracted || !old.lines.iter().any(|old| same_reference(old, line)))
            .cloned(),
    );
    // A post's first stage writes its revision even with nothing to publish: from then on its
    // external reply parent is fixed (the watcher's parent check reads the revision).
    let touched = !old.lines.is_empty() || !lines.is_empty();
    if changes.is_empty() && !old.retracted && old.reconciled {
        return Ok(Staged {
            touched,
            outbox: None,
        });
    }
    let outbox = (!changes.is_empty())
        .then(|| serde_json::to_string(&changes))
        .transpose()
        .map_err(ModelError::from_generic)?;
    let revision = pubky::Keypair::random().public_key().to_z32();
    let mut query = String::from(
        r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        SET p.resource_ref_revision = coalesce(p.resource_ref_revision, '')
        WITH p WHERE p.content = $content AND p.kind = $kind AND NOT coalesce(p.deleted, false)
          AND p.resource_ref_revision = $previous
        SET p.resource_ref_revision = $revision, p.resource_ref_outbox = $outbox
        REMOVE p.resource_ref_retracted
        WITH p
    ",
    );
    let mut groups = Vec::new();
    for (kind, edge, name) in [
        (ResourceChangeKind::Reply, "REPLIED", "replies"),
        (ResourceChangeKind::Embed, "EMBEDS", "embeds"),
        (ResourceChangeKind::Collection, "COLLECTED", "collections"),
    ] {
        let values: Vec<HashMap<String, String>> = lines
            .iter()
            .filter(|line| line.kind == kind)
            .map(|line| {
                let (_, scheme) = normalize_uri(&line.uri).expect("normalized above");
                HashMap::from([
                    ("id".into(), line.resource_id.clone()),
                    ("uri".into(), line.uri.clone()),
                    ("scheme".into(), scheme),
                    ("path".into(), line.object_path.clone()),
                ])
            })
            .collect();
        query.push_str(&format!(
            r"
            FOREACH (item IN ${name} |
                MERGE (r:Resource {{id: item.id}})
                ON CREATE SET r.uri = item.uri, r.scheme = item.scheme, r.indexed_at = $at
                MERGE (p)-[e:{edge}]->(r)
                ON CREATE SET e.indexed_at = $at
                SET e.object_path = item.path
            )
            WITH p
            CALL (p) {{ MATCH (p)-[e:{edge}]->(r:Resource)
                WHERE NOT r.id IN [item IN ${name} | item.id] DELETE e }}
        "
        ));
        groups.push((name, values));
    }
    // Remove orphans in the edge transaction: retry cannot forget an old target.
    query.push_str(
        r"
        CALL () {
            UNWIND $old_targets AS id MATCH (r:Resource {id: id})
            WITH DISTINCT r WHERE NOT EXISTS { (r)--() } DELETE r
        }
        RETURN p.id AS id
    ",
    );
    let kind = serde_json::to_string(&post.kind).map_err(ModelError::from_generic)?;
    let mut query = Query::new("resource_references_sync", query)
        .param("author", post.author.clone())
        .param("id", post.id.clone())
        .param("content", post.content.clone())
        .param("kind", kind.trim_matches('"'))
        .param(
            "old_targets",
            old.lines
                .iter()
                .map(|line| line.resource_id.clone())
                .collect::<Vec<_>>(),
        )
        .param("previous", old.revision)
        .param("revision", revision)
        .param("outbox", outbox.clone())
        .param("at", at);
    for (name, values) in groups {
        query = query.param(name, values);
    }
    if fetch_row_from_graph(query).await?.is_none() {
        return Err(ModelError::StaleWrite(
            "Post changed while reconciling external references".into(),
        ));
    }
    Ok(Staged { touched, outbox })
}

/// Retracts the post's references on the feed before its delete changes the graph, as a tag's
/// delete does: the del of each of its Resource edges, once a staged outbox is published, so
/// the dels follow its puts. A tombstone keeps its edges and retracted them when it became one,
/// so its later hard delete appends nothing. A delete that fails after this append leaves the
/// post, and its retry reads the edges again and appends their dels again: consumers tolerate
/// duplicates. The post is marked retracted first, so the next [`stage`] puts all its edges
/// again: a PUT after a delete that stopped half-way, or one that brings a tombstone back.
///
/// It reads the post's references and outbox, like [`stage`]: call it under the post lock.
pub async fn retract(author: &str, id: &str) -> ModelResult<()> {
    flush(author, id).await?;
    let State {
        deleted, mut lines, ..
    } = state(author, id).await?;
    if deleted || lines.is_empty() {
        return Ok(());
    }
    // Marked before the append: a PUT after a delete that stopped anywhere past here puts the
    // references again, as the graph still holds them.
    get_neo4j_graph()?
        .run(
            Query::new(
                "resource_references_retracted",
                r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        SET p.resource_ref_retracted = true
    ",
            )
            .param("author", author)
            .param("id", id),
        )
        .await
        .map_err(GraphError::from)?;
    let at = chrono::Utc::now().timestamp_millis();
    for line in &mut lines {
        line.op = ResourceChangeOp::Del;
        line.at = at;
    }
    ResourceChange::store_batch(&lines).await?;
    Ok(())
}

/// Existence check without hydrating tags or reference counts.
pub async fn exists(id: &str) -> ModelResult<bool> {
    Ok(fetch_row_from_graph(
        Query::new(
            "resource_reference_exists",
            "MATCH (:Resource {id: $id}) RETURN 1 AS present",
        )
        .param("id", id),
    )
    .await?
    .is_some())
}

/// Indexed Resource lookup, stable newest-first pagination over live source posts.
pub async fn posts(
    id: &str,
    kind: ResourceChangeKind,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<(String, String)>> {
    let edge = match kind {
        ResourceChangeKind::Reply => "REPLIED",
        ResourceChangeKind::Embed => "EMBEDS",
        ResourceChangeKind::Collection => "COLLECTED",
        _ => return Err(ModelError::InvalidInput("Not a post reference".into())),
    };
    let mut rows = get_neo4j_graph()?
        .execute(
            Query::new(
                "resource_reference_posts",
                format!(
                    r"
        MATCH (r:Resource {{id: $id}})<-[:{edge}]-(p:Post)<-[:AUTHORED]-(a:User)
        WHERE NOT coalesce(p.deleted, false)
        RETURN a.id AS author, p.id AS id ORDER BY p.indexed_at DESC, a.id, p.id
        SKIP $skip LIMIT $limit
    "
                ),
            )
            .param("id", id)
            .param("skip", skip as i64)
            .param("limit", limit as i64),
        )
        .await
        .map_err(GraphError::from)?;
    let mut keys = vec![];
    while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
        keys.push((row.get("author")?, row.get("id")?));
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StackConfig, StackManager};

    /// A user with one social Short post in the graph, as the post write leaves it.
    async fn author_with_post(content: &str) -> Result<PostDetails, crate::types::DynError> {
        let author = pubky::Keypair::random().public_key().to_z32();
        let id = format!("{:013X}", chrono::Utc::now().timestamp_micros());
        get_neo4j_graph()?
            .run(
                Query::new("reference_test_user", "CREATE (:User {id:$author})")
                    .param("author", author.clone()),
            )
            .await?;
        let post = PostDetails {
            uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
            id,
            author,
            content: content.into(),
            indexed_at: 1234,
            kind: crate::models::post::PostKind::Short,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;
        Ok(post)
    }

    /// Deletes `posts` as a post delete does, so the Resources only they reference go with
    /// them, then their author.
    async fn delete_posts_and_author(
        author: &str,
        posts: &[&str],
    ) -> Result<(), crate::types::DynError> {
        for id in posts {
            get_neo4j_graph()?
                .run(crate::db::graph::queries::del::delete_post(author, id))
                .await?;
        }
        get_neo4j_graph()?
            .run(
                Query::new(
                    "reference_test_user_cleanup",
                    "MATCH (u:User {id:$author}) DETACH DELETE u",
                )
                .param("author", author.to_string()),
            )
            .await?;
        Ok(())
    }

    /// `author`'s lines appended since `cursor`, as (op, resource id, object path, at).
    async fn own_lines_since(
        author: &str,
        cursor: u64,
    ) -> Result<Vec<(ResourceChangeOp, String, String, i64)>, crate::types::DynError> {
        Ok(ResourceChange::get_from_index(cursor, 100_000)
            .await?
            .0
            .lines
            .into_iter()
            .filter(|line| line.author == author)
            .map(|line| (line.op, line.resource_id, line.object_path, line.at))
            .collect())
    }

    /// The same PUT handled twice appends its references once: the replay finds the edges it
    /// would write and publishes nothing. A later version file that names the same target
    /// moves the edge to its path, publishes the change of key, and keeps the time the edge
    /// was first indexed, in the graph and on the line.
    #[tokio_shared_rt::test(shared)]
    async fn a_replayed_put_appends_nothing_and_an_edge_keeps_its_first_time(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let post = author_with_post("replay fixture").await?;
        let (author, id) = (post.author.clone(), post.id.clone());
        let target = format!("https://example.com/{author}/replayed");
        let first_source = format!("{}/{id}.json", post.uri);
        let line = |source: &str| reference(&author, source, &target, ResourceChangeKind::Reply);
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        sync(&post, vec![line(&first_source)?]).await?;
        let first = own_lines_since(&author, cursor).await?;
        assert_eq!(first.len(), 1, "{first:?}");
        let first_at = state(&author, &id).await?.lines[0].at;

        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        sync(&post, vec![line(&first_source)?]).await?;
        assert_eq!(
            own_lines_since(&author, cursor).await?,
            [],
            "a replay appends nothing"
        );

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second_source = format!("{}/0033SSE3B1FQ0.json", post.uri);
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        sync(&post, vec![line(&second_source)?]).await?;
        let edges = state(&author, &id).await?.lines;
        assert_eq!(edges.len(), 1);
        let path = |source: &str| {
            source
                .strip_prefix(&format!("pubky://{author}"))
                .unwrap()
                .to_string()
        };
        assert_eq!(edges[0].object_path, path(&second_source));
        assert_eq!(edges[0].at, first_at, "the edge keeps its first time");
        let moved = own_lines_since(&author, cursor).await?;
        assert_eq!(
            moved
                .iter()
                .map(|(op, _, object_path, _)| (*op, object_path.clone()))
                .collect::<Vec<_>>(),
            [
                (ResourceChangeOp::Del, path(&first_source)),
                (ResourceChangeOp::Put, path(&second_source))
            ]
        );
        assert_eq!(moved[1].3, first_at, "the put carries the edge's time");

        retract(&author, &id).await?;
        delete_posts_and_author(&author, &[&id]).await
    }

    /// A stage whose post no longer matches the graph (another version's content, written since
    /// its read) is refused whole: a typed error the watcher retries, and no edge, Resource or
    /// outbox written.
    #[tokio_shared_rt::test(shared)]
    async fn a_stage_for_content_the_graph_no_longer_has_is_refused_and_writes_nothing(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let post = author_with_post("the graph's content").await?;
        let (author, id) = (post.author.clone(), post.id.clone());
        let source = format!("{}/{id}.json", post.uri);
        let kept = reference(
            &author,
            &source,
            &format!("https://example.com/{author}/kept"),
            ResourceChangeKind::Reply,
        )?;
        sync(&post, vec![kept.clone()]).await?;
        let before = state(&author, &id).await?;

        let stale = PostDetails {
            content: "an older version's content".into(),
            ..post.clone()
        };
        let refused = reference(
            &author,
            &source,
            &format!("https://example.com/{author}/refused"),
            ResourceChangeKind::Embed,
        )?;
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        let result = stage(&stale, vec![refused.clone()]).await;
        assert!(
            matches!(result, Err(ModelError::StaleWrite(_))),
            "{result:?}"
        );
        let after = state(&author, &id).await?;
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.outbox, None, "no outbox");
        assert_eq!(
            after
                .lines
                .iter()
                .map(|line| (line.kind, line.resource_id.clone()))
                .collect::<Vec<_>>(),
            [(ResourceChangeKind::Reply, kept.resource_id.clone())],
            "the edges are the graph's"
        );
        assert!(!exists(&refused.resource_id).await?, "no Resource");
        assert_eq!(own_lines_since(&author, cursor).await?, []);

        retract(&author, &id).await?;
        delete_posts_and_author(&author, &[&id]).await
    }

    #[tokio_shared_rt::test(shared)]
    async fn edited_orphans_are_revision_safe_and_a_delete_retracts_the_current_edges(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        let id = format!("{:013X}", chrono::Utc::now().timestamp_micros());
        get_neo4j_graph()?
            .run(
                Query::new("reference_revision_user", "CREATE (:User {id:$author})")
                    .param("author", author.clone()),
            )
            .await?;
        let post = PostDetails {
            id: id.clone(),
            author: author.clone(),
            uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
            content: "revision fixture".into(),
            indexed_at: 1234,
            kind: crate::models::post::PostKind::Short,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;
        let source = format!("{}/{id}.json", post.uri);
        let first = reference(
            &author,
            &source,
            &format!("https://example.com/{author}/old"),
            ResourceChangeKind::Reply,
        )?;
        let second = reference(
            &author,
            &source,
            &format!("https://example.com/{author}/new"),
            ResourceChangeKind::Reply,
        )?;
        sync(&post, vec![first.clone()]).await?;
        // An edit commits its edges and outbox, without its append yet
        stage(&post, vec![second.clone()]).await?;
        assert!(
            !exists(&first.resource_id).await?,
            "orphan cleanup commits with edges, before append"
        );
        assert!(exists(&second.resource_id).await?);
        // A delete retracts what the graph holds now, after the edit's outbox
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        retract(&author, &id).await?;
        let lines: Vec<_> = ResourceChange::get_from_index(cursor, 1000)
            .await?
            .0
            .lines
            .into_iter()
            .filter(|line| line.author == author)
            .map(|line| (line.op, line.resource_id))
            .collect();
        assert_eq!(
            lines,
            [
                (ResourceChangeOp::Del, first.resource_id.clone()),
                (ResourceChangeOp::Put, second.resource_id.clone()),
                (ResourceChangeOp::Del, second.resource_id.clone()),
            ],
            "the edit's outbox, then the dels of the current edges"
        );
        assert_eq!(
            posts(&second.resource_id, ResourceChangeKind::Reply, 0, 10).await?,
            [(author.clone(), id.clone())],
            "the retraction leaves the graph to the delete"
        );
        // A retry after the committed edit cannot resurrect the removed target.
        sync(&post, vec![second.clone()]).await?;
        assert!(!exists(&first.resource_id).await?);
        assert!(PostDetails::is_reply_in_graph(&author, &id).await?);
        sync(&post, vec![]).await?;
        assert!(!exists(&second.resource_id).await?);
        assert!(!PostDetails::is_reply_in_graph(&author, &id).await?);
        delete_posts_and_author(&author, &[&id]).await
    }

    #[tokio_shared_rt::test(shared)]
    async fn live_reseed_restores_each_collection_reference() -> Result<(), crate::types::DynError>
    {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        let identifier = format!("{:013X}", chrono::Utc::now().timestamp_micros());
        let id = identifier.as_str();
        get_neo4j_graph()?
            .run(
                Query::new("reference_seed_user", "CREATE (:User {id:$author})")
                    .param("author", author.clone()),
            )
            .await?;
        let post = PostDetails {
            id: id.into(),
            author: author.clone(),
            uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
            content: "collection fixture".into(),
            indexed_at: 1234,
            kind: crate::models::post::PostKind::Collection,
            attachments: vec![],
            lock: None,
            deleted: false,
        };
        post.put_to_graph(None, None).await?;
        let source = format!("{}/{id}.json", post.uri);
        let lines = ["one", "two"]
            .iter()
            .map(|suffix| {
                reference(
                    &author,
                    &source,
                    &format!("https://example.com/{author}/{suffix}"),
                    ResourceChangeKind::Collection,
                )
            })
            .collect::<ModelResult<Vec<_>>>()?;
        stage(&post, lines.clone()).await?;
        let epoch = ResourceChange::get_from_index(0, 0).await?.1;
        ResourceChange::reseed(false).await?;
        let (page, next_epoch, _) = ResourceChange::get_from_index(0, 1000).await?;
        assert_ne!(epoch, next_epoch);
        let own: Vec<_> = page
            .lines
            .iter()
            .filter(|line| line.author == author)
            .collect();
        assert_eq!(own.len(), 2);
        assert!(own
            .iter()
            .all(|line| line.kind == ResourceChangeKind::Collection
                && line.object_path == source.strip_prefix(&format!("pubky://{author}")).unwrap()));
        for expected in &lines {
            assert!(own
                .iter()
                .any(|line| line.resource_id == expected.resource_id));
        }
        // Even after a seed, retrying the committed outbox appends harmless duplicates.
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        flush(&author, id).await?;
        let replayed = ResourceChange::get_from_index(cursor, 1000).await?.0.lines;
        assert_eq!(replayed.len(), lines.len());
        for expected in &lines {
            assert!(replayed
                .iter()
                .any(|line| same_reference(line, expected) && line.op == ResourceChangeOp::Put));
        }
        retract(&author, id).await?;
        delete_posts_and_author(&author, &[id]).await?;
        for expected in &lines {
            assert!(!exists(&expected.resource_id).await?, "no Resource is left");
        }
        Ok(())
    }

    // Runs exclusively via nextest: it replaces the shared feed.
    #[tokio_shared_rt::test(shared)]
    async fn live_reseed_restores_replies_and_embeds_past_one_batch(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        let graph = get_neo4j_graph()?;
        graph
            .run(
                Query::new("reference_seed_user", "CREATE (:User {id:$author})")
                    .param("author", author.clone()),
            )
            .await?;
        let stamp = chrono::Utc::now().timestamp_micros();
        let mut posts = Vec::new();
        for (n, kind) in [
            crate::models::post::PostKind::Short,
            crate::models::post::PostKind::Collection,
        ]
        .into_iter()
        .enumerate()
        {
            let id = format!("{:013X}", stamp + 2 * n as i64);
            let post = PostDetails {
                uri: format!("pubky://{author}/pub/social/v1/posts/{id}"),
                id,
                author: author.clone(),
                content: "seed fixture".into(),
                indexed_at: 1234,
                kind,
                attachments: vec![],
                lock: None,
                deleted: false,
            };
            post.put_to_graph(None, None).await?;
            posts.push(post);
        }
        // A reply and an embed of one post
        let short = &posts[0];
        let source = format!("{}/{}.json", short.uri, short.id);
        // Each on its own target, so a seed that swapped the two kinds is told apart
        let lines = [
            (ResourceChangeKind::Reply, "replied"),
            (ResourceChangeKind::Embed, "embedded"),
        ]
        .into_iter()
        .map(|(kind, target)| {
            reference(
                &author,
                &source,
                &format!("https://example.com/{author}/{target}"),
                kind,
            )
        })
        .collect::<ModelResult<Vec<_>>>()?;
        stage(short, lines.clone()).await?;
        // And more collection items than a seed batch holds
        let collection = &posts[1];
        let items = 1001;
        let items_path = format!(
            "{}/{}.json",
            collection
                .uri
                .strip_prefix(&format!("pubky://{author}"))
                .unwrap(),
            collection.id
        );
        graph
            .run(
                Query::new(
                    "reference_seed_items",
                    r"
            MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
            UNWIND range(0, $items - 1) AS i
            CREATE (r:Resource {id: $author + ':' + toString(i), scheme: 'https',
                uri: 'https://example.com/' + $author + '/item/' + toString(i)})
            CREATE (p)-[:COLLECTED {object_path: $path, indexed_at: 2000 + i}]->(r)
        ",
                )
                .param("id", collection.id.clone())
                .param("author", author.clone())
                .param("items", items)
                .param("path", items_path.clone()),
            )
            .await?;

        ResourceChange::reseed(false).await?;
        let (page, _, _) = ResourceChange::get_from_index(0, usize::MAX).await?;
        let own: Vec<_> = page
            .lines
            .iter()
            .filter(|line| line.author == author)
            .collect();
        assert_eq!(own.len(), lines.len() + items as usize);
        for expected in &lines {
            let found: Vec<_> = own
                .iter()
                .filter(|line| line.kind == expected.kind)
                .collect();
            assert_eq!(found.len(), 1, "{:?}: {found:?}", expected.kind);
            assert!(same_reference(found[0], expected), "{found:?}");
        }
        let collected: std::collections::BTreeSet<_> = own
            .iter()
            .filter(|line| {
                line.kind == ResourceChangeKind::Collection && line.object_path == items_path
            })
            .map(|line| line.resource_id.as_str())
            .collect();
        assert_eq!(collected.len(), items as usize, "each item once");

        retract(&author, &short.id).await?;
        graph
            .run(
                Query::new(
                    "reference_seed_items_cleanup",
                    "MATCH (r:Resource) WHERE r.id STARTS WITH $author + ':' DETACH DELETE r",
                )
                .param("author", author.clone()),
            )
            .await?;
        let ids: Vec<&str> = posts.iter().map(|post| post.id.as_str()).collect();
        delete_posts_and_author(&author, &ids).await?;
        for expected in &lines {
            assert!(!exists(&expected.resource_id).await?, "no Resource is left");
        }
        ResourceChange::reseed(false).await?;
        Ok(())
    }
}
