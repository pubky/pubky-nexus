//! Durable resource references. Reads never create or repair feed state.
//!
//! Delivery is at least once: consumers upsert/delete by `(author, object_path)`.
//! Rebuilds preserve live appends made while the graph snapshot is assembled.

use crate::db::graph::{GraphError, Query};
use crate::db::kv::{RedisError, RedisResult};
use crate::db::{get_neo4j_graph, get_redis_conn, release_lock, try_acquire_lock};
use crate::models::error::{ModelError, ModelResult};
use deadpool_redis::redis::{self, AsyncCommands, Script};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;
use utoipa::ToSchema;

pub const RESOURCE_CHANGES_KEY: &str = "Event:ResourceChanges";
const EPOCH_KEY: &str = "Event:ResourceChanges:Epoch";
const SEED_LOCK: &str = "Event:ResourceChanges:SeedLock";
const SEED_TTL: u64 = 3660;
const BATCH_SIZE: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChangeOp {
    Put,
    Del,
}

/// Only `tag` is emitted today. `reply`, `embed` and `collection` are reserved for the other
/// references a post makes to a URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChangeKind {
    Tag,
    Reply,
    Embed,
    Collection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ResourceChange {
    pub op: ResourceChangeOp,
    /// The Resource node's ID: `universal_tag::normalize::resource_id` of the normalized URI.
    pub resource_id: String,
    /// Normalized target URI; Nexus does not fetch it.
    pub uri: String,
    pub kind: ResourceChangeKind,
    pub author: String,
    /// Present for tag references.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The homeserver path of the tag, reply or collection file that names the URI, as
    /// written. Replay key is (author, object_path), since paths repeat across authors.
    pub object_path: String,
    /// Indexed timestamp in Unix milliseconds (deletion time for del).
    pub at: i64,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ResourceChangesPage {
    pub lines: Vec<ResourceChange>,
    pub next_cursor: u64,
}

// A random 48-bit initial counter avoids reusing a feed identity after Redis is
// cleared. INCR on reseed remains exact in Redis Lua's numeric representation.
fn epoch_seed() -> u64 {
    let token = pubky::Keypair::random().public_key().to_z32();
    let digest = blake3::hash(token.as_bytes());
    let mut bytes = [0; 8];
    bytes[..6].copy_from_slice(&digest.as_bytes()[..6]);
    u64::from_le_bytes(bytes)
}

fn pending_key(author: &str, object_path: &str) -> String {
    format!("Event:ResourceChanges:PendingDelete:{author}:{object_path}")
}

fn object_path<'a>(author: &str, tag_uri: &'a str) -> ModelResult<&'a str> {
    tag_uri
        .strip_prefix(&format!("pubky://{author}"))
        .filter(|path| path.starts_with("/pub/"))
        .ok_or_else(|| ModelError::from_generic("Invalid tag storage URI"))
}

static APPEND: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"
    redis.call('SETNX', KEYS[2], ARGV[2])
    redis.call('RPUSH', KEYS[1], ARGV[1])
    redis.call('DEL', KEYS[3])
",
    )
});

static FINISH_DELETE: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"
    local line = redis.call('GET', KEYS[3])
    if line then
        redis.call('SETNX', KEYS[2], ARGV[1])
        redis.call('RPUSH', KEYS[1], line)
        redis.call('DEL', KEYS[3])
    end
",
    )
});

static READ: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"
    return {redis.call('GET', KEYS[2]) or false,
            redis.call('LLEN', KEYS[1]),
            redis.call('LRANGE', KEYS[1], ARGV[1], ARGV[2])}
",
    )
});

impl ResourceChange {
    #[allow(clippy::too_many_arguments)]
    pub fn tag(
        op: ResourceChangeOp,
        resource_id: String,
        uri: String,
        author: String,
        label: String,
        tag_uri: &str,
        at: i64,
    ) -> ModelResult<Self> {
        let object_path = object_path(&author, tag_uri)?.to_owned();
        Ok(Self {
            op,
            resource_id,
            uri,
            kind: ResourceChangeKind::Tag,
            author,
            label: Some(label),
            object_path,
            at,
        })
    }

    fn json(&self) -> RedisResult<String> {
        serde_json::to_string(self).map_err(|e| RedisError::SerializationFailed(Box::new(e)))
    }

