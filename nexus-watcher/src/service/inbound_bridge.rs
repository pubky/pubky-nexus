//! Brings a seed scope the interactions of users outside it: their replies, mentions,
//! reposts, tags and follows of the seed, which nothing else reads, as outsiders' streams
//! never are.
//!
//! Big Nexus instances say where those records are (the seed's notifications). Each one is
//! fetched from its author's homeserver and indexed only if what is there concerns the
//! seed, so a big Nexus can leave records out, but can't invent or change them, nor make
//! this Nexus index records that have nothing to do with the seed.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::error::ModelError;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::PostDetails;
use nexus_common::models::user::{PkdnsHomeserverResolver, UserIngestor};
use nexus_common::types::DynError;
use nexus_common::{ScopeConfig, WatcherConfig};
use opentelemetry::metrics::{Counter, Gauge};
use opentelemetry::{global, KeyValue};
use pubky::{PubkyResource, PublicKey};
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{tag_uri_builder, ParsedUri, PubkyAppObject, PubkyAppTag, PubkyId, Resource};
use tracing::{debug, info, warn};

use super::big_nexus::{get_json, NexusAnswer};
use super::runner::HomeserverBackoff;
use crate::events::{handle_del_event, handle_put_event, Event, Moderation, ParseResult};
use crate::homeserver::{HomeserverClient, HsError, HsStreamEvent};
use crate::{EventProcessorError, METER_NAME};

/// Notifications asked for per page, the most `/v0/user/{id}/notifications` serves.
const PAGE_LIMIT: usize = 100;
/// Pages one poll reads from one source; a longer backlog is read over the next polls.
const MAX_PAGES: usize = 50;
/// Where each source's position is kept: `Scope:Bridge:<seed>:<source>`.
const POSITION_PREFIX: &str = "Scope:Bridge";
/// Where records waiting for what they point at are kept: `Scope:BridgePending:<seed>`.
const PENDING_PREFIX: &str = "Scope:BridgePending";
/// Polls a record may wait for what it points at before it is given up.
const MAX_PENDING_ATTEMPTS: u32 = 20;
/// Records waiting at once; past it, a new one is given up at once.
const MAX_PENDING: usize = 10_000;

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
    /// and at least `end`, after skipping `skip`. One request.
    async fn page(
        &self,
        user_id: &str,
        start: Option<i64>,
        end: Option<i64>,
        skip: usize,
    ) -> Result<NexusAnswer<Vec<Notification>>, DynError>;
}

/// A big Nexus's `GET /v0/user/{id}/notifications`.
pub struct NexusNotifications {
    /// Validated in the config: http(s), no query or fragment, no trailing slash.
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
    ) -> Result<NexusAnswer<Vec<Notification>>, DynError> {
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
        get_json(&url).await
    }
}

/// The address of the record a notification of `seed` is about. Tags and follows are
/// rebuilt from who made them and what they target, which is what their addresses are
/// derived from.
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

/// Whether a record, as its author's homeserver has it, concerns `seed`: a follow of the
/// seed, a tag on the seed's profile or posts, or a post replying to, reposting or
/// mentioning the seed. Only what is fetched is trusted, so a big Nexus can't make this
/// Nexus index records that have nothing to do with the seed.
pub fn concerns_seed(object: &PubkyAppObject, resource: &Resource, seed: &str) -> bool {
    let by_seed =
        |uri: &str| ParsedUri::try_from(uri).is_ok_and(|uri| uri.user_id.as_ref() == seed);
    match (object, resource) {
        (PubkyAppObject::Follow(_), Resource::Follow(followee)) => followee.as_ref() == seed,
        (PubkyAppObject::Tag(tag), Resource::Tag(_)) => by_seed(&tag.uri),
        (PubkyAppObject::Post(post), Resource::Post(_)) => {
            post.parent.as_deref().is_some_and(by_seed)
                || post.embed.as_ref().is_some_and(|embed| by_seed(&embed.uri))
                || post.content.contains(seed)
        }
        _ => false,
    }
}

