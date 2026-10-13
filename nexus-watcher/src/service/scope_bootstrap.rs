//! Works out a seed scope's members at start from a big Nexus's event log.
//!
//! Without it, a new seed-scoped Nexus finds its members hop by hop: the seed's
//! follows are only known once its stream is read, their follows once theirs are,
//! and so on. A big Nexus already has every follow, so one pass over its
//! `/v0/events` log gives the whole member set, and every member is read from its
//! own homeserver in parallel from the start.
//!
//! Only membership comes from the big Nexus. Content is always read from each
//! member's homeserver, so a big Nexus that lags or leaves events out can't hide
//! any; at worst it misses a member, who then joins later through the crawl.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use futures::{stream, StreamExt};
use nexus_common::db::PubkyConnector;
use nexus_common::models::scope::SeedHopsScope;
use nexus_common::models::user::UserIngestor;
use nexus_common::types::DynError;
use pubky::Method;
use pubky_app_specs::{PubkyId, Resource};
use tracing::{info, warn};

use crate::events::{Event, EventType, ParseResult};

/// Lines asked for per page, the most `/v0/events` serves.
const PAGE_LIMIT: u64 = 1_000;
/// Tries per page before the bootstrap gives up and the crawl takes over.
const MAX_ATTEMPTS: u32 = 3;
/// Times the read starts over because the log was recreated under it.
const MAX_RESTARTS: u32 = 3;
/// Members whose homeserver is resolved and bound at the same time.
const INGEST_CONCURRENCY: usize = 16;

/// Response headers a Nexus with the log epoch sends; see `/v0/events`.
const EPOCH_HEADER: &str = "x-events-epoch";
const LENGTH_HEADER: &str = "x-events-length";

/// One page of a Nexus event log.
#[derive(Debug, Default)]
pub struct LogPage {
    pub lines: Vec<String>,
    /// The position after the last line returned.
    pub next_cursor: u64,
    /// Identifies the log, if the Nexus sends it. A change means it was recreated.
    pub epoch: Option<String>,
    /// Lines in the log when the page was read, if the Nexus sends it.
    pub length: Option<u64>,
}

/// Where the event log is read from: a big Nexus over HTTP, or a fake in tests.
#[async_trait::async_trait]
pub trait EventLogSource: Send + Sync {
    async fn page(&self, cursor: u64) -> Result<LogPage, DynError>;
}

/// A big Nexus's `GET /v0/events`.
pub struct NexusEventLog {
    pub nexus_url: String,
}

#[async_trait::async_trait]
impl EventLogSource for NexusEventLog {
    async fn page(&self, cursor: u64) -> Result<LogPage, DynError> {
        let url = format!(
            "{}/v0/events?cursor={cursor}&limit={PAGE_LIMIT}",
            self.nexus_url
        );
        let response = PubkyConnector::get()?
            .client()
            .request(Method::GET, &url)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(format!("GET {url}: HTTP {}", response.status()).into());
        }

        let header = |name: &str| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)
        };
        let epoch = header(EPOCH_HEADER);
        let length = header(LENGTH_HEADER).and_then(|value| value.parse().ok());

        let body = response.text().await?;
        let mut page = LogPage {
            epoch,
            length,
            next_cursor: cursor,
            ..LogPage::default()
        };
        for line in body.lines() {
            match line.strip_prefix("cursor: ") {
                Some(next) => page.next_cursor = next.trim().parse()?,
                None if !line.is_empty() => page.lines.push(line.to_string()),
                None => {}
            }
        }
        Ok(page)
    }
}

/// Who follows whom at the end of the log.
#[derive(Debug, Default)]
pub struct FollowGraph(HashMap<String, HashSet<String>>);

impl FollowGraph {
    /// Applies one log line; anything but a follow or unfollow is ignored.
    pub fn apply(&mut self, line: &str) {
        let Ok(ParseResult::Parsed(event)) = Event::parse_event(line) else {
            return;
        };
        let Resource::Follow(followee) = event.parsed_uri.resource() else {
            return;
        };
        let follower = event.parsed_uri.user_id().to_string();
        let followee = followee.to_string();

        match event.event_type {
            EventType::Put => {
                self.0.entry(follower).or_default().insert(followee);
            }
            EventType::Del => {
                if let Some(follows) = self.0.get_mut(&follower) {
                    follows.remove(&followee);
                }
            }
        }
    }

