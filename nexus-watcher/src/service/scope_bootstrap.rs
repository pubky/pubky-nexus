//! Finds a new seed scope's members by asking a big Nexus who the seed follows, and who
//! they follow, up to the hops, so they are all read at once instead of hop by hop.
//!
//! Only a suggestion: each suggested user counts as a member for a day, so its own stream
//! is read from its own homeserver, and the follows read that way then confirm it or let
//! it go (see the scope maintenance). Content never comes from the big Nexus.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use deadpool_redis::redis::AsyncCommands;
use futures::{stream, StreamExt, TryStreamExt};
use nexus_common::db::kv::{RedisError, RedisResult};
use nexus_common::db::{get_redis_conn, PubkyConnector};
use nexus_common::types::DynError;
use pubky::Method;
use pubky_app_specs::PubkyId;
use tracing::{info, warn};

use super::scope_maintenance::ScopeMaintainer;
use crate::homeserver::fetch::{fetch_capped, MAX_RESOURCE_SIZE};

/// Follows asked for per page, the most `/v0/user/{id}/following` serves.
const PAGE_LIMIT: usize = 200;
/// The most follows read for one user.
const MAX_FOLLOWS_PER_USER: usize = 10_000;
/// Tries per request before the bootstrap gives up and crawling takes over.
const MAX_ATTEMPTS: u32 = 3;
/// Requests to the big Nexus in flight at once.
const CONCURRENT_REQUESTS: usize = 8;
/// Requests in one bootstrap. Past it, the bootstrap stops with what it found and
/// crawling finds the rest.
const MAX_REQUESTS: usize = 20_000;
/// Users suggested in one bootstrap, for the same reason.
const MAX_SUGGESTED: usize = 50_000;
/// How long one request to the big Nexus may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a suggested user counts as a member without the graph confirming it.
const SUGGESTION_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

/// Where "who does this user follow?" is asked: a big Nexus over HTTP, or a fake in tests.
#[async_trait::async_trait]
pub trait FollowingSource: Send + Sync {
    /// One page of the user's followees, skipping `skip`; `None` if the source doesn't
    /// know the user.
    async fn following(&self, user_id: &str, skip: usize) -> Result<Option<Vec<String>>, DynError>;
}

/// A big Nexus's `GET /v0/user/{id}/following`.
pub struct NexusFollowing {
    pub nexus_url: String,
}

#[async_trait::async_trait]
impl FollowingSource for NexusFollowing {
    async fn following(&self, user_id: &str, skip: usize) -> Result<Option<Vec<String>>, DynError> {
        let url = format!(
            "{}/v0/user/{user_id}/following?skip={skip}&limit={PAGE_LIMIT}",
            self.nexus_url
        );
        let request = PubkyConnector::get()?
            .client()
            .request(Method::GET, &url)
            .send();
        let response = tokio::time::timeout(REQUEST_TIMEOUT, request)
            .await
            .map_err(|_| format!("GET {url}: timed out"))??;
        match response.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(None),
            status if !status.is_success() => Err(format!("GET {url}: HTTP {status}").into()),
            _ => {
                let body = fetch_capped(response, MAX_RESOURCE_SIZE as u64)
                    .await
                    .map_err(|e| format!("GET {url}: {e:?}"))?;
                Ok(Some(serde_json::from_slice(&body)?))
            }
        }
    }
}

/// Everyone the seed reaches in at most `hops` follows according to `source`, with their
/// distance. The seed itself is left out: it is always a member.
///
/// Bounded whatever the source answers: it stops at [`MAX_REQUESTS`] requests or
/// [`MAX_SUGGESTED`] users with what it found so far.
pub async fn suggested_members(
    seed: &str,
    hops: u8,
    source: &dyn FollowingSource,
) -> Result<HashMap<String, u8>, DynError> {
    let mut seen = HashSet::from([seed.to_string()]);
    let mut suggested = HashMap::new();
    let mut frontier = vec![seed.to_string()];
    let requests = AtomicUsize::new(0);

    'hops: for hop in 1..=hops {
        // A hop's users are asked about concurrently; the answers are merged in frontier
        // order, so the result is the same as asking one at a time.
        let mut answers = stream::iter(frontier)
            .map(|user_id| {
                let requests = &requests;
                async move { all_following(&user_id, source, requests).await }
            })
            .buffered(CONCURRENT_REQUESTS);
        let mut next = Vec::new();
        while let Some(followees) = answers.try_next().await? {
            for followee in followees {
                if suggested.len() >= MAX_SUGGESTED {
                    warn!(
                        MAX_SUGGESTED,
                        "Bootstrap suggested as many users as it may; crawling finds the rest"
                    );
                    break 'hops;
                }
                if seen.insert(followee.clone()) {
                    suggested.insert(followee.clone(), hop);
                    next.push(followee);
                }
            }
        }
        if requests.load(Ordering::Relaxed) >= MAX_REQUESTS {
            warn!(
                MAX_REQUESTS,
                "Bootstrap made as many requests as it may; crawling finds the rest"
            );
            break;
        }
        frontier = next;
    }
    Ok(suggested)
}

