use crate::db::{fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphError};
use crate::models::error::ModelResult;
use crate::types::StreamReach;
use pubky_app_specs::PubkyId;
use tokio::time::{timeout, Duration};
use tracing::warn;

const REACH_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Authors resolved from a reach by [`reach_authors`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReachAuthors {
    /// At most `limit` authors, the most prolific first.
    pub author_ids: Vec<PubkyId>,
    /// The reach held more than `limit` authors, so `author_ids` is a subset.
    pub met_limit: bool,
}

/// Resolves up to `limit` users in `observer_id`'s `reach` who authored at
/// least one post, never including the observer. Users without posts are left
/// out: they cannot match a post search, so neither `limit` nor `met_limit`
/// counts them. A larger reach is trimmed to the authors with the most posts,
/// so a scoped search keeps the ones most likely to match, and the result is
/// flagged with `met_limit`. An unknown observer has an empty reach.
///
/// # Errors
/// Returns an error when the graph read fails, including
/// `GraphError::QueryTimeout` for a traversal over its budget.
pub async fn reach_authors(
    observer_id: &str,
    reach: &StreamReach,
    limit: usize,
) -> ModelResult<ReachAuthors> {
    // One extra row tells a reach of exactly `limit` authors from a larger one
    let query =
        queries::get::get_reach_authors_by_posts(observer_id, reach, limit.saturating_add(1));
    let rows = timeout(REACH_QUERY_TIMEOUT, fetch_all_rows_from_graph(query))
        .await
        .map_err(|_| GraphError::QueryTimeout)??;
    let ids = rows
        .iter()
        .map(|row| row.get::<String>("author_id"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(GraphError::from)?;
    Ok(trim_to_limit(ids, limit))
}

/// Keeps the first `limit` of `ids`, flagging with `met_limit` that there were
/// more. Graph ids come from validated events; one that doesn't parse is
/// dropped rather than failing the whole search.
fn trim_to_limit(ids: Vec<String>, limit: usize) -> ReachAuthors {
    let met_limit = ids.len() > limit;
    let author_ids: Vec<PubkyId> = ids
        .into_iter()
        .take(limit)
        .filter_map(|id| match PubkyId::try_from(&id) {
            Ok(id) => Some(id),
            Err(e) => {
                warn!("Skipping invalid user id {id} in reach: {e}");
                None
            }
        })
        .collect();
    ReachAuthors {
        author_ids,
        met_limit,
    }
}

/// Whether `user_id` is in `observer_id`'s `reach`, asked of the graph like
/// [`reach_authors`], so the two cannot disagree on membership; a user in the
/// reach who never posted is reached here but absent from [`reach_authors`].
/// The observer is never in their own reach, and unknown users are in nobody's.
///
/// # Errors
/// Returns an error when the graph read fails, including
/// `GraphError::QueryTimeout`.
pub async fn reach_contains(
    observer_id: &str,
    reach: &StreamReach,
    user_id: &str,
) -> ModelResult<bool> {
    if user_id == observer_id {
        return Ok(false);
    }
    let query = queries::get::reach_contains_user(observer_id, user_id, reach);
    let reached = timeout(REACH_QUERY_TIMEOUT, fetch_key_from_graph(query, "reached"))
        .await
        .map_err(|_| GraphError::QueryTimeout)??;
    Ok(reached.unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::WotDepth;
    use crate::{types::DynError, StackConfig, StackManager};

    // docker/test-graph/mocks/search-reach.cypher
    const OBS: &str = "wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy";
    const FRIEND: &str = "x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay";
    const FOLLOWED: &str = "xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy";
    const FOLLOWER: &str = "xu1n8qam7zjwpg4qtormzjezszs6k9m9hqdp9gsktkzw5dboijcy";
    const D2: &str = "xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo";
    const STRANGER: &str = "y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo";
    /// In every reach of OBS, with no post to its name.
    const LURKER: &str = "zy8gjbx3xoi4j7cgudcajwxg3y6ybc7if8zwiznn4mfy84t5yjco";
    const UNKNOWN: &str = "w8phaw75htdp4pkchuicp76yn1ycwhaixaeqw6zhfubg641ogn8y";

    fn as_strs(ids: &[PubkyId]) -> Vec<&str> {
        ids.iter().map(AsRef::as_ref).collect()
    }

    fn wot(depth: u8) -> StreamReach {
        StreamReach::Wot(WotDepth::new(depth).expect("valid depth"))
    }

    fn owned(ids: &[&str]) -> Vec<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn trim_to_limit_keeps_the_first_ids_of_a_larger_reach() {
        let authors = trim_to_limit(owned(&[FRIEND, D2, FOLLOWED]), 2);
        assert_eq!(as_strs(&authors.author_ids), vec![FRIEND, D2]);
        assert!(authors.met_limit);
    }

    #[test]
    fn trim_to_limit_does_not_flag_a_reach_of_exactly_limit() {
        let authors = trim_to_limit(owned(&[FRIEND, D2]), 2);
        assert_eq!(as_strs(&authors.author_ids), vec![FRIEND, D2]);
        assert!(!authors.met_limit);
    }

    #[test]
    fn trim_to_limit_of_zero_is_empty_and_flagged() {
        let authors = trim_to_limit(owned(&[FRIEND]), 0);
        assert!(authors.author_ids.is_empty());
        assert!(authors.met_limit);
    }

    #[test]
    fn trim_to_limit_drops_an_id_that_does_not_parse() {
        let invalid = "not-a-pubky-id";
        assert!(PubkyId::try_from(invalid).is_err());

        let authors = trim_to_limit(owned(&[FRIEND, invalid, D2]), 3);
        assert_eq!(as_strs(&authors.author_ids), vec![FRIEND, D2]);
        assert!(!authors.met_limit);
    }

    #[tokio_shared_rt::test(shared)]
    async fn reach_authors_resolves_every_reach_without_the_observer() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // FRIEND authored a post and a reply, FOLLOWED and D2 one post each;
        // equal counts break ties by id descending. LURKER is in every reach of
        // OBS but authored nothing, so it is in none of these lists
        let cases = [
            (StreamReach::Following, vec![FRIEND, FOLLOWED]),
            (StreamReach::Followers, vec![FRIEND, FOLLOWER]),
            (StreamReach::Friends, vec![FRIEND]),
            (wot(1), vec![FRIEND, FOLLOWED]),
            // OBS is reachable through FRIEND's follow back and stays out
            (wot(2), vec![FRIEND, D2, FOLLOWED]),
            (wot(3), vec![FRIEND, D2, FOLLOWED]),
        ];
        for (reach, expected) in cases {
            let authors = reach_authors(OBS, &reach, 1_000).await?;
            assert_eq!(as_strs(&authors.author_ids), expected, "{reach:?}");
            assert!(!authors.met_limit, "{reach:?}");
        }
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn reach_authors_skips_users_without_posts() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // An id that does not parse would be dropped as invalid, which would
        // pass the assertions below for the wrong reason
        assert!(
            PubkyId::try_from(LURKER).is_ok(),
            "the fixture id must be a valid Pubky id"
        );

        for reach in [
            StreamReach::Following,
            StreamReach::Followers,
            StreamReach::Friends,
            wot(1),
            wot(3),
        ] {
            let authors = reach_authors(OBS, &reach, 1_000).await?;
            assert!(
                !as_strs(&authors.author_ids).contains(&LURKER),
                "{reach:?} must not list a user without posts"
            );
            // It is in the reach; it just has nothing a post search can find
            assert!(reach_contains(OBS, &reach, LURKER).await?, "{reach:?}");
        }

        // FRIEND and FOLLOWED are the only authors OBS follows, so a limit of
        // two is not met even though the reach holds LURKER too
        let authors = reach_authors(OBS, &StreamReach::Following, 2).await?;
        assert_eq!(as_strs(&authors.author_ids), vec![FRIEND, FOLLOWED]);
        assert!(
            !authors.met_limit,
            "users without posts must not flag a reach as trimmed"
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn reach_authors_trims_to_the_most_prolific_authors() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        // The wot_2 reach holds exactly three authors
        let trimmed = |ids: &[&str]| ReachAuthors {
            author_ids: ids
                .iter()
                .map(|id| PubkyId::try_from(id).expect("valid Pubky id"))
                .collect(),
            met_limit: true,
        };
        assert_eq!(
            reach_authors(OBS, &wot(2), 2).await?,
            trimmed(&[FRIEND, D2])
        );
        assert_eq!(reach_authors(OBS, &wot(2), 1).await?, trimmed(&[FRIEND]));
        assert_eq!(reach_authors(OBS, &wot(2), 0).await?, trimmed(&[]));

        let exact = reach_authors(OBS, &wot(2), 3).await?;
        assert_eq!(as_strs(&exact.author_ids), vec![FRIEND, D2, FOLLOWED]);
        assert!(
            !exact.met_limit,
            "a reach of exactly `limit` authors is complete"
        );
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn reach_authors_is_empty_for_unknown_or_isolated_observers() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        assert_eq!(
            reach_authors(UNKNOWN, &wot(3), 1_000).await?,
            ReachAuthors::default()
        );
        // STRANGER follows nobody and has no followers
        for reach in [StreamReach::Following, StreamReach::Followers, wot(3)] {
            assert_eq!(
                reach_authors(STRANGER, &reach, 1_000).await?,
                ReachAuthors::default()
            );
        }
        Ok(())
    }

    #[tokio_shared_rt::test(shared)]
    async fn reach_contains_matches_reach_authors_for_users_with_posts() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;

        for reach in [
            StreamReach::Following,
            StreamReach::Followers,
            StreamReach::Friends,
            wot(1),
            wot(2),
        ] {
            let ids = reach_authors(OBS, &reach, 1_000).await?.author_ids;
            // LURKER is left out: it is in the reach without being an author
            for user in [OBS, FRIEND, FOLLOWED, FOLLOWER, D2, STRANGER, UNKNOWN] {
                assert_eq!(
                    reach_contains(OBS, &reach, user).await?,
                    ids.iter().any(|id| id.as_ref() == user),
                    "{reach:?} {user}"
                );
            }
        }
        Ok(())
    }
}
