//! Finds a new seed scope's members at start by asking a big Nexus who the seed follows,
//! and who they follow, up to the hops, so they are all polled at once instead of hop by
//! hop. Only a suggestion: content always comes from homeservers, and the members worked
//! out from the graph confirm or drop each suggested user (see `docs/decentralization.md`).

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures::{stream, StreamExt, TryStreamExt};
use nexus_common::types::DynError;
use pubky_app_specs::PubkyId;
use tracing::{info, warn};

use super::scope_maintenance::ScopeMaintainer;
use crate::events::get_nexus_json;

/// Follows asked for per page, the most `/v0/user/{id}/following` serves.
const PAGE_LIMIT: usize = 200;
/// The most follows read for one user: `/v0/user/{id}/following` skips at most this.
const MAX_FOLLOWS_PER_USER: usize = 10_000;
/// Tries per request before the bootstrap gives up and the crawl takes over.
const MAX_ATTEMPTS: u32 = 3;
/// Requests to the big Nexus in flight at once.
const CONCURRENT_REQUESTS: usize = 8;
/// Requests to the big Nexus in one bootstrap. Past it, the bootstrap stops with what
/// it found and crawling finds the rest.
const MAX_REQUESTS: usize = 20_000;
/// Users suggested in one bootstrap, for the same reason.
const MAX_SUGGESTED: usize = 50_000;

/// Where "who does this user follow?" is asked: a big Nexus over HTTP, or a fake in tests.
#[async_trait::async_trait]
pub trait FollowingSource: Send + Sync {
    /// One page of the user's followees; `None` if the source doesn't know the user.
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
        get_nexus_json(&url).await
    }
}

/// Everyone the seed reaches in at most `hops` follows according to `source`, with their
/// distance. The seed itself is left out: it is always a member.
///
/// Bounded whatever the source answers: each answer is merged as it arrives, and the
/// bootstrap stops at [`MAX_REQUESTS`] requests or [`MAX_SUGGESTED`] users with what it
/// found so far.
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

/// Who the user follows, at most [`MAX_FOLLOWS_PER_USER`]; less once the bootstrap's
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

/// Suggests the seed scope's members from `source`, unless that was done before for this
/// seed and hop count, and lets the maintainer admit them at once.
///
/// Errors are logged, never returned: crawling finds the members anyway, more slowly,
/// and the next start tries again.
pub async fn bootstrap_seed_scope(maintainer: &ScopeMaintainer, source: &dyn FollowingSource) {
    let Some(scope) = maintainer.seed_scope() else {
        return;
    };
    match scope.is_bootstrapped().await {
        Ok(false) => {}
        Ok(true) => return,
        Err(e) => {
            warn!(error = %e, "Could not check the scope bootstrap; skipping it");
            return;
        }
    }

    let suggested = match suggested_members(scope.seed(), scope.hops(), source).await {
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

    let ids: Vec<PubkyId> = suggested
        .keys()
        .filter_map(|id| PubkyId::try_from(id.as_str()).ok())
        .collect();
    if let Err(e) = scope.suggest_members(&ids).await {
        warn!(error = %e, "Could not store the suggested members");
        return;
    }
    match maintainer.recompute_now().await {
        Ok(change) => info!(?change, "Admitted the suggested members"),
        Err(e) => warn!(error = %e, "Could not admit the suggested members; the next run does"),
    }
    if let Err(e) = scope.mark_bootstrapped().await {
        warn!(error = %e, "Could not record the scope bootstrap; the next start repeats it");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Who follows whom, served from memory; `None` fails every request.
    struct FakeFollowing(Option<HashMap<&'static str, Vec<&'static str>>>, Mutex<u32>);

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
                    .map(|f| f.to_string())
                    .collect()
            }))
        }
    }

    #[tokio::test]
    async fn suggests_everyone_within_the_hops_at_their_shortest_distance() {
        let source = FakeFollowing(
            Some(HashMap::from([
                ("seed", vec!["b", "c"]),
                ("b", vec!["c", "d"]),
                ("d", vec!["e"]),
            ])),
            Mutex::new(0),
        );

        let suggested = suggested_members("seed", 2, &source).await.unwrap();

        assert_eq!(
            suggested,
            HashMap::from([("b".into(), 1), ("c".into(), 1), ("d".into(), 2)])
        );
        // The seed, then b and c: one request each, nothing past the last hop.
        assert_eq!(*source.1.lock().unwrap(), 3);
    }

    #[tokio::test]
    async fn pages_through_long_follow_lists() {
        let many: Vec<&'static str> = (0..450)
            .map(|i| &*Box::leak(format!("u{i}").into_boxed_str()))
            .collect();
        let source = FakeFollowing(Some(HashMap::from([("seed", many)])), Mutex::new(0));

        let suggested = suggested_members("seed", 1, &source).await.unwrap();

        assert_eq!(suggested.len(), 450);
        assert_eq!(*source.1.lock().unwrap(), 3, "200 + 200 + 50");
    }

    /// A source where everyone follows 10,000 users no one else follows, and pages are
    /// oversized: the bootstrap still stops at its bounds.
    #[tokio::test]
    async fn a_huge_follow_graph_is_read_within_bounds() {
        struct Endless(Mutex<usize>);

        #[async_trait::async_trait]
        impl FollowingSource for Endless {
            async fn following(
                &self,
                user_id: &str,
                skip: usize,
            ) -> Result<Option<Vec<String>>, DynError> {
                *self.0.lock().unwrap() += 1;
                let followees = (skip..skip + PAGE_LIMIT * 2).map(|i| format!("{user_id}/{i}"));
                Ok(Some(followees.collect()))
            }
        }

        let source = Endless(Mutex::new(0));
        let suggested = suggested_members("seed", 3, &source).await.unwrap();

        assert_eq!(suggested.len(), MAX_SUGGESTED);
        assert!(*source.0.lock().unwrap() <= MAX_REQUESTS);
    }
}