/// Who the user follows, at most [`MAX_FOLLOWS_PER_USER`]; fewer once the bootstrap's
/// requests run out.
async fn all_following(
    user_id: &str,
    source: &dyn FollowingSource,
    requests: &AtomicUsize,
) -> Result<Vec<String>, DynError> {
    let mut followees = Vec::new();
    while followees.len() < MAX_FOLLOWS_PER_USER {
        if requests.fetch_add(1, Ordering::Relaxed) >= MAX_REQUESTS {
            break;
        }
        let Some(mut page) = with_retries(|| source.following(user_id, followees.len())).await?
        else {
            break;
        };
        let full_page = page.len() >= PAGE_LIMIT;
        page.truncate(PAGE_LIMIT);
        followees.extend(page);
        if !full_page {
            break;
        }
    }
    Ok(followees)
}

/// Runs `request` up to [`MAX_ATTEMPTS`] times, waiting longer after each failure.
async fn with_retries<T, F, Fut>(mut request: F) -> Result<T, DynError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, DynError>>,
{
    let mut attempt = 1;
    loop {
        match request().await {
            Ok(value) => return Ok(value),
            Err(e) if attempt < MAX_ATTEMPTS => {
                warn!(attempt, error = %e, "Asking the big Nexus failed; retrying");
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Where a seed scope's suggested users wait, scored by when each suggestion expires. Kept
/// per seed and hop count: a narrower scope never inherits a wider one's suggestions.
fn suggested_key(seed: &str, hops: u8) -> String {
    format!("Scope:Suggested:{seed}:{hops}")
}

/// Set once a seed scope was bootstrapped, so a restart doesn't ask the big Nexus again.
fn bootstrapped_key(seed: &str, hops: u8) -> String {
    format!("Scope:Bootstrapped:{seed}:{hops}")
}

/// The users suggested for the seed scope whose suggestion hasn't expired; expired ones
/// are dropped.
pub(crate) async fn unexpired_suggestions(seed: &str, hops: u8) -> RedisResult<Vec<String>> {
    let now = chrono::Utc::now().timestamp_millis();
    let key = suggested_key(seed, hops);
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn
        .zrembyscore(&key, "-inf", now)
        .await
        .map_err(RedisError::from)?;
    redis_conn
        .zrangebyscore(&key, now, "+inf")
        .await
        .map_err(RedisError::from)
}

/// Counts `user_ids` as members of the seed scope for [`SUGGESTION_TTL_MS`].
pub async fn suggest_members(seed: &str, hops: u8, user_ids: &[String]) -> RedisResult<()> {
    if user_ids.is_empty() {
        return Ok(());
    }
    let expires_at = chrono::Utc::now().timestamp_millis() + SUGGESTION_TTL_MS;
    let scored: Vec<(i64, &str)> = user_ids
        .iter()
        .map(|id| (expires_at, id.as_str()))
        .collect();
    let mut redis_conn = get_redis_conn().await?;
    let () = redis_conn
        .zadd_multiple(suggested_key(seed, hops), &scored)
        .await
        .map_err(RedisError::from)?;
    Ok(())
}

/// Suggests the seed scope's members from `source`, unless that was done before for this
/// seed and hop count, and has the maintainer admit them at once.
///
/// Errors are logged, never returned: crawling finds the members anyway, more slowly,
/// and the next start tries again.
pub async fn bootstrap_seed_scope(maintainer: &ScopeMaintainer, source: &dyn FollowingSource) {
    let Some((seed, hops)) = maintainer.seed_scope() else {
        return;
    };
    let mut redis_conn = match get_redis_conn().await {
        Ok(redis_conn) => redis_conn,
        Err(e) => {
            warn!(error = %e, "Could not check the scope bootstrap; skipping it");
            return;
        }
    };
    let done: Result<bool, _> = redis_conn.exists(bootstrapped_key(seed, hops)).await;
    match done {
        Ok(false) => {}
        Ok(true) => return,
        Err(e) => {
            warn!(error = %e, "Could not check the scope bootstrap; skipping it");
            return;
        }
    }

    let suggested = match suggested_members(seed, hops, source).await {
        Ok(suggested) => suggested,
        Err(e) => {
            warn!(error = %e, "Could not ask the big Nexus for members; they are found by crawling");
            return;
        }
    };
    info!(
        suggested = suggested.len(),
        "The big Nexus suggested the seed scope's members"
    );

    let ids: Vec<String> = suggested
        .into_keys()
        .filter(|id| PubkyId::try_from(id.as_str()).is_ok())
        .collect();
    if let Err(e) = suggest_members(seed, hops, &ids).await {
        warn!(error = %e, "Could not store the suggested members");
        return;
    }
    match maintainer.recompute_now().await {
        Ok(change) => info!(?change, "Admitted the suggested members"),
        Err(e) => warn!(error = %e, "Could not admit the suggested members; the next run does"),
    }
    let now = chrono::Utc::now().timestamp_millis();
    let marked: Result<(), _> = redis_conn.set(bootstrapped_key(seed, hops), now).await;
    if let Err(e) = marked {
        warn!(error = %e, "Could not record the scope bootstrap; the next start repeats it");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Who follows whom, served from memory; `None` fails every request.
    struct FakeFollowing(Option<HashMap<&'static str, Vec<String>>>, Mutex<u32>);

    impl FakeFollowing {
        fn new(graph: Option<HashMap<&'static str, Vec<String>>>) -> Self {
            Self(graph, Mutex::new(0))
        }

        fn requests(&self) -> u32 {
            *self.1.lock().unwrap()
        }
    }

    #[async_trait::async_trait]
    impl FollowingSource for FakeFollowing {
        async fn following(
            &self,
            user_id: &str,
            skip: usize,
        ) -> Result<Option<Vec<String>>, DynError> {
            *self.1.lock().unwrap() += 1;
            let graph = self.0.as_ref().ok_or("the big Nexus is down")?;
            Ok(graph.get(user_id).map(|followees| {
                followees
                    .iter()
                    .skip(skip)
                    .take(PAGE_LIMIT)
                    .cloned()
                    .collect()
            }))
        }
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[tokio::test]
    async fn suggests_everyone_within_the_hops_at_their_shortest_distance() {
        let source = FakeFollowing::new(Some(HashMap::from([
            ("seed", ids(&["b", "c"])),
            ("b", ids(&["c", "d", "seed"])),
            ("d", ids(&["e"])),
        ])));

        let suggested = suggested_members("seed", 2, &source).await.unwrap();

        assert_eq!(
            suggested,
            HashMap::from([("b".into(), 1), ("c".into(), 1), ("d".into(), 2)])
        );
        // The seed, then b and c: one request each, none past the last hop.
        assert_eq!(source.requests(), 3);
    }

    #[tokio::test]
    async fn pages_through_long_follow_lists() {
        let many: Vec<String> = (0..450).map(|i| format!("u{i}")).collect();
        let source = FakeFollowing::new(Some(HashMap::from([("seed", many)])));

        let suggested = suggested_members("seed", 1, &source).await.unwrap();

        assert_eq!(suggested.len(), 450);
        assert_eq!(source.requests(), 3, "200 + 200 + 50");
    }

    #[tokio::test]
    async fn stops_at_the_most_follows_read_for_one_user() {
        let many: Vec<String> = (0..MAX_FOLLOWS_PER_USER + 500)
            .map(|i| format!("u{i}"))
            .collect();
        let source = FakeFollowing::new(Some(HashMap::from([("seed", many)])));

        let suggested = suggested_members("seed", 1, &source).await.unwrap();

        assert_eq!(suggested.len(), MAX_FOLLOWS_PER_USER);
        assert_eq!(
            source.requests() as usize,
            MAX_FOLLOWS_PER_USER / PAGE_LIMIT
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_big_nexus_suggests_nobody() {
        let source = FakeFollowing::new(None);

        let suggested = suggested_members("seed", 2, &source).await;

        assert!(suggested.is_err());
        assert_eq!(source.requests(), MAX_ATTEMPTS, "tried, then given up");
    }
}
