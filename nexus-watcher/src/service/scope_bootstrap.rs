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
/// Waits for a rate limit (`429`) per request, each at most [`MAX_RATE_LIMIT_WAIT`],
/// before the request counts as failed.
const MAX_RATE_LIMIT_WAITS: u32 = 5;
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);
/// How long a rate-limited request waits when the Nexus doesn't say.
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(5);
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
        let pubky = PubkyConnector::get()?;
        following_with(&url, || pubky.client().request(Method::GET, &url).send()).await
    }
}

/// One page from `send`, which GETs `url`: `None` for a 404. A rate limit (`429`) is
/// waited out as the Nexus asks (see [`MAX_RATE_LIMIT_WAITS`]) rather than counted as a
/// failure, as a big scope is bound to reach it.
async fn following_with<F, Fut>(url: &str, send: F) -> Result<Option<Vec<String>>, DynError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
{
    let mut waits = 0;
    loop {
        let response = tokio::time::timeout(REQUEST_TIMEOUT, send())
            .await
            .map_err(|_| format!("GET {url}: timed out"))??;
        match response.status() {
            reqwest::StatusCode::NOT_FOUND => return Ok(None),
            reqwest::StatusCode::TOO_MANY_REQUESTS if waits < MAX_RATE_LIMIT_WAITS => {
                waits += 1;
                tokio::time::sleep(retry_after(&response)).await;
            }
            status if !status.is_success() => {
                return Err(format!("GET {url}: HTTP {status}").into())
            }
            _ => {
                let body = fetch_capped(response, MAX_RESOURCE_SIZE as u64)
                    .await
                    .map_err(|e| format!("GET {url}: {e:?}"))?;
                return Ok(Some(serde_json::from_slice(&body)?));
            }
        }
    }
}

/// How long a `429` asks to wait (`Retry-After` in seconds), at most
/// [`MAX_RATE_LIMIT_WAIT`].
fn retry_after(response: &reqwest::Response) -> Duration {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok())
        .map_or(DEFAULT_RATE_LIMIT_WAIT, Duration::from_secs)
        .min(MAX_RATE_LIMIT_WAIT)
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
/// requests run out. Only valid user ids are kept: each one goes into a later request's
/// URL, so whatever else the Nexus answers is dropped.
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
        followees.extend(
            page.into_iter()
                .filter(|id| PubkyId::try_from(id.as_str()).is_ok()),
        );
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
    match is_bootstrapped(seed, hops).await {
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
    if suggested.is_empty() {
        // It may not know the seed yet: the next start asks again.
        return;
    }

    let ids: Vec<String> = suggested.into_keys().collect();
    if let Err(e) = suggest_members(seed, hops, &ids).await {
        warn!(error = %e, "Could not store the suggested members");
        return;
    }
    match maintainer.recompute_now().await {
        Ok(change) => info!(?change, "Admitted the suggested members"),
        Err(e) => warn!(error = %e, "Could not admit the suggested members; the next run does"),
    }
    if let Err(e) = mark_bootstrapped(seed, hops).await {
        warn!(error = %e, "Could not record the scope bootstrap; the next start repeats it");
    }
}

/// Whether the seed scope was bootstrapped before. Each check takes its own connection:
/// none is held while the big Nexus is asked or the members admitted.
async fn is_bootstrapped(seed: &str, hops: u8) -> RedisResult<bool> {
    let mut redis_conn = get_redis_conn().await?;
    redis_conn
        .exists(bootstrapped_key(seed, hops))
        .await
        .map_err(RedisError::from)
}

