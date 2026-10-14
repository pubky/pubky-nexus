//! Brings a seed scope the interactions of users outside it.
//!
//! A seed scope never reads users outside it, so when one of them replies to the seed,
//! mentions it, reposts or tags its posts, or follows it, the record sits on that user's
//! homeserver and is never seen. The bridge asks big Nexus instances for the seed's
//! notifications, turns each into the one record it is about, and fetches that record
//! from its author's homeserver: a big Nexus only says where to look, never what is
//! there, so it can leave things out but can't invent or change them.
//!
//! Each record is indexed through the normal handlers (its author kept as a stub), or
//! deleted if its homeserver no longer has it (an untag, a deleted post). Each source
//! keeps its own position, so several sources can be used and one being down changes
//! nothing for the others.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use deadpool_redis::redis;
use nexus_common::db::{get_redis_conn, PubkyConnector};
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::takedown::is_taken_down;
use nexus_common::models::user::UserIngestor;
use nexus_common::types::DynError;
use nexus_common::{ScopeConfig, WatcherConfig};
use opentelemetry::metrics::{Counter, Gauge};
use opentelemetry::{global, KeyValue};
use pubky::Method;
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{tag_uri_builder, ParsedUri, PubkyAppTag, PubkyId};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::errors::EventProcessorError;
use crate::events::{handle_del_event, handle_put_event, Event, Moderation, ParseResult};
use crate::service::indexer::METER_NAME;

/// Notifications asked for per page, the most `/v0/user/{id}/notifications` serves.
const PAGE_LIMIT: usize = 100;
/// Prefix of each source's position: the newest notification timestamp handled.
const POSITION_PREFIX: &str = "Scope:Bridge";
const MIN_BACKOFF: Duration = Duration::from_secs(60);
const MAX_BACKOFF: Duration = Duration::from_secs(3_600);

struct BridgeMetrics {
    records: Counter<u64>,
    source_errors: Counter<u64>,
    lag_ms: Gauge<u64>,
}

static METRICS: LazyLock<BridgeMetrics> = LazyLock::new(|| {
    let meter = global::meter(METER_NAME);
    BridgeMetrics {
        records: meter
            .u64_counter("watcher.bridge.records")
            .with_description("Records the inbound bridge handled, by outcome")
            .build(),
        source_errors: meter
            .u64_counter("watcher.bridge.source_errors")
            .with_description("Failed polls of a big Nexus by the inbound bridge")
            .build(),
        lag_ms: meter
            .u64_gauge("watcher.bridge.lag_ms")
            .with_description("Age of the newest notification the inbound bridge handled")
            .build(),
    }
});

/// Where the seed's notifications are read: a big Nexus over HTTP, or a fake in tests.
#[async_trait::async_trait]
pub trait NotificationSource: Send + Sync {
    /// Names the source in its stored position and metrics.
    fn name(&self) -> &str;

    /// One page of the user's notifications, newest first: timestamps at most `start`
    /// and at least `end`, after skipping `skip`.
    async fn page(
        &self,
        user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<Vec<Notification>, DynError>;
}

/// A big Nexus's `GET /v0/user/{id}/notifications`.
pub struct NexusNotifications {
    pub nexus_url: String,
}

#[async_trait::async_trait]
impl NotificationSource for NexusNotifications {
    fn name(&self) -> &str {
        &self.nexus_url
    }

