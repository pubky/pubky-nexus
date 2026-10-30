//! References from social posts to universal Resources. Content stays on the Post.

use crate::db::graph::{GraphError, Query};
use crate::db::kv::RedisError;
use crate::db::{fetch_row_from_graph, get_neo4j_graph, get_redis_conn};
use crate::models::error::{ModelError, ModelResult};
use crate::models::event::resource_changes::{
    ResourceChange, ResourceChangeKind, ResourceChangeOp,
};
use crate::models::post::PostDetails;
use crate::universal_tag::normalize::{normalize_uri, resource_id};
use deadpool_redis::redis::AsyncCommands;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
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
    external_reply: bool,
    outbox: Option<String>,
    lines: Vec<ResourceChange>,
}

async fn state(author: &str, id: &str) -> ModelResult<State> {
    let row = fetch_row_from_graph(Query::new("resource_references_state", r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        OPTIONAL MATCH (p)-[e:REPLIED|EMBEDS|COLLECTED]->(r:Resource)
        WITH p, e, r ORDER BY type(e), r.id
        RETURN coalesce(p.resource_ref_revision, '') AS revision,
               coalesce(p.resource_reply, false) AS external_reply,
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
            external_reply: row.get("external_reply")?,
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
    let Some(raw) = state(author, id).await?.outbox else {
        return Ok(());
    };
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
/// source Post. A stale recovery cannot replace a newer post's references.
pub async fn sync(post: &PostDetails, lines: Vec<ResourceChange>) -> ModelResult<()> {
    stage(post, lines).await?;
    flush(&post.author, &post.id).await
}

/// Commit edges and recovery metadata, without acknowledging the feed append.
pub async fn stage(post: &PostDetails, mut lines: Vec<ResourceChange>) -> ModelResult<()> {
    flush(&post.author, &post.id).await?;
    let old = state(&post.author, &post.id).await?;
    let at = chrono::Utc::now().timestamp_millis();
    for line in &mut lines {
        line.at = at;
    }
    lines.sort_by_key(|line| (format!("{:?}", line.kind), line.resource_id.clone()));
    lines.dedup_by(|a, b| same_reference(a, b));
    if old.lines.is_empty() && lines.is_empty() && !old.external_reply {
        return Ok(());
    }
    let mut changes: Vec<_> = old
        .lines
        .iter()
        .filter(|old| !lines.iter().any(|line| same_reference(old, line)))
        .cloned()
        .map(|mut line| {
            line.op = ResourceChangeOp::Del;
            line.at = chrono::Utc::now().timestamp_millis();
            line
        })
        .collect();
    // Always publish the accepted PUT, including replay after a successful graph write.
    changes.extend(lines.iter().cloned());
    let outbox = serde_json::to_string(&changes).map_err(ModelError::from_generic)?;
    let revision = pubky::Keypair::random().public_key().to_z32();
    let mut query = String::from(
        r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        SET p.resource_ref_revision = coalesce(p.resource_ref_revision, '')
        WITH p WHERE p.content = $content AND p.kind = $kind AND NOT coalesce(p.deleted, false)
          AND p.resource_ref_revision = $previous
        SET p.resource_ref_revision = $revision, p.resource_ref_outbox = $outbox,
            p.resource_reply = $is_reply
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
                SET e.object_path = item.path, e.indexed_at = $at
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
        .param(
            "is_reply",
            lines
                .iter()
                .any(|line| line.kind == ResourceChangeKind::Reply),
        )
        .param("previous", old.revision)
        .param("revision", revision)
        .param("outbox", outbox)
        .param("at", at);
    for (name, values) in groups {
        query = query.param(name, values);
    }
    if fetch_row_from_graph(query).await?.is_none() {
        return Err(ModelError::from_generic(
            "Post changed while reconciling external references; retry",
        ));
    }
    Ok(())
}

fn delete_key(author: &str, id: &str) -> String {
    format!("Event:ResourceChanges:PostDelete:{author}:{id}")
}

#[derive(Serialize, Deserialize)]
struct DeleteJournal {
    revision: String,
    lines: Vec<ResourceChange>,
}

/// Retain all targets and their revision before a hard delete removes the graph source.
pub async fn prepare_delete(author: &str, id: &str) -> ModelResult<()> {
    flush(author, id).await?;
    let State {
        revision,
        mut lines,
        ..
    } = state(author, id).await?;
    if lines.is_empty() {
        return Ok(());
    }
    for line in &mut lines {
        line.op = ResourceChangeOp::Del;
        line.at = chrono::Utc::now().timestamp_millis();
    }
    let raw = serde_json::to_string(&DeleteJournal { revision, lines })
        .map_err(ModelError::from_generic)?;
    get_redis_conn()
        .await?
        .set_nx::<_, _, bool>(delete_key(author, id), raw)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}

pub async fn finish_delete(author: &str, id: &str) -> ModelResult<()> {
    let key = delete_key(author, id);
    let mut conn = get_redis_conn().await?;
    let Some(raw): Option<String> = conn.get(&key).await.map_err(RedisError::from)? else {
        return Ok(());
    };
    finish_delete_journal(author, id, &key, &raw).await
}

async fn finish_delete_journal(author: &str, id: &str, key: &str, raw: &str) -> ModelResult<()> {
    let DeleteJournal { revision, lines } =
        serde_json::from_str(raw).map_err(ModelError::from_generic)?;
    // Lock the source before checking its revision. A delayed delete must not
    // remove the Resource edges (or cache) installed by a later accepted PUT.
    // An absent source is a hard delete whose journal still needs publication.
    let applied = fetch_row_from_graph(
        Query::new(
            "resource_references_delete",
            r"
        OPTIONAL MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        SET p.resource_ref_revision = coalesce(p.resource_ref_revision, '')
        WITH p WHERE p IS NULL OR p.resource_ref_revision = $revision
        CALL (p) {
            MATCH (p)-[e:REPLIED|EMBEDS|COLLECTED]->(:Resource) DELETE e
        }
        RETURN true AS applied
    ",
        )
        .param("author", author)
        .param("id", id)
        .param("revision", revision),
    )
    .await?
    .is_some();
    if !applied {
        // Compare-and-delete only this stale journal, without publishing stale DELs.
        return ResourceChange::finish_batch_delete(key, raw, &[]).await;
    }
    cleanup(&lines).await?;
    crate::models::post::PostRelationships::delete(author, id).await?;
    ResourceChange::finish_batch_delete(key, raw, &lines).await
}

/// Reply classification survives removal of an external edge on a tombstone.
/// The edge fallback covers posts indexed before the property was introduced.
pub async fn is_reply(author: &str, id: &str) -> ModelResult<bool> {
    Ok(fetch_row_from_graph(
        Query::new(
            "resource_reply_classification",
            r"
        MATCH (p:Post {id: $id}) WHERE EXISTS { (:User {id: $author})-[:AUTHORED]->(p) }
        RETURN coalesce(p.resource_reply, false) OR EXISTS { (p)-[:REPLIED]->() } AS is_reply
    ",
        )
        .param("author", author)
        .param("id", id),
    )
    .await?
    .map(|row| row.get("is_reply"))
    .transpose()?
    .unwrap_or(false))
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

async fn cleanup(lines: &[ResourceChange]) -> ModelResult<()> {
    let ids: Vec<_> = lines.iter().map(|line| line.resource_id.clone()).collect();
    get_neo4j_graph()?
        .run(
            Query::new(
                "resource_reference_orphans",
                r"
        UNWIND $ids AS id MATCH (r:Resource {id: id})
        WITH DISTINCT r WHERE NOT EXISTS { (r)--() } DELETE r
    ",
            )
            .param("ids", ids),
        )
        .await
        .map_err(GraphError::from)?;
    Ok(())
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

/// Live Collection posts that list the Resource `id`, newest first with stable author/id ties,
/// optionally only those curated by `curator`. The Resource-anchored counterpart of the post
/// stream's `post_collections` source.
pub async fn collections(
    id: &str,
    curator: Option<&str>,
    skip: usize,
    limit: usize,
) -> ModelResult<Vec<(String, String)>> {
    let mut rows = get_neo4j_graph()?
        .execute(
            Query::new(
                "resource_collections",
                r"
        MATCH (r:Resource {id: $id})<-[:COLLECTED]-(p:Post)<-[:AUTHORED]-(a:User)
        WHERE NOT coalesce(p.deleted, false) AND ($curator IS NULL OR a.id = $curator)
        RETURN a.id AS author, p.id AS id ORDER BY p.indexed_at DESC, a.id, p.id
        SKIP $skip LIMIT $limit
    ",
            )
            .param("id", id)
            .param("curator", curator.map(str::to_string))
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

    #[tokio_shared_rt::test(shared)]
    async fn edited_orphans_and_delayed_delete_are_revision_safe(
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
            attachments: Some(vec![]),
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
        prepare_delete(&author, &id).await?;
        let key = delete_key(&author, &id);
        let raw: String = get_redis_conn().await?.get(&key).await?;
        // A delete worker has read this journal; pause it while an edit commits.
        stage(&post, vec![second.clone()]).await?;
        assert!(
            !exists(&first.resource_id).await?,
            "orphan cleanup commits with edges, before append"
        );
        assert!(exists(&second.resource_id).await?);
        flush(&author, &id).await?;
        let (_, _, cursor) = ResourceChange::get_from_index(0, 0).await?;
        finish_delete_journal(&author, &id, &key, &raw).await?;
        assert_eq!(
            posts(&second.resource_id, ResourceChangeKind::Reply, 0, 10).await?,
            [(author.clone(), id.clone())]
        );
        assert!(!get_redis_conn().await?.exists::<_, bool>(&key).await?);
        assert!(ResourceChange::get_from_index(cursor, 1000)
            .await?
            .0
            .lines
            .iter()
            .all(|line| line.author != author));
        // A retry after the committed edit cannot resurrect the removed target.
        sync(&post, vec![second.clone()]).await?;
        assert!(!exists(&first.resource_id).await?);
        assert!(is_reply(&author, &id).await?);
        sync(&post, vec![]).await?;
        assert!(!exists(&second.resource_id).await?);
        assert!(!is_reply(&author, &id).await?);
        get_neo4j_graph()?.run(Query::new("reference_revision_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p,u").param("author", author)).await?;
        Ok(())
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
            attachments: Some(vec![]),
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
        ResourceChange::reseed().await?;
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
        prepare_delete(&author, id).await?;
        finish_delete(&author, id).await?;
        get_neo4j_graph()?.run(Query::new("reference_seed_cleanup", "MATCH (u:User {id:$author}) OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post) DETACH DELETE p,u").param("author", author)).await?;
        Ok(())
    }
}
