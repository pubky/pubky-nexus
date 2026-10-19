use std::collections::{HashMap, HashSet};

use super::{Influencers, UserCounts, UserDetails, UserSearch, UserView};

use crate::db::graph::keyset_scan;
use crate::db::kv::{sets, RedisError, RedisResult, SortOrder};
use crate::db::{fetch_all_rows_from_graph, queries, RedisOps};
use crate::models::error::ModelError;
use crate::models::error::ModelResult;
use crate::models::follow::{Followers, Following, Friends, UserFollows};
use crate::models::post::{PostStream, POST_REPLIES_PER_POST_KEY_PARTS};
use crate::types::{StreamReach, Timeframe, WotDepth};

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, PoisonError};
use tracing::debug;
use utoipa::ToSchema;

pub const USER_MOSTFOLLOWED_KEY_PARTS: [&str; 2] = ["Users", "MostFollowed"];
/// Users per graph read when the most-followed ranking is rebuilt.
const MOST_FOLLOWED_PAGE_SIZE: usize = 500;
pub const USER_INFLUENCERS_KEY_PARTS: [&str; 2] = ["Users", "Influencers"];
pub const CACHE_USER_RECOMMENDED_KEY_PARTS: [&str; 3] = ["Cache", "Users", "Recommended"];
/// Keeps the per-label overfetch inside the graph query's own row cap, so a page is never
/// silently short. A pack is an onboarding list, so paging this far is already meaningless.
pub const STARTER_PACK_MAX_SKIP: usize = 100;
// TTL, 12HR
pub const CACHE_USER_RECOMMENDED_TTL: i64 = 12 * 60 * 60;

#[derive(Deserialize, ToSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserStreamSource {
    Followers,
    Following,
    Friends,
    MostFollowed,
    Influencers,
    Recommended,
    PostReplies,
    StarterPack,
}

pub struct UserStreamInput {
    pub user_id: Option<String>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub source: UserStreamSource,
    pub reach: Option<StreamReach>,
    pub timeframe: Option<Timeframe>,
    pub preview: Option<bool>,
    pub author_id: Option<String>,
    pub post_id: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Default, Clone)]
pub struct UserIdStream(pub Vec<String>);

impl UserIdStream {
    pub fn new(user_ids: Vec<String>) -> Self {
        Self(user_ids)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub struct UserStream(pub Vec<UserView>);

impl RedisOps for UserStream {}

impl UserStream {
    pub fn extend(&mut self, user_stream: UserStream) {
        self.0.extend(user_stream.0);
    }

    pub async fn get_by_id(
        input: UserStreamInput,
        viewer_id: Option<String>,
        depth: Option<u8>,
    ) -> ModelResult<Option<Self>> {
        let user_ids = Self::get_user_list_from_source(input).await?;
        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id.as_deref(), depth).await,
            None => Ok(None),
        }
    }