    async fn page(
        &self,
        user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<Vec<Notification>, DynError> {
        let mut url = format!(
            "{}/v0/user/{user_id}/notifications?limit={PAGE_LIMIT}&skip={skip}",
            self.nexus_url
        );
        if let Some(start) = start {
            url.push_str(&format!("&start={start}"));
        }
        if let Some(end) = end {
            url.push_str(&format!("&end={end}"));
        }
        let response = PubkyConnector::get()?
            .client()
            .request(Method::GET, &url)
            .send()
            .await?;
        match response.status() {
            status if status.is_success() => Ok(response.json().await?),
            reqwest::StatusCode::NOT_FOUND => Ok(Vec::new()),
            status => Err(format!("GET {url}: HTTP {status}").into()),
        }
    }
}

/// The record a notification of `seed` is about, or `None` for one with no record of
/// its own. Tags and follows are rebuilt from who made them and what they target: their
/// addresses are derived from those.
pub fn record_of(body: &NotificationBody, seed: &str) -> Option<String> {
    let profile_uri = || format!("pubky://{seed}/pub/pubky.app/profile.json");
    let follow_uri = |follower: &str| format!("pubky://{follower}/pub/pubky.app/follows/{seed}");
    let tag_uri = |tagger: &str, tagged: String, label: &str| {
        let tag = PubkyAppTag {
            uri: tagged,
            label: label.to_string(),
            created_at: 0,
        };
        tag_uri_builder(tagger.to_string(), tag.create_id())
    };

    match body {
        NotificationBody::Follow { followed_by } | NotificationBody::NewFriend { followed_by } => {
            Some(follow_uri(followed_by))
        }
        NotificationBody::LostFriend { unfollowed_by } => Some(follow_uri(unfollowed_by)),
        NotificationBody::TagPost {
            tagged_by,
            tag_label,
            post_uri,
            ..
        } => Some(tag_uri(tagged_by, post_uri.clone(), tag_label)),
        NotificationBody::UntagPost {
            untagged_by,
            tag_label,
            post_uri,
            ..
        } => Some(tag_uri(untagged_by, post_uri.clone(), tag_label)),
        NotificationBody::TagProfile {
            tagged_by,
            tag_label,
        } => Some(tag_uri(tagged_by, profile_uri(), tag_label)),
        NotificationBody::UntagProfile {
            untagged_by,
            tag_label,
        } => Some(tag_uri(untagged_by, profile_uri(), tag_label)),
        NotificationBody::Reply { reply_uri, .. } => Some(reply_uri.clone()),
        NotificationBody::Repost { repost_uri, .. } => Some(repost_uri.clone()),
        NotificationBody::Mention { post_uri, .. } => Some(post_uri.clone()),
        NotificationBody::PostEdited { edited_uri, .. } => Some(edited_uri.clone()),
        NotificationBody::PostDeleted { deleted_uri, .. } => Some(deleted_uri.clone()),
    }
}

/// What happened to one bridged record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Fetched from its author's homeserver and indexed.
    Indexed,
    /// Gone from its author's homeserver, so removed here too.
    Removed,
    /// Not bridged: its author is in scope (its own stream covers it), it was taken
    /// down, or it can't be read as a record.
    Skipped,
    /// Couldn't be indexed (e.g. invalid); logged and left.
    Failed,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Indexed => "indexed",
            Self::Removed => "removed",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// The bridge of one seed scope.
pub struct InboundBridge {
    seed: PubkyId,
    sources: Vec<Box<dyn NotificationSource>>,
    ingestor: Arc<UserIngestor>,
    moderation: Arc<Moderation>,
    files_path: PathBuf,
    max_file_size: u64,
    lookback: Option<Duration>,
    /// Sources failing recently: skipped until the instant, with their failure count.
    backoff: Mutex<HashMap<String, (Instant, u32)>>,
}

impl InboundBridge {
    /// The bridge the configuration asks for: `None` if it is off or the scope isn't a
    /// seed scope (the bridge follows the seed alone).
    pub fn from_config(config: &WatcherConfig) -> Option<Self> {
        let bridge = &config.inbound_bridge;
        if !bridge.is_on() {
            return None;
        }
        let ScopeConfig::Seed { seed, .. } = &config.stack.scope else {
            warn!("inbound_bridge only applies to a seed scope; ignoring it");
            return None;
        };
        let sources = bridge
            .nexus_urls
            .iter()
            .map(|nexus_url| {
                Box::new(NexusNotifications {
                    nexus_url: nexus_url.clone(),
                }) as Box<dyn NotificationSource>
            })
            .collect();
        Some(Self::new(
            seed.clone(),
            sources,
            Arc::new(UserIngestor::from_watcher_config(config)),
            Moderation::from_config(config),
            config.stack.files_path.clone(),
            config.max_file_size,
            bridge.lookback_days,
        ))
    }

