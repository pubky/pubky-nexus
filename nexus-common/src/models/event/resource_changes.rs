//! Durable resource references. Reads never create or repair feed state.
//!
//! Delivery is at least once. Consumers key a tag reference by `(author, uri, label)`: `uri` is
//! the normalized URI stored on the Resource, so every file that tags one target with one label
//! names the same reference. `object_path` is the path of the tag file the line concerns, for
//! information only. A post reference is keyed by `(author, object_path, kind, resource_id)`,
//! `object_path` being the path of the post's address. Rebuilds preserve live appends made
//! while the graph snapshot is assembled.

use crate::db::graph::{GraphError, Query};
use crate::db::kv::{RedisError, RedisResult};
use crate::db::{get_neo4j_graph, get_redis_conn, release_lock, try_acquire_lock};
use crate::models::error::{ModelError, ModelResult};
use deadpool_redis::redis::{self, AsyncCommands, Script};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::Duration;
use tracing::warn;
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

/// Tags and external replies, embeds and collection items share this feed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ResourceChangeKind {
    Tag,
    Reply,
    Embed,
    Collection,
}

/// One feed line. Lines stay in Redis across builds, so a field added later must be optional
/// (`#[serde(default)]`), and fields a line does not know are ignored.
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// A homeserver path: for a tag, the tag file the line concerns, as written, for
    /// information only; for a post reference, the post's address, the same for every version
    /// file of a `social/v1` post. A tag's replay key is (author, uri, label); a post
    /// reference's is (author, object_path, kind, resource_id).
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

fn object_path<'a>(author: &str, tag_uri: &'a str) -> ModelResult<&'a str> {
    tag_uri
        .strip_prefix(&format!("pubky://{author}"))
        .filter(|path| path.starts_with("/pub/"))
        .ok_or_else(|| ModelError::InvalidInput(format!("Invalid tag storage URI: {tag_uri}")))
}

