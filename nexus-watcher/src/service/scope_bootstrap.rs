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
/// Tries per page before the bootstrap gives up and crawling takes over.
const MAX_ATTEMPTS: u32 = 3;
/// Requests to the big Nexus in flight at once.
const CONCURRENT_REQUESTS: usize = 8;
/// Requests in one bootstrap, retries included. Past it, the bootstrap stops with what it
/// found and crawling finds the rest.
const MAX_REQUESTS: usize = 20_000;
/// Users suggested in one bootstrap, for the same reason.
const MAX_SUGGESTED: usize = 50_000;
/// How long one request to the big Nexus may take, body included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Waits for a rate limit (`429`) per page, each at most [`MAX_RATE_LIMIT_WAIT`], before
/// the bootstrap gives up.
const MAX_RATE_LIMIT_WAITS: u32 = 5;
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);
/// How long a rate-limited request waits when the Nexus doesn't say.
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(5);
/// How long a suggested user counts as a member without the graph confirming it.
const SUGGESTION_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

/// One answer to "who does this user follow?".
#[derive(Debug, Clone, PartialEq)]
pub enum FollowingPage {
    /// A page of followees, as the source sent them: the caller keeps the valid ids.
    Followees(Vec<String>),
    /// The source doesn't know the user.
    Unknown,
    /// The source asks to wait this long before asking again (`429`).
    RateLimited(Duration),
}

/// Where "who does this user follow?" is asked: a big Nexus over HTTP, or a fake in tests.
#[async_trait::async_trait]
pub trait FollowingSource: Send + Sync {
    /// One page of the user's followees, skipping `skip` entries, in one request: the
    /// caller does the retrying, so every request counts against its budget.
    async fn following(&self, user_id: &str, skip: usize) -> Result<FollowingPage, DynError>;
}

/// A big Nexus's `GET /v0/user/{id}/following`.
pub struct NexusFollowing {
    /// Validated in the config: http(s), no query or fragment, no trailing slash.
    pub nexus_url: String,
}

#[async_trait::async_trait]
impl FollowingSource for NexusFollowing {
    async fn following(&self, user_id: &str, skip: usize) -> Result<FollowingPage, DynError> {
        let url = format!(
            "{}/v0/user/{user_id}/following?skip={skip}&limit={PAGE_LIMIT}",
            self.nexus_url
        );
        let pubky = PubkyConnector::get()?;
        read_page(&url, pubky.client().request(Method::GET, &url).send()).await
    }
}