    pub fn new(
        seed: PubkyId,
        sources: Vec<Box<dyn NotificationSource>>,
        ingestor: Arc<UserIngestor>,
        moderation: Arc<Moderation>,
        files_path: PathBuf,
        max_file_size: u64,
        lookback_days: u32,
    ) -> Self {
        Self {
            seed,
            sources,
            ingestor,
            moderation,
            files_path,
            max_file_size,
            lookback: (lookback_days > 0)
                .then(|| Duration::from_secs(u64::from(lookback_days) * 86_400)),
            backoff: Mutex::new(HashMap::new()),
        }
    }

    /// Polls every source not in backoff. A failing source is backed off and the others
    /// still run; the run fails only if every source failed.
    pub async fn poll(&self) -> Result<(), DynError> {
        let mut failed = 0;
        for source in &self.sources {
            if self.in_backoff(source.name()).await {
                debug!(source = source.name(), "Skipping a big Nexus in backoff");
                continue;
            }
            match self.poll_source(source.as_ref()).await {
                Ok(handled) => {
                    self.backoff.lock().await.remove(source.name());
                    if handled > 0 {
                        info!(source = source.name(), handled, "Bridged inbound records");
                    }
                }
                Err(e) => {
                    failed += 1;
                    METRICS
                        .source_errors
                        .add(1, &[KeyValue::new("source", source.name().to_string())]);
                    warn!(source = source.name(), error = %e, "Inbound bridge poll failed");
                    self.back_off(source.name()).await;
                }
            }
        }
        if failed > 0 && failed == self.sources.len() {
            return Err("every inbound bridge source failed".into());
        }
        Ok(())
    }

    /// Reads the source's notifications newer than its position, oldest first, and
    /// bridges each one's record. The position moves forward only past records that
    /// were handled: a record that couldn't be fetched for a passing reason stops the
    /// run there, and the next poll starts from it.
    pub async fn poll_source(&self, source: &dyn NotificationSource) -> Result<usize, DynError> {
        let position_key = format!("{POSITION_PREFIX}:{}:{}", self.seed, source.name());
        let position = read_position(&position_key).await?;
        // At least the position, not past it: a notification sharing its timestamp is
        // read again (bridging is idempotent) rather than possibly missed.
        let end = position.or_else(|| {
            self.lookback
                .map(|lookback| chrono::Utc::now().timestamp_millis() - lookback.as_millis() as i64)
        });

        let mut notifications = read_since(source, &self.seed, end).await?;
        notifications.sort_by_key(|notification| notification.timestamp);

        let mut handled = 0;
        let mut newest = None;
        for notification in &notifications {
            if let Some(uri) = record_of(&notification.body, &self.seed) {
                let outcome = match self.bridge_record(&uri).await {
                    Ok(outcome) => outcome,
                    Err(e) => {
                        if let Some(newest) = newest {
                            write_position(&position_key, newest).await?;
                        }
                        return Err(format!("bridging {uri}: {e}").into());
                    }
                };
                METRICS
                    .records
                    .add(1, &[KeyValue::new("outcome", outcome.label())]);
                handled += 1;
            }
            newest = Some(notification.timestamp);
        }

        if let Some(newest) = newest {
            write_position(&position_key, newest).await?;
            let lag = chrono::Utc::now().timestamp_millis() - newest;
            METRICS.lag_ms.record(lag.max(0) as u64, &[]);
        }
        Ok(handled)
    }