async fn mark_bootstrapped(seed: &str, hops: u8) -> RedisResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut redis_conn = get_redis_conn().await?;
    redis_conn
        .set(bootstrapped_key(seed, hops), now)
        .await
        .map_err(RedisError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::utils::test_utils::random_pubky_id;
    use std::sync::Mutex;

    /// Who follows whom, served from memory, recording who was asked about; `None`
    /// fails every request.
    struct FakeFollowing {
        graph: Option<HashMap<String, Vec<String>>>,
        asked: Mutex<Vec<String>>,
    }

    impl FakeFollowing {
        fn new(graph: Option<HashMap<String, Vec<String>>>) -> Self {
            Self {
                graph,
                asked: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> usize {
            self.asked.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl FollowingSource for FakeFollowing {
        async fn following(
            &self,
            user_id: &str,
            skip: usize,
        ) -> Result<Option<Vec<String>>, DynError> {
            self.asked.lock().unwrap().push(user_id.to_string());
            let graph = self.graph.as_ref().ok_or("the big Nexus is down")?;
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

    fn ids(count: usize) -> Vec<String> {
        (0..count).map(|_| random_pubky_id().to_string()).collect()
    }

    #[tokio::test]
    async fn suggests_everyone_within_the_hops_at_their_shortest_distance() {
        let [seed, b, c, d, e] = [(); 5].map(|_| random_pubky_id().to_string());
        let source = FakeFollowing::new(Some(HashMap::from([
            (seed.clone(), vec![b.clone(), c.clone()]),
            (b.clone(), vec![c.clone(), d.clone(), seed.clone()]),
            (d.clone(), vec![e]),
        ])));

        let suggested = suggested_members(&seed, 2, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(b, 1), (c, 1), (d, 2)]));
        // The seed, then b and c: one request each, none past the last hop.
        assert_eq!(source.requests(), 3);
    }

    #[tokio::test]
    async fn pages_through_long_follow_lists() {
        let seed = random_pubky_id().to_string();
        let source = FakeFollowing::new(Some(HashMap::from([(seed.clone(), ids(450))])));

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested.len(), 450);
        assert_eq!(source.requests(), 3, "200 + 200 + 50");
    }

    #[tokio::test]
    async fn stops_at_the_most_follows_read_for_one_user() {
        let seed = random_pubky_id().to_string();
        let many = ids(MAX_FOLLOWS_PER_USER + 500);
        let source = FakeFollowing::new(Some(HashMap::from([(seed.clone(), many)])));

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested.len(), MAX_FOLLOWS_PER_USER);
        assert_eq!(source.requests(), MAX_FOLLOWS_PER_USER / PAGE_LIMIT);
    }

    /// Whatever isn't a user id is dropped before it can go into a request's URL.
    #[tokio::test]
    async fn drops_answers_that_are_not_user_ids() {
        let [seed, friend] = [(); 2].map(|_| random_pubky_id().to_string());
        let bad = "../../admin?x=".to_string();
        let source = FakeFollowing::new(Some(HashMap::from([(
            seed.clone(),
            vec![bad.clone(), friend.clone()],
        )])));

        let suggested = suggested_members(&seed, 2, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(friend.clone(), 1)]));
        assert_eq!(
            *source.asked.lock().unwrap(),
            vec![seed, friend],
            "never asked about it"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_big_nexus_suggests_nobody() {
        let source = FakeFollowing::new(None);

        let suggested = suggested_members(&random_pubky_id(), 2, &source).await;

        assert!(suggested.is_err());
        assert_eq!(
            source.requests(),
            MAX_ATTEMPTS as usize,
            "tried, then given up"
        );
    }

    /// A response with `status`, an optional `Retry-After` and `body`.
    fn response(status: u16, retry_after: Option<&str>, body: &'static str) -> reqwest::Response {
        let mut builder = http::Response::builder().status(status);
        if let Some(seconds) = retry_after {
            builder = builder.header("retry-after", seconds);
        }
        reqwest::Response::from(builder.body(body).unwrap())
    }

    /// A rate limit is waited out as the Nexus asks, not counted as a failure; one that
    /// never lifts fails once the waits run out.
    #[tokio::test(start_paused = true)]
    async fn waits_out_a_rate_limit() {
        let sent = Mutex::new(0);
        let started = tokio::time::Instant::now();
        let page = following_with("url", || {
            let mut sent = sent.lock().unwrap();
            *sent += 1;
            let reply = if *sent == 1 {
                response(429, Some("7"), "")
            } else {
                response(200, None, "[\"a\"]")
            };
            std::future::ready(Ok(reply))
        })
        .await
        .unwrap();
        assert_eq!(page, Some(vec!["a".to_string()]));
        assert_eq!(*sent.lock().unwrap(), 2);
        assert!(
            started.elapsed() >= Duration::from_secs(7),
            "waited as asked"
        );

        let sent = Mutex::new(0);
        let limited = following_with("url", || {
            *sent.lock().unwrap() += 1;
            std::future::ready(Ok(response(429, Some("1000"), "")))
        })
        .await;
        assert!(limited.is_err());
        assert_eq!(*sent.lock().unwrap(), MAX_RATE_LIMIT_WAITS + 1);
    }
}
