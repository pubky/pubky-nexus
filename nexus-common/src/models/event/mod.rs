use crate::db::get_redis_conn;
use crate::db::kv::lists;
use crate::db::{kv::RedisResult, RedisOps};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tracing::error;

/// Most event-log lines a filtered read scans in one request, matched or not.
pub const EVENTS_FILTER_SCAN_BUDGET: usize = 10_000;
/// Lines fetched per `LRANGE` while scanning.
const EVENTS_FILTER_SCAN_CHUNK: usize = 1_000;

const EVENTS_PREFIX: &str = "Event";
const EVENTS_KEY: &str = "Events";
const EVENTS_EPOCH_KEY: &str = "Event:Epoch";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLine(String);

impl EventLine {
    pub fn new(line: String) -> Self {
        Self(line)
    }
}

#[async_trait::async_trait]
impl RedisOps for EventLine {
    async fn prefix() -> String {
        EVENTS_PREFIX.to_string()
    }
}

impl AsRef<[String]> for EventLine {
    fn as_ref(&self) -> &[String] {
        std::slice::from_ref(&self.0)
    }
}

/// A page of the event log filtered by author.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredEvents {
    /// Matching lines, in log order.
    pub lines: Vec<String>,
    /// Position after the last line scanned (not after the last match), so the
    /// next page neither skips nor repeats lines. Equal to the requested cursor
    /// once the end of the log is reached.
    pub next_cursor: u64,
}

impl EventLine {
    /// Appends the line to the event log, creating the log's epoch if it has none
    /// yet (a new or flushed Redis). One atomic round trip, so no line is ever
    /// stored without an epoch.
    #[tracing::instrument(name = "event.index.write", skip_all)]
    pub async fn store(&self) -> RedisResult<()> {
        let mut redis_conn = get_redis_conn().await?;
        let () = deadpool_redis::redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(EVENTS_EPOCH_KEY)
            .arg(new_epoch())
            .arg("NX")
            .ignore()
            .rpush(events_list_key(), &self.0)
            .ignore()
            .query_async(&mut redis_conn)
            .await?;
        Ok(())
    }

    pub async fn get_from_index(
        cursor: Option<u64>,
        limit: usize,
    ) -> RedisResult<(Vec<String>, u64)> {
        let start = cursor.unwrap_or(0);
        let start_u = usize::try_from(start).unwrap_or(usize::MAX);
        let result =
            EventLine::try_from_index_list(&[EVENTS_KEY], Some(start_u), Some(limit)).await;

        let events = match result {
            Ok(r) => r.unwrap_or_default(),
            Err(error) => {
                error!("IndexReadFailed: Failed to read from list due to Redis error: {error}");
                return Err(error);
            }
        };

        let next_cursor = start + events.len() as u64;
        Ok((events, next_cursor))
    }

    /// Up to `limit` lines from `cursor` whose author is in `authors`, scanning at
    /// most [`EVENTS_FILTER_SCAN_BUDGET`] lines.
    #[tracing::instrument(name = "event.index.read_filtered", skip_all)]
    pub async fn get_filtered_from_index(
        cursor: Option<u64>,
        limit: usize,
        authors: &HashSet<String>,
    ) -> RedisResult<FilteredEvents> {
        scan_filtered(
            EVENTS_PREFIX,
            EVENTS_KEY,
            cursor.unwrap_or(0),
            limit,
            authors,
            EVENTS_FILTER_SCAN_BUDGET,
        )
        .await
        .inspect_err(|e| error!("IndexReadFailed: Failed to scan the event log: {e}"))
    }

    /// Identifies the current event log: created with its first line and recreated
    /// after Redis is flushed (e.g. by `db clear`), so a follower can tell that its
    /// stored cursor points into a different history. Read-only; `None` until the
    /// first line is stored.
    pub async fn epoch() -> RedisResult<Option<String>> {
        Ok(Self::log_state().await?.epoch)
    }

    /// Number of lines in the event log. A follower whose cursor is past it knows the
    /// log was truncated or restored and must restart.
    pub async fn log_len() -> RedisResult<u64> {
        Ok(Self::log_state().await?.len)
    }