    /// Fetches one record from its author's homeserver and indexes it, or removes it if
    /// the homeserver no longer has it. Errors only for a passing failure (the
    /// homeserver or Nexus's own stores unreachable), so the record is tried again.
    pub async fn bridge_record(&self, uri: &str) -> Result<Outcome, EventProcessorError> {
        let Ok(ParseResult::Parsed(put)) = Event::parse_event(&format!("PUT {uri}")) else {
            return Ok(Outcome::Skipped);
        };
        let Ok(parsed) = ParsedUri::try_from(uri) else {
            return Ok(Outcome::Skipped);
        };
        let author = &parsed.user_id;
        if self.ingestor.scope().is_in_scope(author, None).await? || is_taken_down(uri).await? {
            return Ok(Outcome::Skipped);
        }
        match self.ingestor.maybe_ingest_user(author).await {
            Ok(()) => {}
            Err(nexus_common::models::error::ModelError::HsBlacklisted { .. }) => {
                return Ok(Outcome::Skipped)
            }
            Err(e) => return Err(e.into()),
        }

        let result = handle_put_event(
            &put,
            self.max_file_size,
            &self.files_path,
            self.moderation.clone(),
            self.ingestor.clone(),
        )
        .await;
        match result {
            Ok(()) => Ok(Outcome::Indexed),
            Err(e) if e.is_not_found() => {
                let Ok(ParseResult::Parsed(del)) = Event::parse_event(&format!("DEL {uri}")) else {
                    return Ok(Outcome::Skipped);
                };
                match handle_del_event(&del, &self.files_path, self.ingestor.clone()).await {
                    Ok(()) | Err(EventProcessorError::SkipIndexing) => Ok(Outcome::Removed),
                    Err(e) => Err(e),
                }
            }
            Err(e) if is_passing(&e) => Err(e),
            Err(e) => {
                debug!(%uri, error = %e, "Bridged record not indexed");
                Ok(Outcome::Failed)
            }
        }
    }

    async fn in_backoff(&self, source: &str) -> bool {
        self.backoff
            .lock()
            .await
            .get(source)
            .is_some_and(|(until, _)| Instant::now() < *until)
    }

    async fn back_off(&self, source: &str) {
        let mut backoff = self.backoff.lock().await;
        let failures = backoff.get(source).map_or(0, |(_, failures)| *failures) + 1;
        let delay = MIN_BACKOFF
            .saturating_mul(2u32.saturating_pow(failures - 1))
            .min(MAX_BACKOFF);
        backoff.insert(source.to_string(), (Instant::now() + delay, failures));
    }
}

/// A failure that may pass on its own: the network, a homeserver or Nexus's own stores.
fn is_passing(error: &EventProcessorError) -> bool {
    matches!(
        error,
        EventProcessorError::PubkyClientError(_)
            | EventProcessorError::GraphQueryFailed(..)
            | EventProcessorError::IndexOperationFailed(..)
    )
}

/// Every notification of `user_id` from `source` at least as recent as `end`, paging
/// newest first. Pages are chained by timestamp, not by an ever-growing skip, so a long
/// history isn't capped by the endpoint's skip limit.
async fn read_since(
    source: &dyn NotificationSource,
    user_id: &str,
    end: Option<i64>,
) -> Result<Vec<Notification>, DynError> {
    let mut notifications = Vec::new();
    let mut start = None;
    let mut skip = 0;
    loop {
        let page = source.page(user_id, start, end, skip).await?;
        let full_page = page.len() == PAGE_LIMIT;
        let Some(oldest) = page.last().map(|notification| notification.timestamp) else {
            break;
        };
        // The next page starts at the oldest timestamp seen, skipping the notifications
        // already read at exactly that timestamp.
        let at_oldest = page.iter().filter(|n| n.timestamp == oldest).count();
        skip = if start == Some(oldest) {
            skip + page.len()
        } else {
            at_oldest
        };
        start = Some(oldest);
        notifications.extend(page);
        if !full_page {
            break;
        }
    }
    Ok(notifications)
}

async fn read_position(key: &str) -> Result<Option<i64>, DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let position: Option<i64> = redis::cmd("GET")
        .arg(key)
        .query_async(&mut redis_conn)
        .await?;
    Ok(position)
}

async fn write_position(key: &str, timestamp: i64) -> Result<(), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let () = redis::cmd("SET")
        .arg(key)
        .arg(timestamp)
        .query_async(&mut redis_conn)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky_app_specs::PubkyAppPostKind;

    const SEED: &str = "8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty";
    const CAROL: &str = "1hb71xx9km3f4pw5izsy1gn19ff1uuuqonw4mcygzobwkryujoiy";