    /// Call only after the graph write has committed. Replayed puts may append
    /// again. A later accepted put supersedes any unfinished delete of its path.
    pub async fn store(&self) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        APPEND
            .key(RESOURCE_CHANGES_KEY)
            .key(EPOCH_KEY)
            .key(pending_key(&self.author, &self.object_path))
            .arg(self.json()?)
            .arg(epoch_seed())
            .invoke_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    /// Persist delete details before removing the graph edge, which is the only
    /// source of its target URI and label. Retry preserves the first timestamp.
    pub async fn prepare_delete(&self) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        let _: bool = conn
            .set_nx(pending_key(&self.author, &self.object_path), self.json()?)
            .await?;
        Ok(())
    }

    /// Call after graph deletion, including a retry that finds no edge. Append
    /// and pending-record removal are atomic, so a failed acknowledgement is safe.
    pub async fn finish_delete(author: &str, tag_uri: &str) -> ModelResult<()> {
        let path = object_path(author, tag_uri)?;
        let mut conn = get_redis_conn().await?;
        FINISH_DELETE
            .key(RESOURCE_CHANGES_KEY)
            .key(EPOCH_KEY)
            .key(pending_key(author, path))
            .arg(epoch_seed())
            .invoke_async::<()>(&mut conn)
            .await
            .map_err(RedisError::from)?;
        Ok(())
    }

    /// Atomically read the page, epoch and length from one list generation.
    /// No SETNX or other write occurs here, including on an empty instance.
    pub async fn get_from_index(
        cursor: u64,
        limit: usize,
    ) -> RedisResult<(ResourceChangesPage, Option<String>, u64)> {
        let start = cursor.min(i64::MAX as u64);
        let end = start
            .saturating_add(limit.saturating_sub(1) as u64)
            .min(i64::MAX as u64);
        let mut conn = get_redis_conn().await?;
        let (epoch, length, raw): (Option<String>, u64, Vec<String>) = READ
            .key(RESOURCE_CHANGES_KEY)
            .key(EPOCH_KEY)
            .arg(if limit == 0 { 1 } else { start })
            .arg(if limit == 0 { 0 } else { end })
            .invoke_async(&mut conn)
            .await?;
        let lines = raw
            .into_iter()
            .map(|line| {
                serde_json::from_str(&line)
                    .map_err(|e| RedisError::DeserializationFailed(Box::new(e)))
            })
            .collect::<RedisResult<Vec<_>>>()?;
        let next_cursor = cursor.saturating_add(lines.len() as u64);
        Ok((ResourceChangesPage { lines, next_cursor }, epoch, length))
    }

    /// Rebuild from graph edges, then atomically publish with a new epoch.
    /// The old list's suffix is copied after the snapshot, preserving concurrent
    /// watcher appends (duplicates are resolved by the consumer's replay key).
    pub async fn reseed() -> ModelResult<u64> {
        let token = pubky::Keypair::random().public_key().to_z32();
        if !try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await? {
            return Err(ModelError::from_generic(
                "Resource changes seed already running",
            ));
        }
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        let result = tokio::time::timeout(Duration::from_secs(3600), seed_inner(&token, &temp))
            .await
            .map_err(ModelError::from_generic)
            .and_then(|r| r);
        // Aborted processes leave only expiring temporary data and the old feed.
        if let Ok(mut conn) = get_redis_conn().await {
            let _ = conn.del::<_, ()>(&temp).await;
        }
        release_lock(SEED_LOCK, &token).await?;
        result
    }
}

async fn seed_inner(token: &str, temp: &str) -> ModelResult<u64> {
    let mut conn = get_redis_conn().await?;
    let start: u64 = conn
        .llen(RESOURCE_CHANGES_KEY)
        .await
        .map_err(RedisError::from)?;
    let graph = get_neo4j_graph()?;
    let mut rows = graph
        .execute(Query::new(
            "resource_changes_seed",
            r"
        MATCH (u:User)-[t:TAGGED]->(r:Resource)
        RETURN u.id AS author, r.id AS resource_id, r.uri AS uri,
               t.label AS label, t.indexed_at AS indexed_at,
               coalesce(t.uri, 'pubky://' + u.id + '/pub/' +
                   coalesce(t.app, 'pubky.app') + '/tags/' + t.id) AS tag_uri
        ORDER BY indexed_at, author, tag_uri, resource_id
    ",
        ))
        .await
        .map_err(GraphError::from)?;
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
        let tag_uri: String = row.get("tag_uri")?;
        let line = ResourceChange::tag(
            ResourceChangeOp::Put,
            row.get("resource_id")?,
            row.get("uri")?,
            row.get("author")?,
            row.get("label")?,
            &tag_uri,
            row.get("indexed_at")?,
        )?;
        batch.push(line.json()?);
        if batch.len() == BATCH_SIZE {
            write_seed_batch(temp, &batch).await?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        write_seed_batch(temp, &batch).await?;
    }
    publish_seed(token, temp, start).await.map_err(Into::into)
}

