//! Brings a seed scope the interactions of users outside it: their replies, mentions,
//! reposts, tags and follows of the seed. Big Nexus instances say where those records
//! are (the seed's notifications); each is fetched from its author's homeserver, so a big
//! Nexus can leave things out but can't invent or change them (see
//! `docs/decentralization.md`).

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::user::UserIngestor;
use nexus_common::types::DynError;
use nexus_common::{ScopeConfig, WatcherConfig};
use opentelemetry::metrics::{Counter, Gauge};
use opentelemetry::{global, KeyValue};
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{tag_uri_builder, ParsedUri, PubkyAppTag, PubkyId};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::errors::EventProcessorError;
use crate::events::{
    get_nexus_json, handle_del_event, handle_put_event, Event, Moderation, ParseResult,
};
use crate::service::indexer::METER_NAME;
use crate::service::runner::HomeserverBackoff;

/// Notifications asked for per page, the most `/v0/user/{id}/notifications` serves.
const PAGE_LIMIT: usize = 100;
/// Prefix of each source's position: the newest notification timestamp handled.
const POSITION_PREFIX: &str = "Scope:Bridge";

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
        Ok(get_nexus_json(&url).await?.unwrap_or_default())
    }
}

/// The record a notification of `seed` is about. Tags and follows are rebuilt from who
/// made them and what they target, which is what their addresses are derived from.
pub fn record_of(body: &NotificationBody, seed: &str) -> String {
    use NotificationBody::*;
    let tag_uri = |tagger: &str, tagged: String, label: &str| {
        let tag = PubkyAppTag {
            uri: tagged,
            label: label.to_string(),
            created_at: 0,
        };
        tag_uri_builder(tagger.to_string(), tag.create_id())
    };
    let profile_uri = || format!("pubky://{seed}/pub/pubky.app/profile.json");

    match body {
        Follow { followed_by: by }
        | NewFriend { followed_by: by }
        | LostFriend { unfollowed_by: by } => format!("pubky://{by}/pub/pubky.app/follows/{seed}"),
        TagPost {
            tagged_by: by,
            tag_label,
            post_uri,
            ..
        }
        | UntagPost {
            untagged_by: by,
            tag_label,
            post_uri,
            ..
        } => tag_uri(by, post_uri.clone(), tag_label),
        TagProfile {
            tagged_by: by,
            tag_label,
        }
        | UntagProfile {
            untagged_by: by,
            tag_label,
        } => tag_uri(by, profile_uri(), tag_label),
        Reply { reply_uri: uri, .. }
        | Repost {
            repost_uri: uri, ..
        }
        | Mention { post_uri: uri, .. }
        | PostEdited {
            edited_uri: uri, ..
        }
        | PostDeleted {
            deleted_uri: uri, ..
        } => uri.clone(),
    }
}

/// What happened to one bridged record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Handed to the indexing handlers, which fetch it from its author's homeserver (or
    /// drop it if the moderator took it down, counted in `watcher.events.taken_down`).
    Indexed,
    /// Gone from its author's homeserver, so removed here too.
    Removed,
    /// Not bridged: its author is in scope (its own stream covers it), or it can't be
    /// read as a record.
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
    /// Sources failing recently are skipped for a while, as failing homeservers are.
    backoff: Mutex<HomeserverBackoff>,
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
            backoff: Mutex::new(HomeserverBackoff::default()),
        }
    }

    /// Polls every source not in backoff. A failing source is backed off and the others
    /// still run; the run fails only if every source failed.
    pub async fn poll(&self) -> Result<(), DynError> {
        let mut failed = 0;
        for source in &self.sources {
            if self.backoff.lock().await.should_skip(source.name()) {
                debug!(source = source.name(), "Skipping a big Nexus in backoff");
                continue;
            }
            match self.poll_source(source.as_ref()).await {
                Ok(handled) => {
                    self.backoff.lock().await.record_success(source.name());
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
                    self.backoff.lock().await.record_failure(source.name());
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
            let uri = record_of(&notification.body, &self.seed);
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
        // Taken-down records are dropped by `handle_put_event`.
        if self.ingestor.scope().is_in_scope(author, None).await? {
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
    Ok(redis_conn.get(key).await?)
}

async fn write_position(key: &str, timestamp: i64) -> Result<(), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn.set(key, timestamp).await?;
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
            assert_eq!(record_of(&body, SEED), record, "{body:?}");
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