// KEYS[1] and KEYS[2] are the list and the epoch, ARGV[1] seeds the epoch, ARGV[2] is the line.
static APPEND: LazyLock<Script> = LazyLock::new(|| {
    Script::new(
        r"
    redis.call('SETNX', KEYS[2], ARGV[1])
    redis.call('RPUSH', KEYS[1], ARGV[2])
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

    /// Appends the line. A put is appended after its graph write has committed, a del before
    /// its graph delete: a retry appends either again, and consumers tolerate duplicates.
    pub async fn store(&self) -> RedisResult<()> {
        let mut conn = get_redis_conn().await?;
        APPEND
            .key(RESOURCE_CHANGES_KEY)
            .key(EPOCH_KEY)
            .arg(epoch_seed())
            .arg(self.json()?)
            .invoke_async::<()>(&mut conn)
            .await?;
        #[cfg(any(test, feature = "test-utils"))]
        if self.op == ResourceChangeOp::Del {
            crate::utils::test_utils::feed_del_fault(&self.author)?;
        }
        Ok(())
    }

    /// Append one post's reference changes atomically, in the supplied order.
    pub async fn store_batch(lines: &[Self]) -> RedisResult<()> {
        if lines.is_empty() {
            return Ok(());
        }
        static BATCH: LazyLock<Script> = LazyLock::new(|| {
            Script::new(
                r"
            redis.call('SETNX', KEYS[2], ARGV[1])
            for i = 2, #ARGV do redis.call('RPUSH', KEYS[1], ARGV[i]) end
        ",
            )
        });
        let raw = lines
            .iter()
            .map(Self::json)
            .collect::<RedisResult<Vec<_>>>()?;
        BATCH
            .key(RESOURCE_CHANGES_KEY)
            .key(EPOCH_KEY)
            .arg(epoch_seed())
            .arg(raw)
            .invoke_async::<()>(&mut get_redis_conn().await?)
            .await?;
        #[cfg(any(test, feature = "test-utils"))]
        if lines.iter().any(|line| line.op == ResourceChangeOp::Del) {
            crate::utils::test_utils::feed_del_fault(&lines[0].author)?;
        }
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
    ///
    /// A snapshot with no lines does not replace a feed that held lines when the seed started:
    /// an empty or wrong graph would otherwise reset every consumer. With `allow_empty` it does,
    /// for a graph whose references were all deleted.
    pub async fn reseed(allow_empty: bool) -> ModelResult<u64> {
        let token = pubky::Keypair::random().public_key().to_z32();
        if !try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await? {
            return Err(ModelError::from_generic(
                "Resource changes seed already running",
            ));
        }
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        let result = tokio::time::timeout(
            Duration::from_secs(3600),
            seed_inner(&token, &temp, allow_empty),
        )
        .await
        .map_err(ModelError::from_generic)
        .and_then(|r| r);
        // Aborted processes leave only expiring temporary data and the old feed.
        if let Ok(mut conn) = get_redis_conn().await {
            let _ = conn.del::<_, ()>(&temp).await;
        }
        // The lease expires by itself: a failed release must not report a published seed
        // as failed.
        if let Err(e) = release_lock(SEED_LOCK, &token).await {
            warn!(error = %e, "Resource changes seed lease not released; it expires by itself");
        }
        result
    }
}

/// The put line of one resource tag edge the seed reads, or why it cannot be one: a missing
/// field or a stored address outside its author's folder.
fn seed_line(row: &neo4rs::Row) -> ModelResult<ResourceChange> {
    let tag_uri: Option<String> = row.get("tag_uri")?;
    let tag_uri =
        tag_uri.ok_or_else(|| ModelError::InvalidInput("no stored tag address".into()))?;
    ResourceChange::tag(
        ResourceChangeOp::Put,
        row.get("resource_id")?,
        row.get("uri")?,
        row.get("author")?,
        row.get("label")?,
        &tag_uri,
        row.get("indexed_at")?,
    )
}

async fn seed_inner(token: &str, temp: &str, allow_empty: bool) -> ModelResult<u64> {
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
        RETURN elementId(t) AS edge, u.id AS author, r.id AS resource_id, r.uri AS uri,
               t.label AS label, t.indexed_at AS indexed_at, t.uri AS tag_uri
        ORDER BY indexed_at, author, tag_uri, resource_id
    ",
        ))
        .await
        .map_err(GraphError::from)?;
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
        // One edge that cannot be a line must not stop the seed, which the migration runs
        let line = match seed_line(&row) {
            Ok(line) => line,
            Err(e) => {
                let edge: Option<String> = row.get("edge").ok();
                warn!(?edge, error = %e, "Resource changes seed skipped a resource tag edge");
                continue;
            }
        };
        batch.push(line.json()?);
        if batch.len() == BATCH_SIZE {
            write_seed_batch(temp, &batch).await?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        write_seed_batch(temp, &batch).await?;
    }
    seed_post_references(temp).await?;
    publish_seed(token, temp, start, allow_empty)
        .await
        .map_err(Into::into)
}

async fn seed_post_references(temp: &str) -> ModelResult<()> {
    let mut rows = get_neo4j_graph()?
        .execute(crate::db::graph::queries::get::resource_changes_post_seed())
        .await
        .map_err(GraphError::from)?;
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
        // As for tags: one reference that cannot be a line must not stop the seed
        let line: ResourceChange = match row.get("line") {
            Ok(line) => line,
            Err(e) => {
                warn!(error = %e, "Resource changes seed skipped a post reference");
                continue;
            }
        };
        batch.push(line.json()?);
        if batch.len() == BATCH_SIZE {
            write_seed_batch(temp, &batch).await?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        write_seed_batch(temp, &batch).await?;
    }
    Ok(())
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

async fn publish_seed(token: &str, temp: &str, start: u64, allow_empty: bool) -> RedisResult<u64> {
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
        if ARGV[4] ~= '1' and tonumber(ARGV[2]) > 0 and redis.call('EXISTS', KEYS[3]) == 0 then
            return redis.error_reply('Resource changes seed found no references while the feed '
                .. 'holds ' .. ARGV[2] .. ' lines: run resource-changes-seed-empty to publish it')
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
        .arg(if allow_empty { "1" } else { "0" })
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

    #[test]
    fn older_and_newer_lines_parse() {
        // A line with no optional field, as an older build may have stored one
        let minimal = r#"{"op":"del","resource_id":"r","uri":"https://example.com/a","kind":"tag","author":"a","object_path":"/pub/pubky.app/tags/x","at":1}"#;
        let line: ResourceChange = serde_json::from_str(minimal).unwrap();
        assert_eq!((line.op, line.label), (ResourceChangeOp::Del, None));
        // A line with a field this build does not know, as a later build may store one
        let extended = r#"{"op":"put","resource_id":"r","uri":"https://example.com/a","kind":"tag","author":"a","label":"l","object_path":"/pub/pubky.app/tags/x","at":2,"later":{"any":1}}"#;
        let line: ResourceChange = serde_json::from_str(extended).unwrap();
        assert_eq!((line.label.as_deref(), line.at), (Some("l"), 2));
    }

    // Runs exclusively via nextest: it replaces the shared feed, never the graph.
    #[tokio_shared_rt::test(shared)]
    async fn live_feed_paging_and_atomic_seed() -> Result<(), crate::types::DynError> {
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
        deleted.store().await?;
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
                MERGE (u:User {id: $author}) ON CREATE SET u.epoch = 'v0'
                MERGE (r:Resource {id: $resource_id}) SET r.uri = $uri, r.scheme = 'https'
                MERGE (u)-[t:TAGGED {id: $id}]->(r)
                SET t.label = $label, t.uri = $tag_uri, t.indexed_at = $at
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
        ResourceChange::reseed(false).await?;
        let (seeded, new_epoch, length) = ResourceChange::get_from_index(0, 1000).await?;
        assert_ne!(new_epoch, epoch);
        assert_eq!(length as usize, seeded.lines.len());
        // The seed writes the tag lines in edge time order, then the post reference lines in
        // theirs: only this test's own lines, all tags, are compared, order included.
        let own: Vec<_> = seeded
            .lines
            .into_iter()
            .filter(|line| line.author == author)
            .collect();
        assert_eq!(own, [first.clone(), second]);

        // Emulate a watcher append between snapshot start and publication. The lease is
        // released before any assertion or error return, so a failure leaves no seed blocked.
        let token = format!("feed-test-{author}");
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        assert!(try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await?);
        let leased = async {
            let start: u64 = conn.llen(RESOURCE_CHANGES_KEY).await?;
            write_seed_batch(&temp, &[first.json()?]).await?;
            deleted.store().await?;
            let before = ResourceChange::get_from_index(0, 1000).await?;
            let wrong_token = publish_seed("wrong-token", &temp, start, false).await;
            let after_wrong_token = ResourceChange::get_from_index(0, 1000).await?.0.lines;
            let published = publish_seed(&token, &temp, start, false).await;
            Ok::<_, crate::types::DynError>((before, wrong_token, after_wrong_token, published))
        }
        .await;
        release_lock(SEED_LOCK, &token).await?;
        let (before, wrong_token, after_wrong_token, published) = leased?;
        assert_eq!(before.1, new_epoch, "staging must not publish early");
        assert!(wrong_token.is_err());
        assert_eq!(after_wrong_token, before.0.lines);
        assert_eq!(published?, 2);
        let after = ResourceChange::get_from_index(0, 1000).await?;
        assert_eq!(after.0.lines, [first, deleted]);
        assert_ne!(after.1, before.1);

        graph.run(Query::new("cleanup_feed_test", "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:TAGGED]->(r:Resource) DETACH DELETE r, u").param("author", author)).await?;
        ResourceChange::reseed(false).await?;
        Ok(())
    }

    // Runs exclusively via nextest: it replaces the shared feed and writes, then deletes, its
    // own graph edges.
    #[tokio_shared_rt::test(shared)]
    async fn live_seed_writes_and_copies_past_one_batch() -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let mut conn = get_redis_conn().await?;
        let graph = get_neo4j_graph()?;
        let author = pubky::Keypair::random().public_key().to_z32();
        // One edge more than a seed batch, `indexed_at` 0, 2, 4, …, and two edges the seed cannot
        // turn into lines, which it skips: one with no stored address inside the first batch
        // (at 601), one whose address is another user's just before the last good edge, in the
        // second batch. A seed that stopped at a bad edge instead of skipping it misses the good
        // edges after it.
        let edges = BATCH_SIZE + 1;
        let other = pubky::Keypair::random().public_key().to_z32();
        graph
            .run(
                Query::new(
                    "seed_batches_test_edges",
                    r"
            MERGE (u:User {id: $author}) ON CREATE SET u.epoch = 'v0'
            WITH u
            UNWIND range(0, $edges + 1) AS i
            CREATE (r:Resource {id: $author + ':' + toString(i), scheme: 'https',
                uri: 'https://example.com/seed-batches/' + $author + '/' + toString(i)})
            CREATE (u)-[t:TAGGED {id: 'seed' + toString(i), label: 'test'}]->(r)
            SET t.indexed_at = CASE
                WHEN i < $edges THEN 2 * i
                WHEN i = $edges THEN 601
                ELSE 2 * $edges - 3
            END
            SET t.uri = CASE
                WHEN i < $edges THEN 'pubky://' + $author + '/pub/pubky.app/tags/seed' + toString(i)
                WHEN i > $edges THEN 'pubky://' + $other + '/pub/pubky.app/tags/seed' + toString(i)
            END
        ",
                )
                .param("author", author.as_str())
                .param("other", other.as_str())
                .param("edges", edges as i64),
            )
            .await?;
        let length = ResourceChange::reseed(false).await?;
        let (seeded, _, read) = ResourceChange::get_from_index(0, usize::MAX).await?;
        assert_eq!((read, seeded.lines.len() as u64), (length, length));
        let own: Vec<_> = seeded
            .lines
            .into_iter()
            .filter(|line| line.author == author)
            .collect();
        let paths: Vec<String> = (0..edges)
            .map(|i| format!("/pub/pubky.app/tags/seed{i}"))
            .collect();
        assert_eq!(
            own.iter()
                .map(|line| line.object_path.as_str())
                .collect::<Vec<_>>(),
            paths,
            "each good edge once, in indexed_at order, and the bad ones skipped"
        );

        // Appends made while a seed runs, more than one copy chunk of them, follow the seed
        let token = format!("feed-test-{author}");
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        let snapshot = tag(&author, "snapshot", 0);
        let appended: Vec<ResourceChange> = (0..2500)
            .map(|i| tag(&author, &format!("appended{i}"), i))
            .collect();
        // The lease is released before any assertion or error return, so a failure leaves no
        // seed blocked
        assert!(try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await?);
        let leased = async {
            let start: u64 = conn.llen(RESOURCE_CHANGES_KEY).await?;
            write_seed_batch(&temp, &[snapshot.json()?]).await?;
            let json = appended
                .iter()
                .map(ResourceChange::json)
                .collect::<RedisResult<Vec<_>>>()?;
            let _: () = conn.rpush(RESOURCE_CHANGES_KEY, &json).await?;
            Ok::<_, crate::types::DynError>(publish_seed(&token, &temp, start, false).await)
        }
        .await;
        release_lock(SEED_LOCK, &token).await?;
        assert_eq!(leased??, 2501);
        let (published, _, _) = ResourceChange::get_from_index(0, usize::MAX).await?;
        assert_eq!(published.lines[0], snapshot);
        assert_eq!(published.lines[1..], appended[..]);

        graph.run(Query::new("cleanup_feed_test", "MATCH (u:User {id: $author}) OPTIONAL MATCH (u)-[:TAGGED]->(r:Resource) DETACH DELETE r, u").param("author", author)).await?;
        ResourceChange::reseed(false).await?;
        Ok(())
    }

    // Runs exclusively via nextest: it replaces the shared feed, never the graph.
    #[tokio_shared_rt::test(shared)]
    async fn live_empty_seed_does_not_replace_a_feed_with_lines(
    ) -> Result<(), crate::types::DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let mut conn = get_redis_conn().await?;
        let author = pubky::Keypair::random().public_key().to_z32();
        tag(&author, "kept", 1).store().await?;
        let before = ResourceChange::get_from_index(0, usize::MAX).await?;
        let token = format!("feed-test-{author}");
        let temp = format!("{RESOURCE_CHANGES_KEY}:Seed:{token}");
        // The lease is released before any assertion or error return
        assert!(try_acquire_lock(SEED_LOCK, &token, SEED_TTL).await?);
        let leased = async {
            let start: u64 = conn.llen(RESOURCE_CHANGES_KEY).await?;
            // A snapshot that read no references, as from an empty or wrong graph
            let refused = publish_seed(&token, &temp, start, false).await;
            let after_refusal = ResourceChange::get_from_index(0, usize::MAX).await?;
            // Allowed explicitly, it publishes the empty feed under a new epoch
            let allowed = publish_seed(&token, &temp, start, true).await;
            let after_allowed = ResourceChange::get_from_index(0, usize::MAX).await?;
            Ok::<_, crate::types::DynError>((start, refused, after_refusal, allowed, after_allowed))
        }
        .await;
        release_lock(SEED_LOCK, &token).await?;
        ResourceChange::reseed(false).await?;

        let (start, refused, after_refusal, allowed, after_allowed) = leased?;
        assert!(start > 0);
        let refused = refused.expect_err("an empty snapshot must not replace the feed");
        assert!(
            refused.to_string().contains("found no references"),
            "{refused}"
        );
        assert_eq!(after_refusal.0.lines, before.0.lines);
        assert_eq!(
            (after_refusal.1, after_refusal.2),
            (before.1.clone(), before.2)
        );
        assert_eq!(allowed?, 0);
        assert!(after_allowed.0.lines.is_empty());
        assert_ne!(after_allowed.1, before.1);
        Ok(())
    }
}