    /// Everyone the seed reaches in at most `hops` follows, with their distance.
    /// The seed itself is at 0.
    pub fn members(&self, seed: &str, hops: u8) -> HashMap<String, u8> {
        let mut members = HashMap::from([(seed.to_string(), 0)]);
        let mut frontier = VecDeque::from([(seed.to_string(), 0u8)]);

        while let Some((user, hop)) = frontier.pop_front() {
            if hop >= hops {
                continue;
            }
            for followee in self.0.get(&user).into_iter().flatten() {
                if !members.contains_key(followee) {
                    members.insert(followee.clone(), hop + 1);
                    frontier.push_back((followee.clone(), hop + 1));
                }
            }
        }
        members
    }
}

/// Reads the whole log once into a follow graph.
///
/// Starts over if the log is recreated under it (the epoch changes from one value to
/// another, or the cursor ends up past the log's length), as `/v0/events` advises.
pub async fn read_follow_graph(source: &dyn EventLogSource) -> Result<FollowGraph, DynError> {
    'restart: for _ in 0..=MAX_RESTARTS {
        let mut graph = FollowGraph::default();
        let mut cursor = 0;
        let mut epoch: Option<String> = None;

        loop {
            let page = page_with_retries(source, cursor).await?;

            let epoch_changed =
                matches!((&epoch, &page.epoch), (Some(old), Some(new)) if old != new);
            let past_the_end = page.length.is_some_and(|length| cursor > length);
            if epoch_changed || past_the_end {
                warn!("The big Nexus's event log was recreated while it was read; starting over");
                continue 'restart;
            }
            if page.epoch.is_some() {
                epoch = page.epoch;
            }

            if page.lines.is_empty() || page.next_cursor <= cursor {
                return Ok(graph);
            }
            for line in &page.lines {
                graph.apply(line);
            }
            cursor = page.next_cursor;
        }
    }
    Err("the big Nexus's event log kept being recreated while it was read".into())
}