/// What happened to one bridged record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Handed to the indexing handlers (which drop it if the moderator took it down).
    Indexed,
    /// Gone from its author's homeserver, so removed here too.
    Removed,
    /// Not bridged: its author is in scope (its own stream covers it), on a blacklisted
    /// homeserver, or it isn't a record's address.
    Skipped,
    /// Fetched, but it doesn't concern the seed: not indexed.
    Unrelated,
    /// Points at something not indexed yet (e.g. the seed's post it replies to); kept and
    /// tried again on the next polls.
    Waiting,
    /// Couldn't be indexed (e.g. invalid); logged and left.
    Failed,
}

impl Outcome {
    fn label(self) -> &'static str {
        match self {
            Self::Indexed => "indexed",
            Self::Removed => "removed",
            Self::Skipped => "skipped",
            Self::Unrelated => "unrelated",
            Self::Waiting => "waiting",
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
    client: Arc<dyn HomeserverClient>,
    files_path: PathBuf,
    /// How far back a source's first poll reads; `None` reads everything.
    lookback: Option<Duration>,
    /// Sources failing recently are skipped for a while, as failing homeservers are.
    backoff: HomeserverBackoff,
}

impl InboundBridge {
    /// The bridge the configuration asks for, `None` if it is off. Configured for any scope
    /// but a seed scope, it is an error: the bridge follows the seed's notifications.
    pub fn from_config(
        config: &WatcherConfig,
        client: Arc<dyn HomeserverClient>,
    ) -> Result<Option<Self>, DynError> {
        let bridge = &config.inbound_bridge;
        if !bridge.is_on() {
            return Ok(None);
        }
        let ScopeConfig::Seed { seed, .. } = &config.stack.scope else {
            return Err(
                "inbound_bridge needs a seed scope: it reads the seed's notifications".into(),
            );
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
        Ok(Some(Self::new(
            seed.clone(),
            sources,
            Arc::new(crate::scope::watcher_ingestor(config, client.clone())),
            Moderation::from_config(config),
            client,
            config.stack.files_path.clone(),
            bridge.lookback_days,
        )))
    }

    pub fn new(
        seed: PubkyId,
        sources: Vec<Box<dyn NotificationSource>>,
        ingestor: Arc<UserIngestor>,
        moderation: Arc<Moderation>,
        client: Arc<dyn HomeserverClient>,
        files_path: PathBuf,
        lookback_days: u32,
    ) -> Self {
        Self {
            seed,
            sources,
            ingestor,
            moderation,
            client,
            files_path,
            lookback: (lookback_days > 0)
                .then(|| Duration::from_secs(u64::from(lookback_days) * 86_400)),
            backoff: HomeserverBackoff::default(),
        }
    }

    /// Tries again the records waiting from earlier polls, then polls every source not
    /// backed off. A failing source is backed off and the others still run; the run fails
    /// only if every source failed.
    pub async fn poll(&self) -> Result<(), DynError> {
        self.retry_pending().await?;
        let mut failed = 0;
        for source in &self.sources {
            if self.backoff.should_skip(source.name()) {
                debug!(source = source.name(), "Skipping a big Nexus in backoff");
                continue;
            }
            match self.poll_source(source.as_ref()).await {
                Ok(handled) => {
                    self.backoff.record_success(source.name());
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
                    self.backoff.record_failure(source.name());
                }
            }
        }
        if failed > 0 && failed == self.sources.len() {
            return Err("every inbound bridge source failed".into());
        }
        Ok(())
    }

    /// Reads the source's notifications newer than its position, oldest first, and
    /// bridges each one's record. A record whose author's homeserver can't be read now waits
    /// with the others (see [`Self::start_waiting`]), so it doesn't hold up the rest. The
    /// position moves forward only past records that were handled: a failure of this
    /// Nexus's own stores stops the run there, and the next poll starts from it.
    ///
    /// One poll reads at most [`MAX_PAGES`] pages, newest first. If there is more, what was
    /// read is handled but the position stays: a catch-up point (the oldest notification
    /// read) is kept, the next polls page on down from it, and the position moves once they
    /// reach it.
    pub async fn poll_source(&self, source: &dyn NotificationSource) -> Result<usize, DynError> {
        let position_key = format!("{POSITION_PREFIX}:{}:{}", self.seed, source.name());
        let catch_up_key = format!("{position_key}:CatchUp");
        let position = read_position(&position_key).await?;
        // (oldest read, newest handled) of a read the page budget cut short.
        let catch_up = read_catch_up(&catch_up_key).await?;
        // At least the position, not past it: a notification sharing its timestamp is read
        // again (bridging is idempotent) rather than possibly missed.
        let end = position.or_else(|| {
            self.lookback
                .map(|lookback| chrono::Utc::now().timestamp_millis() - lookback.as_millis() as i64)
        });

        let start = catch_up.map(|(oldest_read, _)| oldest_read);
        let Read {
            mut notifications,
            cut_short,
        } = read_since(source, &self.seed, start, end).await?;
        notifications.sort_by_key(|notification| notification.timestamp);

        let mut handled = 0;
        let mut newest = None;
        for notification in &notifications {
            let uri = record_of(&notification.body, &self.seed);
            // Boxed: the handlers make the future too large for the stack.
            let outcome = match Box::pin(self.bridge_record(&uri)).await {
                Ok(Outcome::Waiting) => {
                    self.start_waiting(&uri).await?;
                    Outcome::Waiting
                }
                Ok(outcome) => outcome,
                Err(e) if is_homeserver_failure(&e) => {
                    debug!(%uri, error = %e, "Bridged record's homeserver unreachable; waiting");
                    self.start_waiting(&uri).await?;
                    Outcome::Waiting
                }
                Err(e) => {
                    // Past the handled records only if nothing older is still unread.
                    if let (Some(newest), false, None) = (newest, cut_short, catch_up) {
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

        // The newest notification handled, in this read or the reads it continues.
        let newest = newest.max(catch_up.map(|(_, newest)| newest));
        match (newest, notifications.first()) {
            (Some(newest), Some(oldest)) if cut_short => {
                write_catch_up(&catch_up_key, oldest.timestamp, newest).await?;
            }
            (Some(newest), _) => {
                write_position(&position_key, newest).await?;
                clear_catch_up(&catch_up_key).await?;
                let lag = chrono::Utc::now().timestamp_millis() - newest;
                METRICS.lag_ms.record(lag.max(0) as u64, &[]);
            }
            (None, _) => {}
        }
        Ok(handled)
    }

    /// Fetches one record from its author's homeserver and indexes it if it concerns the
    /// seed, or removes it if the homeserver no longer has it. Errors only for a passing
    /// failure (a homeserver or this Nexus's stores unreachable), so it is tried again.
    pub async fn bridge_record(&self, uri: &str) -> Result<Outcome, EventProcessorError> {
        let Ok(ParseResult::Parsed(put)) = Event::parse_event(&format!("PUT {uri}")) else {
            return Ok(Outcome::Skipped);
        };
        let author = put.parsed_uri.user_id().clone();
        // Members' records are read from their own streams.
        if self.ingestor.scope().is_in_scope(&author, None).await? {
            return Ok(Outcome::Skipped);
        }
        match self.ingestor.maybe_ingest_user(&author).await {
            Ok(()) => {}
            Err(ModelError::HsBlacklisted { .. }) => return Ok(Outcome::Skipped),
            Err(e) => return Err(e.into()),
        }
        // Also for an author already known, which `maybe_ingest_user` doesn't look up: its
        // homeserver may have been blacklisted since.
        match self.ingestor.ensure_hs_not_blacklisted(&author).await {
            Ok(_) => {}
            Err(ModelError::HsBlacklisted { .. }) => return Ok(Outcome::Skipped),
            Err(e) => return Err(e.into()),
        }

        let resource: PubkyResource = uri.parse()?;
        let blob = match self.client.get_resource(&resource).await {
            Ok(blob) => blob,
            Err(e) => {
                let e = EventProcessorError::from(e);
                return if e.is_not_found() {
                    self.remove(uri).await
                } else if is_passing(&e) {
                    Err(e)
                } else {
                    debug!(%uri, error = %e, "Bridged record not fetched");
                    Ok(Outcome::Failed)
                };
            }
        };
        let resource_kind = put.parsed_uri.resource().clone();
        let Ok(object) = PubkyAppObject::from_resource(&resource_kind, blob.as_slice()) else {
            return Ok(Outcome::Failed);
        };
        if !self.concerns_seed(&object, &resource_kind, &author).await? {
            debug!(%uri, "Bridged record doesn't concern the seed; not indexed");
            return Ok(Outcome::Unrelated);
        }

        // Handed to the handlers as a normal event, served the record just fetched.
        let client = Prefetched {
            uri: resource.to_string(),
            blob,
            inner: self.client.clone(),
        };
        let result = handle_put_event(
            &put,
            &self.files_path,
            self.moderation.clone(),
            self.ingestor.clone(),
            &client,
        )
        .await;
        match result {
            Ok(()) => Ok(Outcome::Indexed),
            Err(e) if e.is_missing_dependency() => Ok(Outcome::Waiting),
            Err(e) if is_passing(&e) => Err(e),
            Err(e) => {
                debug!(%uri, error = %e, "Bridged record not indexed");
                Ok(Outcome::Failed)
            }
        }
    }

    /// [`concerns_seed`], or a post this Nexus already holds: an edit or deletion the seed
    /// is told of keeps a post it interacted with up to date.
    async fn concerns_seed(
        &self,
        object: &PubkyAppObject,
        resource: &Resource,
        author: &str,
    ) -> Result<bool, EventProcessorError> {
        if concerns_seed(object, resource, &self.seed) {
            return Ok(true);
        }
        let (PubkyAppObject::Post(_), Resource::Post(post_id)) = (object, resource) else {
            return Ok(false);
        };
        Ok(PostDetails::get_from_index(author, post_id)
            .await?
            .is_some())
    }

    /// Removes a record its author's homeserver no longer has. One never indexed here is
    /// skipped.
    async fn remove(&self, uri: &str) -> Result<Outcome, EventProcessorError> {
        let Ok(ParseResult::Parsed(del)) = Event::parse_event(&format!("DEL {uri}")) else {
            return Ok(Outcome::Skipped);
        };
        match handle_del_event(&del, &self.files_path, self.ingestor.clone()).await {
            Ok(()) => Ok(Outcome::Removed),
            Err(EventProcessorError::SkipIndexing) => Ok(Outcome::Skipped),
            Err(e) => Err(e),
        }
    }
}

impl InboundBridge {
    fn pending_key(&self) -> String {
        format!("{PENDING_PREFIX}:{}", self.seed)
    }

    /// Keeps a record that waits for something it points at, to try it on the next polls.
    /// One already waiting keeps its count, so a source repeating it can't keep it waiting
    /// forever. Given up at once when too many records are waiting.
    async fn start_waiting(&self, uri: &str) -> Result<(), DynError> {
        let mut redis_conn = get_redis_conn().await?;
        let key = self.pending_key();
        let waiting: usize = redis_conn.hlen(&key).await?;
        if waiting >= MAX_PENDING {
            warn!(%uri, waiting, "Too many bridged records waiting; giving this one up");
            return Ok(());
        }
        let _: bool = redis_conn.hset_nx(&key, uri, 0).await?;
        Ok(())
    }

    /// Counts one more poll a record waited, and gives it up once it waited too long.
    async fn keep_waiting(&self, uri: &str, attempts: u32) -> Result<(), DynError> {
        let mut redis_conn = get_redis_conn().await?;
        let key = self.pending_key();
        if attempts >= MAX_PENDING_ATTEMPTS {
            warn!(%uri, attempts, "Giving up a bridged record still waiting for what it points at");
            let () = redis_conn.hdel(&key, uri).await?;
        } else {
            let () = redis_conn.hset(&key, uri, attempts).await?;
        }
        Ok(())
    }

    /// Tries again the records that waited on earlier polls. One whose author's homeserver
    /// is still unreachable counts a try like one still waiting, so it is given up in time
    /// and never holds up the others; a failure of this Nexus's own stores stops the round,
    /// and what is left is tried on the next poll.
    async fn retry_pending(&self) -> Result<(), DynError> {
        let pending: Vec<(String, u32)> = {
            let mut redis_conn = get_redis_conn().await?;
            redis_conn.hgetall(self.pending_key()).await?
        };
        for (uri, attempts) in pending {
            // Boxed: the handlers make the future too large for the stack.
            match Box::pin(self.bridge_record(&uri)).await {
                Ok(Outcome::Waiting) => self.keep_waiting(&uri, attempts + 1).await?,
                Err(e) if is_homeserver_failure(&e) => {
                    debug!(%uri, error = %e, "Waiting bridged record's homeserver still unreachable");
                    self.keep_waiting(&uri, attempts + 1).await?
                }
                Ok(outcome) => {
                    METRICS
                        .records
                        .add(1, &[KeyValue::new("outcome", outcome.label())]);
                    let mut redis_conn = get_redis_conn().await?;
                    let () = redis_conn.hdel(self.pending_key(), &uri).await?;
                }
                Err(e) => {
                    warn!(%uri, error = %e, "Retrying a waiting bridged record failed; next poll");
                    break;
                }
            }
        }
        Ok(())
    }
}

/// A failure of the record's author's homeserver (or the network to it), which may pass on
/// its own and concerns that record alone.
fn is_homeserver_failure(error: &EventProcessorError) -> bool {
    matches!(error, EventProcessorError::PubkyClientError(_))
}

/// A failure that may pass on its own: the network, a homeserver or this Nexus's stores.
fn is_passing(error: &EventProcessorError) -> bool {
    matches!(
        error,
        EventProcessorError::PubkyClientError(_)
            | EventProcessorError::GraphQueryFailed(..)
            | EventProcessorError::IndexOperationFailed(..)
    )
}

/// A client serving one record already fetched, and passing everything else to `inner`,
/// so the handlers index exactly what was checked, without fetching it twice.
struct Prefetched {
    uri: String,
    blob: Vec<u8>,
    inner: Arc<dyn HomeserverClient>,
}

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for Prefetched {
    async fn resolve_homeserver(
        &self,
        user_pk: &PublicKey,
    ) -> nexus_common::db::PubkyClientResult<Option<PubkyId>> {
        self.inner.resolve_homeserver(user_pk).await
    }
}

#[async_trait::async_trait]
impl HomeserverClient for Prefetched {
    async fn poll_events(
        &self,
        hs: &PubkyId,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<String>, HsError> {
        self.inner.poll_events(hs, cursor, limit).await
    }

    async fn user_events(
        &self,
        hs: &PubkyId,
        user: &PublicKey,
        cursor: u64,
        limit: u16,
    ) -> Result<Vec<HsStreamEvent>, HsError> {
        self.inner.user_events(hs, user, cursor, limit).await
    }

    async fn get_resource(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        if resource.to_string() == self.uri {
            return Ok(self.blob.clone());
        }
        self.inner.get_resource(resource).await
    }

    async fn get_blob(&self, resource: &PubkyResource) -> Result<Vec<u8>, HsError> {
        self.inner.get_blob(resource).await
    }
}

/// What one [`read_since`] read.
struct Read {
    /// Newest first.
    notifications: Vec<Notification>,
    /// Whether the page budget ran out before the end was reached.
    cut_short: bool,
}

/// The notifications of `user_id` from `source` at most as recent as `start` and at least
/// as recent as `end`, paging newest first, at most [`MAX_PAGES`] pages. Pages are chained
/// by timestamp, not by an ever-growing skip, so a long history isn't capped by the
/// endpoint's skip limit. A page that doesn't go back in time (a source repeating itself)
/// fails the read, so the source is backed off; so does a rate limit.
async fn read_since(
    source: &dyn NotificationSource,
    user_id: &str,
    mut start: Option<i64>,
    end: Option<i64>,
) -> Result<Read, DynError> {
    let mut notifications: Vec<Notification> = Vec::new();
    let mut skip = 0;
    for _ in 0..MAX_PAGES {
        let page = match source.page(user_id, start, end, skip).await? {
            NexusAnswer::Found(page) => page,
            // It doesn't know the seed (yet): nothing to read.
            NexusAnswer::NotFound => Vec::new(),
            NexusAnswer::RateLimited(wait) => {
                return Err(format!("{} is rate limiting for {wait:?}", source.name()).into())
            }
        };
        let Some(oldest) = page.last().map(|notification| notification.timestamp) else {
            return Ok(Read {
                notifications,
                cut_short: false,
            });
        };
        let newest_first = page.windows(2).all(|w| w[0].timestamp >= w[1].timestamp);
        let goes_back = start.is_none_or(|start| page[0].timestamp <= start);
        if page.len() > PAGE_LIMIT || !newest_first || !goes_back {
            return Err(format!("{} sent a page out of order", source.name()).into());
        }
        let full_page = page.len() == PAGE_LIMIT;
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
            return Ok(Read {
                notifications,
                cut_short: false,
            });
        }
    }
    Ok(Read {
        notifications,
        cut_short: true,
    })
}

async fn read_catch_up(key: &str) -> Result<Option<(i64, i64)>, DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let value: Option<String> = redis_conn.get(key).await?;
    Ok(value.and_then(|value| {
        let (oldest, newest) = value.split_once(':')?;
        Some((oldest.parse().ok()?, newest.parse().ok()?))
    }))
}

async fn write_catch_up(key: &str, oldest_read: i64, newest: i64) -> Result<(), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn
        .set(key, format!("{oldest_read}:{newest}"))
        .await?;
    Ok(())
}

async fn clear_catch_up(key: &str) -> Result<(), DynError> {
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn.del(key).await?;
    Ok(())
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
    use nexus_common::models::notification::PostChangedSource;
    use nexus_common::utils::test_utils::random_pubky_id;
    use pubky_app_specs::{
        post_uri_builder, PubkyAppBookmark, PubkyAppFollow, PubkyAppPost, PubkyAppPostEmbed,
        PubkyAppPostKind,
    };
    use std::sync::Mutex;

    fn ids() -> (String, String) {
        (random_pubky_id().to_string(), random_pubky_id().to_string())
    }

    /// Every notification maps to the record it is about: the post itself, or the follow
    /// or tag rebuilt from who made it and what it targets.
    #[test]
    fn every_notification_maps_to_the_record_it_is_about() {
        use NotificationBody::*;
        let (seed, carol) = ids();
        let seed_post = post_uri_builder(seed.clone(), "0033SSE3B1FQ0".into());
        let carol_post = post_uri_builder(carol.clone(), "0033SSE3B1FQ1".into());
        let tag = |tagged: &str, label: &str| {
            let tag = PubkyAppTag {
                uri: tagged.to_string(),
                label: label.to_string(),
                created_at: 0,
            };
            tag_uri_builder(carol.clone(), tag.create_id())
        };
        let follow = format!("pubky://{carol}/pub/pubky.app/follows/{seed}");
        let profile = format!("pubky://{seed}/pub/pubky.app/profile.json");
        let kind = PubkyAppPostKind::Short;

        let cases = [
            (
                Follow {
                    followed_by: carol.clone(),
                },
                follow.clone(),
            ),
            (
                NewFriend {
                    followed_by: carol.clone(),
                },
                follow.clone(),
            ),
            (
                LostFriend {
                    unfollowed_by: carol.clone(),
                },
                follow,
            ),
            (
                TagPost {
                    tagged_by: carol.clone(),
                    tag_label: "rust".into(),
                    post_uri: seed_post.clone(),
                    post_kind: kind.clone(),
                },
                tag(&seed_post, "rust"),
            ),
            (
                UntagPost {
                    untagged_by: carol.clone(),
                    tag_label: "rust".into(),
                    post_uri: seed_post.clone(),
                    post_kind: kind.clone(),
                },
                tag(&seed_post, "rust"),
            ),
            (
                TagProfile {
                    tagged_by: carol.clone(),
                    tag_label: "friend".into(),
                },
                tag(&profile, "friend"),
            ),
            (
                UntagProfile {
                    untagged_by: carol.clone(),
                    tag_label: "friend".into(),
                },
                tag(&profile, "friend"),
            ),
            (
                Reply {
                    replied_by: carol.clone(),
                    parent_post_uri: seed_post.clone(),
                    reply_uri: carol_post.clone(),
                    post_kind: kind.clone(),
                },
                carol_post.clone(),
            ),
            (
                Repost {
                    reposted_by: carol.clone(),
                    embed_uri: seed_post.clone(),
                    repost_uri: carol_post.clone(),
                    post_kind: kind.clone(),
                },
                carol_post.clone(),
            ),
            (
                Mention {
                    mentioned_by: carol.clone(),
                    post_uri: carol_post.clone(),
                    post_kind: kind.clone(),
                },
                carol_post.clone(),
            ),
            (
                PostEdited {
                    edit_source: PostChangedSource::ReplyParent,
                    edited_by: carol.clone(),
                    edited_uri: carol_post.clone(),
                    linked_uri: seed_post.clone(),
                    post_kind: kind.clone(),
                },
                carol_post.clone(),
            ),
            (
                PostDeleted {
                    delete_source: PostChangedSource::Reply,
                    deleted_by: carol.clone(),
                    deleted_uri: carol_post.clone(),
                    linked_uri: seed_post,
                    post_kind: kind,
                },
                carol_post,
            ),
        ];
        for (body, record) in cases {
            assert_eq!(record_of(&body, &seed), record, "{body:?}");
        }
    }

    /// Only records that concern the seed are indexed, whatever a big Nexus pointed at.
    #[test]
    fn only_records_about_the_seed_concern_it() {
        let (seed, carol) = ids();
        let seed_post = post_uri_builder(seed.clone(), "0033SSE3B1FQ0".into());
        let other_post = post_uri_builder(carol.clone(), "0033SSE3B1FQ1".into());
        let post_id = Resource::Post("0033SSE3B1FQ2".into());
        let post = |parent: Option<&str>, embed: Option<&str>, content: &str| {
            PubkyAppObject::Post(PubkyAppPost {
                content: content.to_string(),
                kind: PubkyAppPostKind::Short,
                parent: parent.map(String::from),
                embed: embed.map(|uri| PubkyAppPostEmbed {
                    kind: PubkyAppPostKind::Short,
                    uri: uri.to_string(),
                }),
                attachments: None,
                lock: None,
            })
        };
        let tag = |tagged: &str| {
            PubkyAppObject::Tag(PubkyAppTag {
                uri: tagged.to_string(),
                label: "x".into(),
                created_at: 0,
            })
        };
        let follow = PubkyAppObject::Follow(PubkyAppFollow { created_at: 0 });
        let followee = |id: &str| Resource::Follow(PubkyId::try_from(id).unwrap());
        let tag_id = Resource::Tag("ABC".into());
        let profile = format!("pubky://{seed}/pub/pubky.app/profile.json");

        let concerning = [
            (follow.clone(), followee(&seed)),
            (tag(&seed_post), tag_id.clone()),
            (tag(&profile), tag_id.clone()),
            (post(Some(&seed_post), None, "hi"), post_id.clone()),
            (post(None, Some(&seed_post), ""), post_id.clone()),
            (
                post(None, None, &format!("hi pubky{seed}")),
                post_id.clone(),
            ),
        ];
        for (object, resource) in concerning {
            assert!(concerns_seed(&object, &resource, &seed), "{object:?}");
        }
        let unrelated = [
            (follow, followee(&carol)),
            (tag(&other_post), tag_id),
            (post(Some(&other_post), None, "hi"), post_id.clone()),
            (post(None, Some(&other_post), "hi"), post_id.clone()),
            (post(None, None, "nothing about anyone"), post_id),
            (
                PubkyAppObject::Bookmark(PubkyAppBookmark {
                    uri: seed_post,
                    created_at: 0,
                }),
                Resource::Bookmark("B".into()),
            ),
        ];
        for (object, resource) in unrelated {
            assert!(!concerns_seed(&object, &resource, &seed), "{object:?}");
        }
    }

    /// Notifications served from memory, newest first, as the endpoint pages them; each
    /// request is recorded.
    #[derive(Default)]
    struct FakeNotifications {
        notifications: Vec<Notification>,
        out_of_order: bool,
        asked: Mutex<Vec<(Option<i64>, usize)>>,
    }

    #[async_trait::async_trait]
    impl NotificationSource for FakeNotifications {
        fn name(&self) -> &str {
            "fake"
        }

        async fn page(
            &self,
            _user_id: &str,
            start: Option<i64>,
            end: Option<i64>,
            skip: usize,
        ) -> Result<NexusAnswer<Vec<Notification>>, DynError> {
            self.asked.lock().unwrap().push((start, skip));
            let mut matching: Vec<Notification> = self
                .notifications
                .iter()
                .filter(|n| start.is_none_or(|start| n.timestamp <= start))
                .filter(|n| end.is_none_or(|end| n.timestamp >= end))
                .map(|n| Notification {
                    timestamp: n.timestamp,
                    body: n.body.clone(),
                })
                .collect();
            matching.sort_by_key(|n| std::cmp::Reverse(n.timestamp));
            let mut page: Vec<Notification> =
                matching.into_iter().skip(skip).take(PAGE_LIMIT).collect();
            if self.out_of_order {
                page.reverse();
            }
            Ok(NexusAnswer::Found(page))
        }
    }

    fn follow_at(timestamp: i64) -> Notification {
        Notification {
            timestamp,
            body: NotificationBody::Follow {
                followed_by: random_pubky_id().to_string(),
            },
        }
    }

    /// Everything since the position is read across pages, notifications sharing a
    /// timestamp at a page's edge included.
    #[tokio::test]
    async fn reads_every_notification_since_the_position_across_pages() {
        // 250, with a run of the same timestamp across the first page's edge.
        let notifications: Vec<Notification> = (0..250)
            .map(|i| follow_at(if (90..110).contains(&i) { 1_000 } else { i }))
            .collect();
        let source = FakeNotifications {
            notifications,
            ..Default::default()
        };

        let read = read_since(&source, "seed", None, Some(0)).await.unwrap();

        assert_eq!(read.notifications.len(), 250);
        assert!(!read.cut_short);
    }

    #[tokio::test]
    async fn a_source_sending_pages_out_of_order_fails_the_read() {
        let source = FakeNotifications {
            notifications: (0..5).map(follow_at).collect(),
            out_of_order: true,
            ..Default::default()
        };

        assert!(read_since(&source, "seed", None, None).await.is_err());
    }

    /// One poll reads at most a page budget; the rest is left for the next polls.
    #[tokio::test]
    async fn a_long_history_is_cut_short_at_the_page_budget() {
        let notifications: Vec<Notification> = (0..(MAX_PAGES * PAGE_LIMIT + 10) as i64)
            .map(follow_at)
            .collect();
        let source = FakeNotifications {
            notifications,
            ..Default::default()
        };

        let read = read_since(&source, "seed", None, None).await.unwrap();

        assert!(read.cut_short);
        assert_eq!(read.notifications.len(), MAX_PAGES * PAGE_LIMIT);
        assert_eq!(source.asked.lock().unwrap().len(), MAX_PAGES);
    }
}
