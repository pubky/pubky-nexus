use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::db::kv::RedisResult;
use crate::db::{get_neo4j_graph, queries, GraphError, RedisOps};
use crate::models::error::ModelResult;

/// Redis key parts for the trust ranking projection: `Sorted:Users:SocialGraph`.
pub const USER_SOCIAL_GRAPH_KEY_PARTS: [&str; 2] = ["Users", "SocialGraph"];

/// Share of the ranked population marked `established`. A placeholder until the
/// real distribution is known; the ranking stores positions rather than tiers so
/// that changing this needs no recompute.
const ESTABLISHED_FRACTION: f64 = 0.05;

/// How established an account is in the follow graph, derived from the seeded
/// PageRank trust ranking.
///
/// Describes position in the graph, not character: it says an account is
/// expensive to fake, never that it is trustworthy.
#[derive(Serialize, Deserialize, ToSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SocialGraphStatus {
    /// No path from the seed set: a brand new account, or a follow farm that
    /// only follows itself.
    New,
    /// Reachable from the seed set.
    Networked,
    /// Top slice of the ranking.
    Established,
}

impl RedisOps for SocialGraphStatus {}

impl SocialGraphStatus {
    /// Reads the status of several users.
    ///
    /// Returns exactly one slot per requested id on every path, including when
    /// the ranking is missing: callers zip the result positionally against their
    /// id list, and a short vec would silently truncate it.
    pub async fn get_by_ids<T: AsRef<str>>(
        user_ids: &[T],
    ) -> RedisResult<Vec<Option<SocialGraphStatus>>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let members: Vec<&str> = user_ids.iter().map(|id| id.as_ref()).collect();

        // The population sizes the `established` cut, so it is read alongside the
        // ranks rather than baked into them: tuning the cut needs no rebuild. Both
        // come from one snapshot, or a rebuild landing mid-read could size the cut
        // from one ranking and place users by another.
        let (population, ranks) =
            Self::index_sorted_set_card_and_members(&USER_SOCIAL_GRAPH_KEY_PARTS, &members, None)
                .await?;

        debug_assert_eq!(ranks.len(), user_ids.len());
        Ok(Self::classify(population, &ranks))
    }

    /// Reads one user's status.
    pub async fn get_by_id(user_id: &str) -> RedisResult<Option<SocialGraphStatus>> {
        Ok(Self::get_by_ids(&[user_id])
            .await?
            .into_iter()
            .next()
            .flatten())
    }

    /// Maps raw ranks to statuses.
    ///
    /// An empty ranking means it was never built, which is not the same as
    /// ranking everyone as new, so every slot is `None` and the badge hides.
    fn classify(population: usize, ranks: &[Option<isize>]) -> Vec<Option<SocialGraphStatus>> {
        if population == 0 {
            return vec![None; ranks.len()];
        }
        let established_cut = (population as f64 * ESTABLISHED_FRACTION).ceil() as isize;

        ranks
            .iter()
            .map(|rank| {
                Some(match rank {
                    // Unreachable from the seed set, or created since the last
                    // rebuild. Both are new.
                    None => SocialGraphStatus::New,
                    Some(rank) if *rank <= established_cut => SocialGraphStatus::Established,
                    Some(_) => SocialGraphStatus::Networked,
                })
            })
            .collect()
    }

    /// Rebuilds the ranking from the trust scores in the graph. Run by the trust
    /// recompute job and by a full reindex.
    ///
    /// # Errors
    /// Returns an error when the graph read or the Redis write fails.
    pub async fn reindex() -> ModelResult<()> {
        let graph = get_neo4j_graph()?;
        let mut rows = graph
            .execute(queries::get::get_trust_ranked_user_ids())
            .await
            .map_err(GraphError::from)?;

        // Streamed rather than collected as rows: only the ids are needed.
        let mut ranked: Vec<String> = Vec::new();
        while let Some(row) = rows.try_next().await.map_err(GraphError::from)? {
            ranked.push(row.get("user_id")?);
        }

        // Empty drops the key, so readers fall back to "unavailable" rather
        // than "everyone is new".
        Self::replace_index_sorted_set(
            &USER_SOCIAL_GRAPH_KEY_PARTS,
            &Self::rank_elements(&ranked),
            None,
            None,
        )
        .await?;

        Ok(())
    }

    /// Turns the ordered ids into the sorted-set payload.
    ///
    /// Rank comes from position, never from comparing scores against a cut
    /// value: seeded PageRank hands identical scores to every spoke of a hub, so
    /// a `score >= cut` comparison would promote a whole plateau at once.
    fn rank_elements(ranked: &[String]) -> Vec<(f64, &str)> {
        ranked
            .iter()
            .enumerate()
            .map(|(index, user_id)| ((index + 1) as f64, user_id.as_str()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the write and read halves together, so the tests exercise the rank
    /// encoding end to end without Redis.
    fn statuses_for(population: usize) -> Vec<SocialGraphStatus> {
        let ids: Vec<String> = (0..population).map(|i| format!("user{i}")).collect();
        let ranks: Vec<Option<isize>> = SocialGraphStatus::rank_elements(&ids)
            .iter()
            .map(|(rank, _)| Some(*rank as isize))
            .collect();

        SocialGraphStatus::classify(population, &ranks)
            .into_iter()
            .map(|status| status.expect("a built ranking resolves every slot"))
            .collect()
    }

    #[test]
    fn ranks_are_positional() {
        let ids: Vec<String> = (0..3).map(|i| format!("user{i}")).collect();

        let elements = SocialGraphStatus::rank_elements(&ids);

        assert_eq!(
            elements,
            vec![(1.0, "user0"), (2.0, "user1"), (3.0, "user2")]
        );
    }

    #[test]
    fn the_cut_is_inclusive_at_its_boundary() {
        let statuses = statuses_for(100);

        // ceil(100 * 0.05) == 5, so rank 5 is in and rank 6 is out.
        assert_eq!(statuses[4], SocialGraphStatus::Established);
        assert_eq!(statuses[5], SocialGraphStatus::Networked);
    }

    // The cut rounds up, so a ranking always has an established slice.
    #[test]
    fn a_lone_ranked_user_is_established() {
        assert_eq!(statuses_for(1), vec![SocialGraphStatus::Established]);
    }

    #[test]
    fn an_unbuilt_ranking_hides_the_badge() {
        let statuses = SocialGraphStatus::classify(0, &[Some(1), None, Some(9)]);

        assert_eq!(statuses, vec![None, None, None]);
    }

    // Absent from a built ranking means unreachable from the seeds, or created
    // after the last rebuild. Both are new.
    #[test]
    fn an_absent_user_in_a_live_ranking_is_new() {
        let statuses = SocialGraphStatus::classify(100, &[None]);

        assert_eq!(statuses, vec![Some(SocialGraphStatus::New)]);
    }
}