    pub async fn get_from_username_search(
        username: &str,
        viewer_id: Option<&str>,
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<Option<Self>> {
        let user_ids = UserSearch::get_by_name(username, skip, limit)
            .await?
            .map(|result| result.0);

        match user_ids {
            Some(users) => Self::from_listed_user_ids(&users, viewer_id, None).await,
            None => Ok(None),
        }
    }

    pub async fn from_listed_user_ids(
        user_ids: &[String],
        viewer_id: Option<&str>,
        depth: Option<u8>,
    ) -> ModelResult<Option<Self>> {
        // Use the new mget batch operation to retrieve all user views efficiently
        let user_views_result = UserView::get_by_ids(user_ids, viewer_id, depth).await?;

        let mut user_views = Vec::with_capacity(user_ids.len());

        for view in user_views_result.into_iter().flatten() {
            user_views.push(view);
        }

        match user_views.is_empty() {
            true => Ok(None),
            false => Ok(Some(Self(user_views))),
        }
    }

    /// Adds the post to a Redis sorted set using the follower counts as score.
    pub async fn add_to_influencers_sorted_set(
        user_id: &str,
        counts: &UserCounts,
    ) -> RedisResult<()> {
        let score = (counts.tagged + counts.posts) as f64 * (counts.followers as f64).sqrt();
        Self::put_index_sorted_set(&USER_INFLUENCERS_KEY_PARTS, &[(score, user_id)], None, None)
            .await
    }
    /// Retrieves recommended user IDs based on the specified criteria.
    pub async fn get_recommended_ids(
        user_id: &str,
        limit: Option<usize>,
    ) -> ModelResult<Option<Vec<String>>> {
        let count = limit.unwrap_or(5) as isize;

        // Attempt to get cached data from Redis
        if let Some(cached_ids) = Self::try_get_cached_recommended(user_id, count).await? {
            // Filter out deleted users from cached IDs
            let details_list = UserDetails::mget(&cached_ids).await?;

            // Collect both filtered IDs and deleted IDs in one pass
            let mut filtered_ids: Vec<String> = Vec::new();
            let mut deleted_ids: Vec<String> = Vec::new();

            for (id, details) in cached_ids.into_iter().zip(details_list) {
                match details {
                    Some(ref d) if !d.deleted => filtered_ids.push(id),
                    _ => deleted_ids.push(id),
                }
            }

            // Remove deleted users from the cache set to improve cache quality
            Self::remove_from_cached_recommended(user_id, &deleted_ids).await;

            return Ok(if filtered_ids.is_empty() {
                None
            } else {
                Some(filtered_ids)
            });
        }

        // Cache miss; proceed to query Neo4j
        let query = queries::get::recommend_users(user_id, 30);
        let rows = fetch_all_rows_from_graph(query).await?;

        let mut user_ids = Vec::new();

        for row in rows {
            // Deleted users are filtered in Cypher; only the user ID is projected.
            if let Some(uid) = row.get::<Option<String>>("recommended_user_id")? {
                user_ids.push(uid);
            }
        }

        if user_ids.is_empty() {
            Ok(None)
        } else {
            Self::cache_recommended_users(user_id, &user_ids).await?;
            if let Some(limit) = limit {
                user_ids.truncate(limit);
            };
            Ok(Some(user_ids))
        }
    }

    async fn try_get_cached_recommended(
        user_id: &str,
        count: isize,
    ) -> RedisResult<Option<Vec<String>>> {
        let key_parts = &[user_id];
        Self::try_get_random_from_index_set(
            key_parts,
            count,
            Some(CACHE_USER_RECOMMENDED_KEY_PARTS.join(":")),
        )
        .await
    }

    /// Helper method to cache recommended users in Redis with a TTL.
    async fn cache_recommended_users(user_id: &str, user_ids: &[String]) -> RedisResult<()> {
        let values: Vec<&str> = user_ids.iter().map(|s| s.as_str()).collect();
        // Cache the result in Redis with a TTL of 12 hours
        Self::put_index_set(
            &[user_id],
            &values,
            Some(CACHE_USER_RECOMMENDED_TTL),
            Some(CACHE_USER_RECOMMENDED_KEY_PARTS.join(":")),
        )
        .await
    }

    /// Helper method to remove deleted users from the cached recommendations.
    /// This improves cache quality by evicting stale entries instead of just filtering them at read time.
    async fn remove_from_cached_recommended(user_id: &str, deleted_ids: &[String]) {
        if deleted_ids.is_empty() {
            return;
        }

        let prefix = CACHE_USER_RECOMMENDED_KEY_PARTS.join(":");
        let deleted_refs: Vec<&str> = deleted_ids.iter().map(|s| s.as_str()).collect();

        let _ = sets::del(&prefix, user_id, &deleted_refs).await;
    }

    /// One deduplicated ranked list of people to follow for a set of interest tags.
    ///
    /// Interleaved in the caller's tag order, so a popular interest cannot crowd out a niche
    /// one. `user_id` is optional; the cold-start case this exists for has none.
    pub async fn get_starter_pack_ids(
        labels: &[String],
        user_id: Option<&str>,
        since: i64,
        skip: usize,
        limit: usize,
    ) -> ModelResult<Option<Vec<String>>> {
        // Overfetch per label: without slack, an overlapping label's own people are never
        // fetched. STARTER_PACK_MAX_SKIP keeps the result inside the query's own row cap.
        let per_label = skip
            .saturating_add(limit)
            .saturating_mul(labels.len().max(1));
        let query = queries::get::starter_pack_users(labels, user_id, since, per_label);

        let mut by_label: HashMap<String, Vec<String>> = HashMap::new();
        for row in fetch_all_rows_from_graph(query).await? {
            by_label.insert(row.get("label")?, row.get("candidates")?);
        }

        // Labels come back in arbitrary order and an empty one is absent, so never zip.
        // Taking each entry also collapses a repeated label.
        let ranked = labels
            .iter()
            .filter_map(|label| by_label.remove(label))
            .collect();

        let user_ids: Vec<String> = interleave_unique(ranked)
            .into_iter()
            .skip(skip)
            .take(limit)
            .collect();

        Ok((!user_ids.is_empty()).then_some(user_ids))
    }

    /// Rebuilds the most-followed ranking from the graph, so its members and scores
    /// converge on the graph's. Nothing else writes it and it has no TTL: the
    /// `most-followed-cache` job runs this on its schedule, a missing key falls back to it,
    /// and so does a full reindex.
    pub async fn refresh_most_followed() -> ModelResult<()> {
        let ranking = Self::read_most_followed_ranking(MOST_FOLLOWED_PAGE_SIZE).await;
        Self::write_most_followed_ranking(ranking, None).await
    }

    /// Scores every ranked user by follower count, `page_size` users per graph read.
    async fn read_most_followed_ranking(page_size: usize) -> ModelResult<Vec<(String, f64)>> {
        let ranking = Mutex::new(Vec::new());
        keyset_scan(page_size, "most_followed", |after_id| {
            let ranking = &ranking;
            async move {
                let query = queries::get::get_most_followed_users(&after_id, page_size);
                let mut page: Vec<(String, f64)> = Vec::with_capacity(page_size);
                for row in fetch_all_rows_from_graph(query).await? {
                    let followers: i64 = row.get("followers")?;
                    page.push((row.get("id")?, followers as f64));
                }
                let last_id = page.last().map(|(id, _)| id.clone());
                let count = page.len();
                ranking
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .extend(page);
                Ok::<_, ModelError>((count, last_id))
            }
        })
        .await?;
        Ok(ranking.into_inner().unwrap_or_else(PoisonError::into_inner))
    }

    /// Swaps a complete ranking in with one atomic write. A failed read writes nothing,
    /// so the previous ranking is served until a later refresh succeeds. An empty one
    /// deletes the key: with no TTL, keeping the old ranking would keep its members
    /// forever. Tests drive it directly with injected results, under their own `prefix`.
    async fn write_most_followed_ranking(
        ranking: ModelResult<Vec<(String, f64)>>,
        prefix: Option<&str>,
    ) -> ModelResult<()> {
        let ranking = ranking?;
        debug!(count = ranking.len(), "Replacing the most-followed ranking");
        let elements: Vec<(f64, &str)> = ranking
            .iter()
            .map(|(id, followers)| (*followers, id.as_str()))
            .collect();
        Self::replace_index_sorted_set(&USER_MOSTFOLLOWED_KEY_PARTS, &elements, prefix, None)
            .await?;
        Ok(())
    }

    /// Retrieves most-followed user IDs from the sorted set, filtering out deleted users
    /// and cleaning them from the cache.
    ///
    /// Only a missing key counts as a miss: the ranking is rebuilt from the graph, then read
    /// again. An existing key whose window is empty is served as an empty page.
    async fn get_most_followed(
        skip: Option<usize>,
        limit: Option<usize>,
    ) -> ModelResult<Option<Vec<String>>> {
        let read = || {
            Self::try_from_index_sorted_set(
                &USER_MOSTFOLLOWED_KEY_PARTS,
                None,
                None,
                skip,
                limit,
                SortOrder::Descending,
                None,
            )
        };
        let mut result = read().await?;
        if result.is_none() {
            Self::refresh_most_followed().await?;
            result = read().await?;
        }

        let Some(set) = result else {
            return Ok(None);
        };

        let ids: Vec<String> = set.iter().map(|(id, _)| id.clone()).collect();
        let details_list = UserDetails::mget(&ids).await?;

        let mut filtered_ids = Vec::new();
        let mut deleted_ids = Vec::new();

        for ((id, _score), details) in set.into_iter().zip(details_list) {
            match details {
                Some(ref d) if !d.deleted => filtered_ids.push(id),
                _ => deleted_ids.push(id),
            }
        }

        if !deleted_ids.is_empty() {
            let refs: Vec<&str> = deleted_ids.iter().map(|s| s.as_str()).collect();
            let _ =
                Self::remove_from_index_sorted_set(None, &USER_MOSTFOLLOWED_KEY_PARTS, &refs).await;
        }

        Ok(if filtered_ids.is_empty() {
            None
        } else {
            Some(filtered_ids)
        })
    }

    async fn get_post_replies_ids(
        post_id: Option<String>,
        author_id: Option<String>,
    ) -> RedisResult<Option<Vec<String>>> {
        let post_id = post_id
            .ok_or("Post ID should be provided for user streams with source 'post_replies'")
            .map_err(|e| RedisError::InvalidInput(e.to_string()))?;
        let author_id = author_id
            .ok_or("Author ID should be provided for user streams with source 'post_replies'")
            .map_err(|e| RedisError::InvalidInput(e.to_string()))?;
        let key_parts = [
            &POST_REPLIES_PER_POST_KEY_PARTS[..],
            &[author_id.as_str(), post_id.as_str()],
        ]
        .concat();
        let replies = PostStream::try_from_index_sorted_set(
            &key_parts,
            None,
            None,
            None,
            None,
            SortOrder::Descending,
            None,
        )
        .await?;

        // If there are replies, extract unique user IDs using a HashSet.
        let unique_user_ids: HashSet<String> = if let Some(replies) = replies {
            replies
                .into_iter()
                .filter_map(|reply| reply.0.split(':').next().map(|s| s.to_string()))
                .collect()
        } else {
            // If no replies are found, return None.
            return Ok(None);
        };

        // Convert the HashSet to a Vec. (Note: the ordering will be arbitrary.)
        Ok(Some(unique_user_ids.into_iter().collect()))
    }

    /// Get list of users based on the specified reach type
    pub async fn get_user_list_from_source(
        input: UserStreamInput,
    ) -> ModelResult<Option<Vec<String>>> {
        let UserStreamInput {
            user_id,
            skip,
            limit,
            source,
            reach,
            timeframe,
            preview,
            author_id,
            post_id,
            tags,
        } = input;
        let user_ids = match source {
            UserStreamSource::Followers => Followers::get_by_id(
                user_id
                    .ok_or("User ID should be provided for user streams with source 'followers'")
                    .map_err(ModelError::from_generic)?
                    .as_str(),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::Following => Following::get_by_id(
                user_id
                    .ok_or("User ID should be provided for user streams with source 'following'")
                    .map_err(ModelError::from_generic)?
                    .as_str(),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::Friends => Friends::get_by_id(
                user_id
                    .ok_or("User ID should be provided for user streams with source 'friends'")
                    .map_err(ModelError::from_generic)?
                    .as_str(),
                skip,
                limit,
            )
            .await?
            .map(|u| u.0),
            UserStreamSource::MostFollowed => Self::get_most_followed(skip, limit).await?,
            UserStreamSource::Influencers => Influencers::get_influencers(
                user_id.as_deref(),
                Some(reach.unwrap_or(StreamReach::Wot(WotDepth::default()))),
                skip.unwrap_or(0),
                limit.unwrap_or(10).min(100),
                timeframe.unwrap_or(Timeframe::AllTime),
                preview.unwrap_or(false),
            )
            .await?
            .map(|result| {
                result
                    .iter()
                    .map(|(influencer_id, _)| influencer_id.clone())
                    .collect()
            }),
            UserStreamSource::Recommended => {
                UserStream::get_recommended_ids(
                    user_id
                        .ok_or(
                            "User ID should be provided for user streams with source 'recommended'",
                        )
                        .map_err(ModelError::from_generic)?
                        .as_str(),
                    limit,
                )
                .await?
            }
            UserStreamSource::PostReplies => {
                UserStream::get_post_replies_ids(post_id, author_id).await?
            }
            UserStreamSource::StarterPack => {
                let labels = tags
                    .ok_or("Tags should be provided for user streams with source 'starter_pack'")
                    .map_err(ModelError::from_generic)?;
                // AllTime lands on 0, which keeps the liveness gate at "has ever posted".
                let (since, _) = timeframe.unwrap_or(Timeframe::AllTime).to_timestamp_range();
                UserStream::get_starter_pack_ids(
                    &labels,
                    user_id.as_deref(),
                    since,
                    skip.unwrap_or(0),
                    limit.unwrap_or(5),
                )
                .await?
            }
        };
        Ok(user_ids)
    }
}

/// Round-robin merge, so every label the caller picked lands near the front.
///
/// A label misses out only when all its fetched candidates were claimed by an earlier one.
fn interleave_unique(ranked: Vec<Vec<String>>) -> Vec<String> {
    // `find` doubles as the cursor, stepping over ids an earlier label claimed.
    let mut lists: Vec<_> = ranked.into_iter().map(Vec::into_iter).collect();
    let mut seen = HashSet::new();
    let mut merged = Vec::new();

    loop {
        let round_start = merged.len();
        for candidates in &mut lists {
            if let Some(id) = candidates.find(|id| seen.insert(id.clone())) {
                merged.push(id);
            }
        }
        if merged.len() == round_start {
            return merged;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        interleave_unique, UserStream, MOST_FOLLOWED_PAGE_SIZE, USER_MOSTFOLLOWED_KEY_PARTS,
    };
    use crate::db::kv::{RedisResult, SortOrder};
    use crate::db::RedisOps;
    use crate::models::error::ModelError;
    use crate::models::{
        traits::Collection,
        user::{UserCounts, UserDetails},
    };
    use crate::{types::DynError, StackConfig, StackManager};
    use pubky::Keypair;
    use pubky_app_specs::PubkyId;
    use std::collections::HashMap;

    fn lists(raw: &[&[&str]]) -> Vec<Vec<String>> {
        raw.iter()
            .map(|l| l.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn takes_one_per_label_per_round() {
        let merged = interleave_unique(lists(&[&["a1", "a2", "a3"], &["b1", "b2"]]));
        assert_eq!(merged, ["a1", "b1", "a2", "b2", "a3"]);
    }

    #[test]
    fn a_niche_label_is_not_crowded_out_by_a_popular_one() {
        let popular: Vec<&str> = vec!["p1", "p2", "p3", "p4", "p5"];
        let merged = interleave_unique(lists(&[&popular, &["niche"]]));
        assert_eq!(merged[1], "niche");
    }

    #[test]
    fn duplicates_keep_their_first_position_and_free_a_later_slot() {
        // The second label should fall through to its own next pick, not lose the round.
        let merged = interleave_unique(lists(&[&["dup", "a2"], &["dup", "b2"]]));
        assert_eq!(merged, ["dup", "b2", "a2"]);
    }

    #[test]
    fn exhausted_labels_stop_holding_slots() {
        let merged = interleave_unique(lists(&[&["a1"], &["b1", "b2", "b3"]]));
        assert_eq!(merged, ["a1", "b1", "b2", "b3"]);
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(interleave_unique(lists(&[])).is_empty());
        assert!(interleave_unique(lists(&[&[], &[]])).is_empty());
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_most_followed_ranking_keeps_the_previous_ranking_when_the_read_fails(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "MostFollowedTest:FailedRead";
        let previous = vec![("alice".to_string(), 10.0)];
        UserStream::write_most_followed_ranking(Ok(previous.clone()), Some(prefix)).await?;

        let failed_read = Err(ModelError::from_generic("graph unavailable"));
        let result = UserStream::write_most_followed_ranking(failed_read, Some(prefix)).await;

        assert!(result.is_err(), "a failed read must fail the refresh");
        assert_eq!(read_most_followed(Some(prefix)).await?, Some(previous));
        UserStream::write_most_followed_ranking(Ok(Vec::new()), Some(prefix)).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_most_followed_ranking_replaces_members_and_scores() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "MostFollowedTest:Replace";
        let previous = vec![("alice".to_string(), 10.0), ("bob".to_string(), 5.0)];
        UserStream::write_most_followed_ranking(Ok(previous), Some(prefix)).await?;

        let ranking = vec![("bob".to_string(), 1.0), ("carol".to_string(), 2.0)];
        UserStream::write_most_followed_ranking(Ok(ranking), Some(prefix)).await?;

        // Not additive: alice leaves, and bob's unfollows lower his score.
        assert_eq!(
            read_most_followed(Some(prefix)).await?,
            Some(vec![("carol".to_string(), 2.0), ("bob".to_string(), 1.0)])
        );
        UserStream::write_most_followed_ranking(Ok(Vec::new()), Some(prefix)).await?;
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn write_most_followed_ranking_deletes_the_key_on_an_empty_ranking(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let prefix = "MostFollowedTest:Empty";
        let previous = vec![("alice".to_string(), 10.0)];
        UserStream::write_most_followed_ranking(Ok(previous), Some(prefix)).await?;

        UserStream::write_most_followed_ranking(Ok(Vec::new()), Some(prefix)).await?;

        assert_eq!(
            read_most_followed(Some(prefix)).await?,
            None,
            "an empty ranking must not keep the previous members"
        );
        Ok(())
    }

    /// A stale live ranking converges on the graph at the next refresh, with no watcher
    /// update: a score above the user's follower count, as after an unfollow, drops to it,
    /// a missing user comes back, and members the graph does not rank (gone, tombstoned,
    /// stubs) leave. Rewrites the shared live set, so `.config/nextest.toml` runs it alone.
    #[tokio_shared_rt::test(shared)]
    async fn refresh_converges_a_stale_most_followed_ranking() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let graph_ranking = UserStream::read_most_followed_ranking(MOST_FOLLOWED_PAGE_SIZE).await?;
        let (lowered, _) = graph_ranking
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("the fixture ranks users");
        let (missing, _) = graph_ranking
            .iter()
            .find(|(id, _)| id != lowered)
            .expect("the fixture ranks two users");

        let gone = PubkyId::from(Keypair::random());
        let tombstone = PubkyId::from(Keypair::random());
        let stub = PubkyId::from(Keypair::random());
        let unranked: [(&str, &str); 3] = [
            (&*gone, "a user gone from the graph"),
            (&*tombstone, "a tombstoned user"),
            (&*stub, "a discovery stub"),
        ];
        let stale: Vec<(f64, &str)> = std::iter::once(lowered.as_str())
            .chain(unranked.iter().map(|(id, _)| *id))
            .map(|id| (1e9, id))
            .collect();
        UserStream::put_index_sorted_set(&USER_MOSTFOLLOWED_KEY_PARTS, &stale, None, None).await?;
        UserStream::remove_from_index_sorted_set(
            None,
            &USER_MOSTFOLLOWED_KEY_PARTS,
            &[missing.as_str()],
        )
        .await?;
        UserDetails::tombstone(&tombstone).put_to_graph().await?;
        UserDetails::from_pubky(stub.clone()).put_to_graph().await?;

        // Remove the nodes before asserting, so a failure cannot leave them in the graph.
        let refreshed = UserStream::refresh_most_followed().await;
        let live = read_most_followed(None).await;
        UserDetails::delete(&tombstone).await?;
        UserDetails::delete(&stub).await?;
        refreshed?;
        let live: HashMap<String, f64> = live?.unwrap_or_default().into_iter().collect();

        let counts = UserCounts::get_from_graph(lowered)
            .await?
            .expect("a ranked user has counts");
        assert_eq!(
            live.get(lowered),
            Some(&f64::from(counts.followers)),
            "a score above the follower count must drop to it"
        );
        assert!(
            live.contains_key(missing),
            "a user missing from the ranking must come back"
        );
        for (id, member) in unranked {
            assert!(!live.contains_key(id), "{member} must leave the ranking");
        }
        assert_eq!(
            live.len(),
            graph_ranking.len(),
            "every ranked user is in it"
        );
        Ok(())
    }

    /// The fixture fits in one page of `MOST_FOLLOWED_PAGE_SIZE`, so walk it in small pages
    /// to cover the cursor a larger graph needs.
    #[tokio_shared_rt::test(shared)]
    async fn read_most_followed_ranking_walks_every_page() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let page_size = 7;

        let whole = UserStream::read_most_followed_ranking(MOST_FOLLOWED_PAGE_SIZE).await?;
        let paged = UserStream::read_most_followed_ranking(page_size).await?;

        assert!(
            whole.len() > 2 * page_size,
            "the fixture must span several pages"
        );
        assert_eq!(paged, whole);
        Ok(())
    }

    /// The whole most-followed ranking under `prefix`, the live one for `None`.
    async fn read_most_followed(prefix: Option<&str>) -> RedisResult<Option<Vec<(String, f64)>>> {
        UserStream::try_from_index_sorted_set(
            &USER_MOSTFOLLOWED_KEY_PARTS,
            None,
            None,
            Some(0),
            Some(10_000),
            SortOrder::Descending,
            prefix,
        )
        .await
    }
}