    /// The log's epoch and length, read together in one round trip.
    pub async fn log_state() -> RedisResult<EventLogState> {
        let mut redis_conn = get_redis_conn().await?;
        let (epoch, len): (Option<String>, u64) = deadpool_redis::redis::pipe()
            .get(EVENTS_EPOCH_KEY)
            .llen(events_list_key())
            .query_async(&mut redis_conn)
            .await?;
        Ok(EventLogState { epoch, len })
    }
}

/// What identifies the event log at one moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventLogState {
    /// See [`EventLine::epoch`].
    pub epoch: Option<String>,
    /// See [`EventLine::log_len`].
    pub len: u64,
}

fn events_list_key() -> String {
    format!("{EVENTS_PREFIX}:{EVENTS_KEY}")
}

fn new_epoch() -> String {
    chrono::Utc::now().timestamp_millis().to_string()
}

/// The author of an event line: `PUT pubky://<author>/pub/...` -> `<author>`.
pub fn author_of_line(line: &str) -> Option<&str> {
    let (_, uri) = line.split_once(' ')?;
    let author = uri.strip_prefix("pubky://")?.split('/').next()?;
    (!author.is_empty()).then_some(author)
}

/// Scans the list `prefix:key` from `cursor`, keeping lines whose author is in
/// `authors`, until `limit` lines match, `budget` lines are scanned, or the list ends.
async fn scan_filtered(
    prefix: &str,
    key: &str,
    cursor: u64,
    limit: usize,
    authors: &HashSet<String>,
    budget: usize,
) -> RedisResult<FilteredEvents> {
    let mut lines = Vec::new();
    let mut position = cursor;
    let mut scanned = 0;

    while scanned < budget && lines.len() < limit {
        let chunk = EVENTS_FILTER_SCAN_CHUNK.min(budget - scanned);
        let start = usize::try_from(position).unwrap_or(usize::MAX);
        let Some(batch) = lists::get_range(prefix, key, Some(start), Some(chunk)).await? else {
            break;
        };
        let reached_end = batch.len() < chunk;

        for line in batch {
            position = position.saturating_add(1);
            scanned += 1;
            if author_of_line(&line).is_some_and(|author| authors.contains(author)) {
                lines.push(line);
                if lines.len() == limit {
                    break;
                }
            }
        }

        if reached_end {
            break;
        }
    }

    Ok(FilteredEvents {
        lines,
        next_cursor: position,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn author_of_put_and_del_lines() {
        assert_eq!(
            author_of_line("PUT pubky://alice/pub/pubky.app/posts/0034"),
            Some("alice")
        );
        assert_eq!(
            author_of_line("DEL pubky://bob/pub/pubky.app/follows/alice"),
            Some("bob")
        );
    }

    #[test]
    fn author_of_malformed_lines() {
        assert_eq!(author_of_line(""), None);
        assert_eq!(author_of_line("PUT"), None);
        assert_eq!(author_of_line("PUT https://alice/pub"), None);
        assert_eq!(author_of_line("PUT pubky:///pub/x"), None);
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    use crate::db::kv::lists;
    use crate::{StackConfig, StackManager};
    use deadpool_redis::redis::AsyncCommands;

    const PREFIX: &str = "Test:EventScan";

    async fn setup(key: &str, lines: &[String]) {
        StackManager::setup(&StackConfig::default()).await.unwrap();
        let mut redis_conn = get_redis_conn().await.unwrap();
        let _: () = redis_conn.del(format!("{PREFIX}:{key}")).await.unwrap();
        for batch in lines.chunks(1_000) {
            let refs: Vec<&str> = batch.iter().map(String::as_str).collect();
            lists::put(PREFIX, key, &refs).await.unwrap();
        }
    }

    async fn teardown(key: &str) {
        let mut redis_conn = get_redis_conn().await.unwrap();
        let _: () = redis_conn.del(format!("{PREFIX}:{key}")).await.unwrap();
    }

    fn line(author: &str, i: usize) -> String {
        format!("PUT pubky://{author}/pub/pubky.app/posts/{i}")
    }

    fn only(author: &str) -> HashSet<String> {
        HashSet::from([author.to_string()])
    }

    /// `a`, `b`, `a`, `b`, ... for `n` lines.
    fn alternating(n: usize) -> Vec<String> {
        (0..n)
            .map(|i| line(if i % 2 == 0 { "a" } else { "b" }, i))
            .collect()
    }

    #[tokio_shared_rt::test(shared)]
    async fn returns_only_matching_authors_in_order() {
        let key = "match";
        setup(key, &alternating(10)).await;

        let page = scan_filtered(PREFIX, key, 0, 100, &only("a"), 10_000)
            .await
            .unwrap();
        let expected: Vec<String> = (0..10).step_by(2).map(|i| line("a", i)).collect();
        assert_eq!(page.lines, expected);
        assert_eq!(page.next_cursor, 10);

        teardown(key).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn limit_reached_mid_chunk_continues_without_gaps() {
        let key = "limit";
        setup(key, &alternating(10)).await;

        // Matches are at positions 0, 2, 4, 6, 8.
        let first = scan_filtered(PREFIX, key, 0, 2, &only("a"), 10_000)
            .await
            .unwrap();
        assert_eq!(first.lines, vec![line("a", 0), line("a", 2)]);
        assert_eq!(
            first.next_cursor, 3,
            "cursor sits right after the last match"
        );

        let second = scan_filtered(PREFIX, key, first.next_cursor, 10, &only("a"), 10_000)
            .await
            .unwrap();
        assert_eq!(second.lines, vec![line("a", 4), line("a", 6), line("a", 8)]);
        assert_eq!(second.next_cursor, 10);

        teardown(key).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn budget_bounds_each_request_and_paging_reaches_the_end() {
        let key = "budget";
        // 2,500 lines, one match at the very end.
        let mut lines: Vec<String> = (0..2_499).map(|i| line("noise", i)).collect();
        lines.push(line("rare", 2_499));
        setup(key, &lines).await;

        let first = scan_filtered(PREFIX, key, 0, 10, &only("rare"), 1_000)
            .await
            .unwrap();
        assert!(first.lines.is_empty());
        assert_eq!(first.next_cursor, 1_000, "stops after the budget");

        let second = scan_filtered(PREFIX, key, 1_000, 10, &only("rare"), 1_000)
            .await
            .unwrap();
        assert!(second.lines.is_empty());
        assert_eq!(second.next_cursor, 2_000);

        let third = scan_filtered(PREFIX, key, 2_000, 10, &only("rare"), 1_000)
            .await
            .unwrap();
        assert_eq!(third.lines, vec![line("rare", 2_499)]);
        assert_eq!(third.next_cursor, 2_500);

        teardown(key).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn end_of_log_returns_the_requested_cursor() {
        let key = "end";
        setup(key, &alternating(4)).await;

        let page = scan_filtered(PREFIX, key, 4, 10, &only("a"), 10_000)
            .await
            .unwrap();
        assert!(page.lines.is_empty());
        assert_eq!(page.next_cursor, 4);

        let huge = scan_filtered(PREFIX, key, u64::MAX, 10, &only("a"), 10_000)
            .await
            .unwrap();
        assert!(huge.lines.is_empty());
        assert_eq!(huge.next_cursor, u64::MAX);

        teardown(key).await;
    }

    #[tokio_shared_rt::test(shared)]
    async fn storing_lines_keeps_one_epoch_and_grows_the_log() {
        StackManager::setup(&StackConfig::default()).await.unwrap();
        let author = format!("epoch-test-{}", new_epoch());

        EventLine::new(line(&author, 0)).store().await.unwrap();
        let first = EventLine::log_state().await.unwrap();
        assert!(first.epoch.is_some(), "storing a line creates the epoch");

        EventLine::new(line(&author, 1)).store().await.unwrap();
        let second = EventLine::log_state().await.unwrap();
        assert_eq!(second.epoch, first.epoch, "the epoch is set only once");
        assert!(second.len > first.len, "the line was appended");

        // Leave the shared log as it was.
        let mut redis_conn = get_redis_conn().await.unwrap();
        for i in 0..2 {
            let _: i64 = redis_conn
                .lrem(events_list_key(), 0, line(&author, i))
                .await
                .unwrap();
        }
    }
}