async fn write_seed_batch(temp: &str, batch: &[String]) -> RedisResult<()> {
    let mut conn = get_redis_conn().await?;
    redis::pipe()
        .atomic()
        .cmd("RPUSH")
        .arg(temp)
        .arg(batch)
        .ignore()
        .cmd("EXPIRE")
        .arg(temp)
        .arg(SEED_TTL)
        .ignore()
        .query_async::<()>(&mut conn)
        .await?;
    Ok(())
}

async fn publish_seed(token: &str, temp: &str, start: u64) -> RedisResult<u64> {
    static PUBLISH: LazyLock<Script> = LazyLock::new(|| {
        Script::new(
            r"
        if redis.call('GET', KEYS[4]) ~= ARGV[1] then
            return redis.error_reply('Resource changes seed lease expired')
        end
        local length = redis.call('LLEN', KEYS[1])
        if length < tonumber(ARGV[2]) then
            return redis.error_reply('Resource changes feed was truncated during seed')
        end
        for offset = tonumber(ARGV[2]), length - 1, 1000 do
            local tail = redis.call('LRANGE', KEYS[1], offset, math.min(offset + 999, length - 1))
            redis.call('RPUSH', KEYS[3], unpack(tail))
        end
        if redis.call('EXISTS', KEYS[3]) == 1 then
            redis.call('RENAME', KEYS[3], KEYS[1])
            redis.call('PERSIST', KEYS[1])
        else
            redis.call('DEL', KEYS[1])
        end
        redis.call('SETNX', KEYS[2], ARGV[3])
        redis.call('INCR', KEYS[2])
        return redis.call('LLEN', KEYS[1])
    ",
        )
    });
    let mut conn = get_redis_conn().await?;
    Ok(PUBLISH
        .key(RESOURCE_CHANGES_KEY)
        .key(EPOCH_KEY)
        .key(temp)
        .key(SEED_LOCK)
        .arg(token)
        .arg(start)
        .arg(epoch_seed())
        .invoke_async(&mut conn)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StackConfig, StackManager};

    fn tag(author: &str, id: &str, at: i64) -> ResourceChange {
        let uri = format!("https://example.com/feed/{author}/{id}");
        ResourceChange::tag(
            ResourceChangeOp::Put,
            crate::universal_tag::normalize::resource_id(&uri),
            uri,
            author.into(),
            "test".into(),
            &format!("pubky://{author}/pub/test.app/tags/{id}"),
            at,
        )
        .unwrap()
    }

    #[test]
    fn tag_line_fields_and_object_path() {
        let line = tag("author", "one", 1);
        let json = serde_json::to_value(&line).unwrap();
        assert_eq!(json["kind"], "tag");
        assert_eq!(json["op"], "put");
        assert_eq!(json["object_path"], "/pub/test.app/tags/one");
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "at",
                "author",
                "kind",
                "label",
                "object_path",
                "op",
                "resource_id",
                "uri"
            ]
        );
        assert!(object_path("author", "pubky://someone-else/pub/test.app/tags/one").is_err());
        assert!(object_path("author", "pubky://author/private/one").is_err());
    }

    // Runs exclusively via nextest: it replaces the shared feed, never the graph.
    #[tokio_shared_rt::test(shared)]
    async fn live_feed_paging_delete_recovery_and_atomic_seed() -> Result<(), crate::types::DynError>
    {
        StackManager::setup(&StackConfig::default()).await?;
        let mut conn = get_redis_conn().await?;
        let _: () = conn.del(&[RESOURCE_CHANGES_KEY, EPOCH_KEY]).await?;
        let (page, epoch, length) = ResourceChange::get_from_index(0, 1000).await?;
        assert!(page.lines.is_empty());
        assert_eq!(page.next_cursor, 0);
        assert_eq!((epoch, length), (None, 0));
        let exists: u64 = redis::cmd("EXISTS")
            .arg(&[RESOURCE_CHANGES_KEY, EPOCH_KEY])
            .query_async(&mut conn)
            .await?;
        assert_eq!(exists, 0, "reads must not initialize keys");

        let author = pubky::Keypair::random().public_key().to_z32();
        let first = tag(&author, "one", 10);
        first.store().await?;
        let mut deleted = first.clone();
        deleted.op = ResourceChangeOp::Del;
        deleted.at = 11;
        deleted.prepare_delete().await?;
        assert_eq!(
            ResourceChange::get_from_index(0, 1000).await?.0.lines,
            std::slice::from_ref(&first)
        );
        let tag_uri = format!("pubky://{author}{}", first.object_path);
        // Models an interrupted handler after the graph edge has gone.
        ResourceChange::finish_delete(&author, &tag_uri).await?;
        ResourceChange::finish_delete(&author, &tag_uri).await?;
        let (page, epoch, length) = ResourceChange::get_from_index(0, 1).await?;
        assert_eq!(page.lines, std::slice::from_ref(&first));
        assert_eq!(page.next_cursor, 1);
        assert_eq!(length, 2);
        let next = ResourceChange::get_from_index(page.next_cursor, 1).await?;
        assert_eq!(next.0.lines, std::slice::from_ref(&deleted));
        assert_eq!(next.0.next_cursor, 2);
        assert_eq!(next.1, epoch);
        assert!(ResourceChange::get_from_index(2, 1000)
            .await?
            .0
            .lines
            .is_empty());
        assert_eq!(
            ResourceChange::get_from_index(u64::MAX, 1000)
                .await?
                .0
                .next_cursor,
            u64::MAX
        );
        assert!(ResourceChange::get_from_index(0, 0)
            .await?
            .0
            .lines
            .is_empty());

        let graph = get_neo4j_graph()?;
        let second = tag(&author, "two", 20);
        for line in [&second, &first] {
            // Insertion order differs from indexed_at.
            graph
                .run(
                    Query::new(
                        "seed_feed_test_edges",
                        r"
                MERGE (u:User {id: $author})
                MERGE (r:Resource {id: $resource_id}) SET r.uri = $uri, r.scheme = 'https'
                MERGE (u)-[t:TAGGED {id: $id}]->(r)
                SET t.label = $label, t.uri = $tag_uri, t.indexed_at = $at, t.app = 'test.app'
            ",
                    )
                    .param("author", author.as_str())
                    .param("resource_id", line.resource_id.clone())
                    .param("uri", line.uri.clone())
                    .param("id", line.object_path.clone())
                    .param("tag_uri", format!("pubky://{author}{}", line.object_path))
                    .param("label", "test")
                    .param("at", line.at),
                )
                .await?;
        }
        ResourceChange::reseed().await?;
        let (seeded, new_epoch, length) = ResourceChange::get_from_index(0, 1000).await?;
        assert_ne!(new_epoch, epoch);
        assert_eq!(length as usize, seeded.lines.len());
        assert!(seeded.lines.windows(2).all(|pair| pair[0].at <= pair[1].at));
        let own: Vec<_> = seeded
            .lines
            .into_iter()
            .filter(|line| line.author == author)
            .collect();
        assert_eq!(own, [first.clone(), second]);

        // Emulate a watcher append between snapshot start and publication.
        let token = format!("feed-test-{author}");
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        assert!(try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await?);
        let start: u64 = conn.llen(RESOURCE_CHANGES_KEY).await?;
        write_seed_batch(&temp, &[first.json()?]).await?;
        deleted.store().await?;
        let before = ResourceChange::get_from_index(0, 1000).await?;
        assert_eq!(before.1, new_epoch, "staging must not publish early");
        assert!(publish_seed("wrong-token", &temp, start).await.is_err());
        assert_eq!(
            ResourceChange::get_from_index(0, 1000).await?.0.lines,
            before.0.lines
        );
        assert_eq!(publish_seed(&token, &temp, start).await?, 2);
        release_lock(SEED_LOCK, &token).await?;
        let after = ResourceChange::get_from_index(0, 1000).await?;
        assert_eq!(after.0.lines, [first, deleted]);
        assert_ne!(after.1, before.1);

        graph.run(Query::new("cleanup_feed_test", "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:TAGGED]->(r:Resource) DETACH DELETE r, u").param("author", author)).await?;
        ResourceChange::reseed().await?;
        Ok(())
    }
}