async fn page_with_retries(source: &dyn EventLogSource, cursor: u64) -> Result<LogPage, DynError> {
    let mut attempt = 1;
    loop {
        match source.page(cursor).await {
            Ok(page) => return Ok(page),
            Err(e) if attempt < MAX_ATTEMPTS => {
                warn!(cursor, attempt, error = %e, "Reading the big Nexus's event log failed; retrying");
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Adds every member the big Nexus knows of to the seed scope, unless that was done
/// before for this seed and hop count. Each member is ingested in scope, so it is
/// polled from the start of its own stream.
///
/// Errors are logged, never returned: the crawl finds the members anyway, more slowly.
pub async fn bootstrap_seed_scope(
    scope: &SeedHopsScope,
    ingestor: &UserIngestor,
    source: &dyn EventLogSource,
) {
    match scope.is_bootstrapped().await {
        Ok(false) => {}
        Ok(true) => return,
        Err(e) => {
            warn!(error = %e, "Could not check the scope bootstrap; skipping it");
            return;
        }
    }

    let graph = match read_follow_graph(source).await {
        Ok(graph) => graph,
        Err(e) => {
            warn!(error = %e, "Could not read the big Nexus's event log; members are found by crawling");
            return;
        }
    };
    let members = graph.members(scope.seed(), scope.hops());
    info!(
        members = members.len(),
        "Bootstrapping the seed scope's members from the big Nexus"
    );

    let failed = stream::iter(members)
        .map(|(user_id, hop)| async move { add_member(scope, ingestor, &user_id, hop).await })
        .buffer_unordered(INGEST_CONCURRENCY)
        .filter(|result| std::future::ready(result.is_err()))
        .count()
        .await;

    if failed > 0 {
        // Not marked done, so the next start tries again for the ones that failed.
        warn!(
            failed,
            "Some members could not be added; the next start retries them"
        );
        return;
    }
    if let Err(e) = scope.mark_bootstrapped().await {
        warn!(error = %e, "Could not record the scope bootstrap; the next start repeats it");
    }
}

async fn add_member(
    scope: &SeedHopsScope,
    ingestor: &UserIngestor,
    user_id: &str,
    hop: u8,
) -> Result<(), DynError> {
    let user_id = PubkyId::try_from(user_id)?;
    scope.add_member(&user_id, hop).await?;
    ingestor
        .ensure_in_scope(&user_id)
        .await
        .inspect_err(|e| warn!(%user_id, error = %e, "Could not add a bootstrapped member"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Valid z32 keys, so the lines parse like real ones.
    const A: &str = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
    const B: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";
    const C: &str = "uo7jgkykft4885n8cruizwy6khw71mnu5pq3ay9i8pw1ymcn85ko";
    const D: &str = "8pinxxgqs41n4aididenw5apqp1urfmzdztr8jt4abrkdn435ewo";

    fn follow(op: &str, follower: &str, followee: &str) -> String {
        format!("{op} pubky://{follower}/pub/pubky.app/follows/{followee}")
    }

    #[test]
    fn members_are_who_the_seed_reaches_within_the_hops() {
        let mut graph = FollowGraph::default();
        for line in [
            follow("PUT", A, B),
            follow("PUT", B, C),
            follow("PUT", C, D),
            format!("PUT pubky://{A}/pub/pubky.app/posts/0034FJVRPG4V0"),
        ] {
            graph.apply(&line);
        }

        assert_eq!(
            graph.members(A, 2),
            HashMap::from([(A.into(), 0), (B.into(), 1), (C.into(), 2)])
        );
        assert_eq!(graph.members(A, 1).len(), 2);
    }

    #[test]
    fn an_unfollow_removes_the_follow() {
        let mut graph = FollowGraph::default();
        graph.apply(&follow("PUT", A, B));
        graph.apply(&follow("DEL", A, B));

        assert_eq!(graph.members(A, 3), HashMap::from([(A.into(), 0)]));
    }

    #[test]
    fn a_shorter_path_wins() {
        let mut graph = FollowGraph::default();
        graph.apply(&follow("PUT", A, B));
        graph.apply(&follow("PUT", B, C));
        graph.apply(&follow("PUT", A, C));

        assert_eq!(graph.members(A, 3)[C], 1);
    }

    /// A log served from memory, which can be swapped out mid-read.
    struct FakeLog {
        logs: Mutex<VecDeque<(Option<&'static str>, Vec<String>)>>,
        reads: Mutex<u32>,
    }

    #[async_trait::async_trait]
    impl EventLogSource for FakeLog {
        async fn page(&self, cursor: u64) -> Result<LogPage, DynError> {
            let mut reads = self.reads.lock().unwrap();
            *reads += 1;
            let mut logs = self.logs.lock().unwrap();
            // After the second read the log is recreated, if a next one is queued.
            if *reads > 2 && logs.len() > 1 {
                logs.pop_front();
            }
            let (epoch, lines) = logs.front().unwrap();
            let start = cursor as usize;
            let end = (start + 1).min(lines.len());
            Ok(LogPage {
                lines: lines.get(start..end).unwrap_or_default().to_vec(),
                next_cursor: end.max(start) as u64,
                epoch: epoch.map(str::to_string),
                length: Some(lines.len() as u64),
            })
        }
    }

    #[tokio::test]
    async fn reads_every_page_and_starts_over_when_the_log_is_recreated() {
        let old_log = vec![follow("PUT", A, B), follow("PUT", B, C)];
        let new_log = vec![
            follow("PUT", A, D),
            follow("PUT", D, C),
            follow("PUT", A, B),
        ];
        let source = FakeLog {
            logs: Mutex::new(VecDeque::from([
                (Some("old"), old_log),
                (Some("new"), new_log),
            ])),
            reads: Mutex::new(0),
        };

        let graph = read_follow_graph(&source).await.unwrap();

        // Only the new log's follows: B -> C was in the old log alone.
        assert_eq!(
            graph.members(A, 2),
            HashMap::from([(A.into(), 0), (B.into(), 1), (D.into(), 1), (C.into(), 2)])
        );
        assert!(graph.0.get(B).is_none_or(HashSet::is_empty));
    }
}