/// One request, from `send` to the end of its body, within [`REQUEST_TIMEOUT`]: a Nexus
/// that sends headers and stalls its body is cut off too.
async fn read_page(
    url: &str,
    send: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
) -> Result<FollowingPage, DynError> {
    let read = async {
        let response = send.await?;
        match response.status() {
            reqwest::StatusCode::NOT_FOUND => Ok(FollowingPage::Unknown),
            reqwest::StatusCode::TOO_MANY_REQUESTS => {
                Ok(FollowingPage::RateLimited(retry_after(&response)))
            }
            status if !status.is_success() => Err(format!("GET {url}: HTTP {status}").into()),
            _ => {
                let body = fetch_capped(response, MAX_RESOURCE_SIZE as u64)
                    .await
                    .map_err(|e| format!("GET {url}: {e:?}"))?;
                Ok(FollowingPage::Followees(serde_json::from_slice(&body)?))
            }
        }
    };
    tokio::time::timeout(REQUEST_TIMEOUT, read)
        .await
        .map_err(|_| format!("GET {url}: timed out"))?
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

/// The requests one bootstrap may still send. Taken before every request, retries and
/// rate-limited ones included.
struct RequestBudget {
    sent: AtomicUsize,
    max: usize,
}

impl RequestBudget {
    fn new(max: usize) -> Self {
        Self {
            sent: AtomicUsize::new(0),
            max,
        }
    }

    /// Whether one more request may be sent, counting it if so.
    fn take(&self) -> bool {
        self.sent.fetch_add(1, Ordering::Relaxed) < self.max
    }

    fn is_spent(&self) -> bool {
        self.sent.load(Ordering::Relaxed) >= self.max
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
    suggested_members_within(seed, hops, source, &RequestBudget::new(MAX_REQUESTS)).await
}

async fn suggested_members_within(
    seed: &str,
    hops: u8,
    source: &dyn FollowingSource,
    budget: &RequestBudget,
) -> Result<HashMap<String, u8>, DynError> {
    let mut seen = HashSet::from([seed.to_string()]);
    let mut suggested = HashMap::new();
    let mut frontier = vec![seed.to_string()];

    'hops: for hop in 1..=hops {
        // A hop's users are asked about concurrently; the answers are merged in frontier
        // order, so the result is the same as asking one at a time.
        let mut answers = stream::iter(frontier)
            .map(|user_id| async move { all_following(&user_id, source, budget).await })
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
        if budget.is_spent() {
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

/// Who the user follows, at most [`MAX_FOLLOWS_PER_USER`] entries read; fewer once the
/// budget runs out. Only valid user ids are kept, as each goes into a later request's URL,
/// but paging moves on by every entry read, so a page of anything else can't stall it.
///
/// Each request is taken from `budget`. A rate limit is waited out as the source asks, at
/// most [`MAX_RATE_LIMIT_WAITS`] times per page; any other failure is tried again, at most
/// [`MAX_ATTEMPTS`] times per page, then the bootstrap gives up.
async fn all_following(
    user_id: &str,
    source: &dyn FollowingSource,
    budget: &RequestBudget,
) -> Result<Vec<String>, DynError> {
    let mut followees = Vec::new();
    let mut read = 0;
    let (mut failures, mut waits) = (0, 0);
    while read < MAX_FOLLOWS_PER_USER {
        if !budget.take() {
            break;
        }
        match source.following(user_id, read).await {
            Ok(FollowingPage::Followees(mut page)) => {
                (failures, waits) = (0, 0);
                let full_page = page.len() >= PAGE_LIMIT;
                page.truncate(PAGE_LIMIT);
                read += page.len();
                followees.extend(
                    page.into_iter()
                        .filter(|id| PubkyId::try_from(id.as_str()).is_ok()),
                );
                if !full_page {
                    break;
                }
            }
            Ok(FollowingPage::Unknown) => break,
            Ok(FollowingPage::RateLimited(wait)) if waits < MAX_RATE_LIMIT_WAITS => {
                waits += 1;
                tokio::time::sleep(wait).await;
            }
            Ok(FollowingPage::RateLimited(_)) => {
                return Err(format!("still rate limited after {waits} waits").into())
            }
            Err(e) if failures + 1 < MAX_ATTEMPTS => {
                failures += 1;
                warn!(attempt = failures, error = %e, "Asking the big Nexus failed; retrying");
                tokio::time::sleep(Duration::from_secs(2u64.pow(failures))).await;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(followees)
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
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Who follows whom, served from memory, recording each request (user, skip). Scripted
    /// answers are given first; `down` fails every other request.
    #[derive(Default)]
    struct FakeFollowing {
        graph: HashMap<String, Vec<String>>,
        script: Mutex<VecDeque<Result<FollowingPage, String>>>,
        down: bool,
        asked: Mutex<Vec<(String, usize)>>,
    }

    impl FakeFollowing {
        fn new(graph: HashMap<String, Vec<String>>) -> Self {
            Self {
                graph,
                ..Default::default()
            }
        }

        fn scripted(
            graph: HashMap<String, Vec<String>>,
            script: Vec<Result<FollowingPage, String>>,
        ) -> Self {
            Self {
                graph,
                script: Mutex::new(script.into()),
                ..Default::default()
            }
        }

        fn requests(&self) -> usize {
            self.asked.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl FollowingSource for FakeFollowing {
        async fn following(&self, user_id: &str, skip: usize) -> Result<FollowingPage, DynError> {
            self.asked.lock().unwrap().push((user_id.to_string(), skip));
            if let Some(answer) = self.script.lock().unwrap().pop_front() {
                return answer.map_err(Into::into);
            }
            if self.down {
                return Err("the big Nexus is down".into());
            }
            Ok(match self.graph.get(user_id) {
                Some(followees) => FollowingPage::Followees(
                    followees
                        .iter()
                        .skip(skip)
                        .take(PAGE_LIMIT)
                        .cloned()
                        .collect(),
                ),
                None => FollowingPage::Unknown,
            })
        }
    }

    fn ids(count: usize) -> Vec<String> {
        (0..count).map(|_| random_pubky_id().to_string()).collect()
    }

    #[tokio::test]
    async fn suggests_everyone_within_the_hops_at_their_shortest_distance() {
        let [seed, b, c, d, e] = [(); 5].map(|_| random_pubky_id().to_string());
        let source = FakeFollowing::new(HashMap::from([
            (seed.clone(), vec![b.clone(), c.clone()]),
            (b.clone(), vec![c.clone(), d.clone(), seed.clone()]),
            (d.clone(), vec![e]),
        ]));

        let suggested = suggested_members(&seed, 2, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(b, 1), (c, 1), (d, 2)]));
        // The seed, then b and c: one request each, none past the last hop.
        assert_eq!(source.requests(), 3);
    }

    #[tokio::test]
    async fn pages_through_long_follow_lists() {
        let seed = random_pubky_id().to_string();
        let source = FakeFollowing::new(HashMap::from([(seed.clone(), ids(450))]));

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested.len(), 450);
        assert_eq!(source.requests(), 3, "200 + 200 + 50");
    }

    #[tokio::test]
    async fn stops_at_the_most_follows_read_for_one_user() {
        let seed = random_pubky_id().to_string();
        let many = ids(MAX_FOLLOWS_PER_USER + 500);
        let source = FakeFollowing::new(HashMap::from([(seed.clone(), many)]));

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested.len(), MAX_FOLLOWS_PER_USER);
        assert_eq!(source.requests(), MAX_FOLLOWS_PER_USER / PAGE_LIMIT);
    }

    /// Whatever isn't a user id is dropped before it can go into a request's URL.
    #[tokio::test]
    async fn drops_answers_that_are_not_user_ids() {
        let [seed, friend] = [(); 2].map(|_| random_pubky_id().to_string());
        let bad = "../../admin?x=".to_string();
        let source = FakeFollowing::new(HashMap::from([(seed.clone(), vec![bad, friend.clone()])]));

        let suggested = suggested_members(&seed, 2, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(friend.clone(), 1)]));
        assert_eq!(
            *source.asked.lock().unwrap(),
            vec![(seed, 0), (friend, 0)],
            "never asked about it"
        );
    }

    /// Paging moves on by every entry read, so a full page of non-ids can't stall it on
    /// the same page.
    #[tokio::test]
    async fn a_page_of_non_ids_doesnt_stall_paging() {
        let [seed, friend] = [(); 2].map(|_| random_pubky_id().to_string());
        let mut followees: Vec<String> =
            (0..PAGE_LIMIT).map(|i| format!("not-an-id-{i}")).collect();
        followees.push(friend.clone());
        let source = FakeFollowing::new(HashMap::from([(seed.clone(), followees)]));

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(friend, 1)]));
        assert_eq!(
            *source.asked.lock().unwrap(),
            vec![(seed.clone(), 0), (seed, PAGE_LIMIT)]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_big_nexus_suggests_nobody() {
        let source = FakeFollowing {
            down: true,
            ..Default::default()
        };

        let suggested = suggested_members(&random_pubky_id(), 2, &source).await;

        assert!(suggested.is_err());
        assert_eq!(
            source.requests(),
            MAX_ATTEMPTS as usize,
            "tried, then given up"
        );
    }

    /// A rate limit is waited out as the source asks; one that never lifts fails the
    /// bootstrap once the waits run out.
    #[tokio::test(start_paused = true)]
    async fn waits_out_a_rate_limit() {
        let [seed, friend] = [(); 2].map(|_| random_pubky_id().to_string());
        let source = FakeFollowing::scripted(
            HashMap::from([(seed.clone(), vec![friend.clone()])]),
            vec![Ok(FollowingPage::RateLimited(Duration::from_secs(7)))],
        );
        let started = tokio::time::Instant::now();

        let suggested = suggested_members(&seed, 1, &source).await.unwrap();

        assert_eq!(suggested, HashMap::from([(friend, 1)]));
        assert_eq!(source.requests(), 2);
        assert!(
            started.elapsed() >= Duration::from_secs(7),
            "waited as asked"
        );

        let limited = (0..=MAX_RATE_LIMIT_WAITS)
            .map(|_| Ok(FollowingPage::RateLimited(Duration::from_secs(1))))
            .collect();
        let source = FakeFollowing::scripted(HashMap::new(), limited);
        assert!(suggested_members(&seed, 1, &source).await.is_err());
        assert_eq!(source.requests(), MAX_RATE_LIMIT_WAITS as usize + 1);
    }

    /// Every request is taken from the budget, retries and rate-limited ones included.
    #[tokio::test(start_paused = true)]
    async fn retries_count_against_the_budget() {
        let seed = random_pubky_id().to_string();
        for source in [
            FakeFollowing {
                down: true,
                ..Default::default()
            },
            FakeFollowing::scripted(
                HashMap::new(),
                vec![Ok(FollowingPage::RateLimited(Duration::from_secs(1))); 4],
            ),
        ] {
            let suggested =
                suggested_members_within(&seed, 1, &source, &RequestBudget::new(2)).await;

            assert_eq!(suggested.unwrap(), HashMap::new(), "stopped by the budget");
            assert_eq!(source.requests(), 2, "two requests sent, as budgeted");
        }
    }

    /// A body that never arrives is cut off by the same timeout as the headers.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_body_times_out() {
        let stalled = futures::stream::pending::<Result<bytes::Bytes, std::io::Error>>();
        let response =
            reqwest::Response::from(http::Response::new(reqwest::Body::wrap_stream(stalled)));

        // A deadline of its own, so the test fails rather than hangs if the body is read
        // outside the request's timeout.
        let deadline = REQUEST_TIMEOUT + Duration::from_secs(1);
        let read =
            tokio::time::timeout(deadline, read_page("url", std::future::ready(Ok(response))))
                .await
                .expect("cut off within the request's timeout");

        assert!(read.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn reads_how_long_a_rate_limit_asks_to_wait() {
        let limited = |retry_after: &str| {
            let response = http::Response::builder()
                .status(429)
                .header("retry-after", retry_after)
                .body("")
                .unwrap();
            read_page(
                "url",
                std::future::ready(Ok(reqwest::Response::from(response))),
            )
        };

        assert_eq!(
            limited("7").await.unwrap(),
            FollowingPage::RateLimited(Duration::from_secs(7))
        );
        assert_eq!(
            limited("100000").await.unwrap(),
            FollowingPage::RateLimited(MAX_RATE_LIMIT_WAIT),
            "at most the longest wait"
        );
    }
}