    #[test]
    fn every_notification_maps_to_the_record_it_is_about() {
        let post_uri = format!("pubky://{SEED}/pub/pubky.app/posts/0034FJVRPG4V0");
        let carol_post = format!("pubky://{CAROL}/pub/pubky.app/posts/0034FJVRPG4V1");
        let follow = format!("pubky://{CAROL}/pub/pubky.app/follows/{SEED}");
        let tag_on = |tagged: &str, label: &str| {
            let tag = PubkyAppTag {
                uri: tagged.to_string(),
                label: label.to_string(),
                created_at: 0,
            };
            tag_uri_builder(CAROL.to_string(), tag.create_id())
        };
        let profile = format!("pubky://{SEED}/pub/pubky.app/profile.json");

        let cases = [
            (
                NotificationBody::Follow {
                    followed_by: CAROL.into(),
                },
                follow.clone(),
            ),
            (
                NotificationBody::NewFriend {
                    followed_by: CAROL.into(),
                },
                follow.clone(),
            ),
            (
                NotificationBody::LostFriend {
                    unfollowed_by: CAROL.into(),
                },
                follow,
            ),
            (
                NotificationBody::TagPost {
                    tagged_by: CAROL.into(),
                    tag_label: "rust".into(),
                    post_uri: post_uri.clone(),
                    post_kind: PubkyAppPostKind::Short,
                },
                tag_on(&post_uri, "rust"),
            ),
            (
                NotificationBody::UntagProfile {
                    untagged_by: CAROL.into(),
                    tag_label: "friend".into(),
                },
                tag_on(&profile, "friend"),
            ),
            (
                NotificationBody::Reply {
                    replied_by: CAROL.into(),
                    parent_post_uri: post_uri.clone(),
                    reply_uri: carol_post.clone(),
                    post_kind: PubkyAppPostKind::Short,
                },
                carol_post.clone(),
            ),
            (
                NotificationBody::Mention {
                    mentioned_by: CAROL.into(),
                    post_uri: carol_post.clone(),
                    post_kind: PubkyAppPostKind::Short,
                },
                carol_post,
            ),
        ];
        for (body, record) in cases {
            assert_eq!(record_of(&body, SEED), Some(record), "{body:?}");
        }
    }

    /// Notifications in memory, served newest first like a Nexus.
    struct FakeSource(Vec<Notification>);

    #[async_trait::async_trait]
    impl NotificationSource for FakeSource {
        fn name(&self) -> &str {
            "fake"
        }

        async fn page(
            &self,
            _user_id: &str,
            start: Option<i64>,
            end: Option<i64>,
            skip: usize,
        ) -> Result<Vec<Notification>, DynError> {
            let mut matching: Vec<&Notification> = self
                .0
                .iter()
                .filter(|n| start.is_none_or(|start| n.timestamp <= start))
                .filter(|n| end.is_none_or(|end| n.timestamp >= end))
                .collect();
            matching.sort_by_key(|n| std::cmp::Reverse(n.timestamp));
            Ok(matching
                .into_iter()
                .skip(skip)
                .take(PAGE_LIMIT)
                .map(|n| Notification {
                    timestamp: n.timestamp,
                    body: n.body.clone(),
                })
                .collect())
        }
    }

    fn follow_at(timestamp: i64) -> Notification {
        Notification {
            timestamp,
            body: NotificationBody::Follow {
                followed_by: format!("user{timestamp}"),
            },
        }
    }

    /// Long histories, many notifications sharing a timestamp, and the lower bound.
    #[tokio::test]
    async fn reads_every_notification_since_the_position_across_pages() {
        let mut all: Vec<Notification> = (0..250).map(follow_at).collect();
        all.extend((0..150).map(|_| follow_at(300)));
        let source = FakeSource(all);

        assert_eq!(read_since(&source, SEED, None).await.unwrap().len(), 400);
        assert_eq!(
            read_since(&source, SEED, Some(200)).await.unwrap().len(),
            200
        );
    }
}
