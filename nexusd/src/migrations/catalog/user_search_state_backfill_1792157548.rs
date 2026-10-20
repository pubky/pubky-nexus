use async_trait::async_trait;
use tracing::info;

use crate::migrations::manager::Migration;
use nexus_common::db::{get_redis_conn, kv::sorted_key};
use nexus_common::models::user::{UserSearch, USER_ID_KEY_PARTS, USER_NAME_KEY_PARTS};
use nexus_common::types::DynError;

/// Members read per `ZSCAN` page.
const SCAN_COUNT: usize = 1_000;

/// Builds the search state `UserSearch` keeps beside the user search indexes,
/// then drops the index members that state does not vouch for.
///
/// 1. `UserSearch::reindex` walks every `:User`: `User:Search:Names` gets
///    the name each live user is indexed under, `User:Search:Deleted` gets
///    the tombstones, users without a `uri` (stubs) leave both indexes, and
///    each tombstone is evicted from the users-by-tag index, where a tag event
///    between deploy and this run could have re-added it.
/// 2. `Sorted:Users:Name` and `Sorted:Users:ID` are scanned, and every
///    member the hash does not record is removed: names leaked by renames
///    and deletes before the hash existed, and users gone from the graph.
///
/// Run it after the binaries of this change are deployed, with every watcher
/// stopped: the walk writes graph state as of its read, so a profile event
/// landing during it could be overwritten with the older name. Safe to re-run.
pub struct UserSearchStateBackfill1792157548;

/// `(user id, name)` for a `name:id` member of the name index; a member
/// without a `:` belongs to nobody.
fn name_member_owner(member: &str) -> (&str, &str) {
    member
        .rsplit_once(':')
        .map(|(name, id)| (id, name))
        .unwrap_or(("", member))
}

/// Removes the members of the sorted set at `key_parts` whose user the names
/// hash does not record under that very name (`is_name_index`) or at all.
async fn drop_unrecorded_members(
    key_parts: &[&str],
    is_name_index: bool,
) -> Result<usize, DynError> {
    let key = sorted_key(key_parts);
    let mut conn = get_redis_conn().await?;
    let mut cursor: u64 = 0;
    let mut dropped = 0;
    loop {
        // ZSCAN replies member and score alternating
        let (next_cursor, entries): (u64, Vec<String>) = redis::cmd("ZSCAN")
            .arg(&key)
            .arg(cursor)
            .arg("COUNT")
            .arg(SCAN_COUNT)
            .query_async(&mut conn)
            .await?;
        let members: Vec<&str> = entries.iter().step_by(2).map(String::as_str).collect();
        let stale = unrecorded_members(&members, is_name_index).await?;
        if !stale.is_empty() {
            redis::cmd("ZREM")
                .arg(&key)
                .arg(&stale)
                .query_async::<()>(&mut conn)
                .await?;
            dropped += stale.len();
        }
        cursor = next_cursor;
        if cursor == 0 {
            return Ok(dropped);
        }
    }
}

async fn unrecorded_members<'a>(
    members: &[&'a str],
    is_name_index: bool,
) -> Result<Vec<&'a str>, DynError> {
    let owners: Vec<(&str, Option<&str>)> = members
        .iter()
        .map(|member| match is_name_index {
            true => {
                let (id, name) = name_member_owner(member);
                (id, Some(name))
            }
            false => (*member, None),
        })
        .collect();
    let ids: Vec<&str> = owners.iter().map(|(id, _)| *id).collect();
    let recorded = UserSearch::indexed_names(&ids).await?;

    Ok(members
        .iter()
        .zip(owners)
        .zip(recorded)
        .filter(|((_, (_, carried)), recorded)| match carried {
            Some(name) => recorded.as_deref() != Some(name),
            None => recorded.is_none(),
        })
        .map(|((member, _), _)| *member)
        .collect())
}

#[async_trait]
impl Migration for UserSearchStateBackfill1792157548 {
    fn id(&self) -> &'static str {
        "UserSearchStateBackfill1792157548"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        UserSearch::reindex().await?;
        info!("UserSearchStateBackfill migration: search state rebuilt from the graph");

        let names = drop_unrecorded_members(&USER_NAME_KEY_PARTS, true).await?;
        let ids = drop_unrecorded_members(&USER_ID_KEY_PARTS, false).await?;
        info!(
            "UserSearchStateBackfill migration: complete, dropped {names} unrecorded name \
             members and {ids} unrecorded id members"
        );
        Ok(())
    }

    async fn cutover(&self) -> Result<(), DynError> {
        Ok(())
    }

    async fn cleanup(&self) -> Result<(), DynError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::kv::ScoreAction;
    use nexus_common::db::{exec_single_row, graph::Query, RedisOps};
    use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
    use nexus_common::models::tag::user::TagUser;
    use nexus_common::models::user::{
        UserDetails, UsersByTagSearch, SEARCH_DELETED_KEY_PARTS, TAG_GLOBAL_USER_TAGGERS,
    };
    use nexus_common::utils::test_utils::random_pubky_id;
    use nexus_common::{StackConfig, StackManager};
    use pubky_app_specs::{PubkyAppUser, PubkyId};

    const LABEL: &str = "searchstatebackfill";

    fn live_user(id: &PubkyId, name: &str) -> UserDetails {
        UserDetails::from_homeserver(
            PubkyAppUser {
                bio: None,
                image: None,
                links: None,
                name: name.to_string(),
                status: None,
            },
            id,
        )
    }

    async fn name_member(member: &str) -> Result<bool, DynError> {
        Ok(
            UserSearch::check_sorted_set_member(None, &USER_NAME_KEY_PARTS, &[member])
                .await?
                .is_some(),
        )
    }

    async fn id_member(user_id: &str) -> Result<bool, DynError> {
        Ok(
            UserSearch::check_sorted_set_member(None, &USER_ID_KEY_PARTS, &[user_id])
                .await?
                .is_some(),
        )
    }

    async fn delete_fixture(ids: &[&str], members: &[&str]) -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "user_search_state_backfill_test_cleanup",
                "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
            )
            .param("ids", ids.to_vec()),
        )
        .await?;
        for id in ids {
            UserSearch::delete(id).await?;
            TagUser(vec![]).del_from_index(id, None, LABEL).await.ok();
        }
        UserSearch::remove_from_index_sorted_set(None, &USER_NAME_KEY_PARTS, members).await?;
        UserSearch::remove_from_index_sorted_set(None, &USER_ID_KEY_PARTS, ids).await?;
        Ok(())
    }

    /// A live user keeps only their current name; a leaked previous name, a
    /// user gone from the graph and a stub indexed under its id are dropped;
    /// a tagged tombstone joins the deleted set and leaves the users-by-tag
    /// index. A second run changes nothing.
    #[tokio_shared_rt::test(shared)]
    async fn test_backfill_rebuilds_state_and_drops_stale_members() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let live_id = random_pubky_id();
        let tombstone_id = random_pubky_id();
        let stub_id = random_pubky_id();
        let gone_id = random_pubky_id().to_string();
        let tagger_id = random_pubky_id().to_string();
        let live = live_id.to_string();
        let tombstone = tombstone_id.to_string();
        let stub = stub_id.to_string();
        let current = format!("backfill current:{live}");
        let previous = format!("backfill previous:{live}");
        let gone_member = format!("backfill gone:{gone_id}");
        let stub_member = format!("{stub}:{stub}");
        let ids = [
            live.as_str(),
            tombstone.as_str(),
            stub.as_str(),
            gone_id.as_str(),
        ];
        let members = [
            current.as_str(),
            previous.as_str(),
            gone_member.as_str(),
            stub_member.as_str(),
        ];
        delete_fixture(&ids, &members).await?;

        live_user(&live_id, "Backfill Current")
            .put_profile_to_graph(&format!("pubky://{live}/pub/pubky.app/profile.json"))
            .await?;
        UserDetails::tombstone(&tombstone_id).put_to_graph().await?;
        UserDetails::from_pubky(stub_id).put_to_graph().await?;
        // Legacy index state: no hash, a leaked previous name, a user without
        // a node, a stub indexed under its id by a cache-miss read
        UserSearch::put_index_sorted_set(
            &USER_NAME_KEY_PARTS,
            &[
                (0.0, current.as_str()),
                (0.0, previous.as_str()),
                (0.0, gone_member.as_str()),
                (0.0, stub_member.as_str()),
            ],
            None,
            None,
        )
        .await?;
        UserSearch::put_index_sorted_set(
            &USER_ID_KEY_PARTS,
            &[
                (0.0, live.as_str()),
                (0.0, gone_id.as_str()),
                (0.0, stub.as_str()),
            ],
            None,
            None,
        )
        .await?;
        // The tombstone was tagged, and a tag event re-added it before the deleted set existed
        TagUser::add_tagger_to_index(&tombstone, None, &tagger_id, LABEL).await?;
        TagUser::update_index_score(&tombstone, None, LABEL, ScoreAction::Increment(1.0)).await?;
        UsersByTagSearch::sync_index_score(&tombstone, LABEL).await?;

        let outcome = async {
            for run in 1..=2 {
                UserSearchStateBackfill1792157548.backfill().await?;

                assert_eq!(
                    UserSearch::indexed_names(&[&live, &tombstone, &stub, &gone_id]).await?,
                    vec![Some("backfill current".to_string()), None, None, None],
                    "run {run}: the hash records the live user's current name only"
                );
                assert!(name_member(&current).await?, "run {run}: current name kept");
                assert!(
                    !name_member(&previous).await?,
                    "run {run}: leaked name dropped"
                );
                assert!(
                    !name_member(&gone_member).await?,
                    "run {run}: gone user dropped"
                );
                assert!(
                    !name_member(&stub_member).await?,
                    "run {run}: stub name dropped"
                );
                assert!(id_member(&live).await?, "run {run}: live id kept");
                assert!(!id_member(&gone_id).await?, "run {run}: gone id dropped");
                assert!(!id_member(&stub).await?, "run {run}: stub id dropped");
                assert!(
                    UserSearch::check_set_member(&SEARCH_DELETED_KEY_PARTS, &tombstone)
                        .await?
                        .1,
                    "run {run}: the tombstone is in the deleted set"
                );
                let score = UsersByTagSearch::check_sorted_set_member(
                    None,
                    &[&TAG_GLOBAL_USER_TAGGERS[..], &[LABEL]].concat(),
                    &[&tombstone],
                )
                .await?;
                assert!(
                    score.is_none(),
                    "run {run}: the tombstone left users-by-tag"
                );
            }
            Ok::<(), DynError>(())
        }
        .await;

        delete_fixture(&ids, &members).await?;
        outcome
    }

    #[test]
    fn name_member_owner_splits_at_the_last_colon() {
        assert_eq!(name_member_owner("a:b:id"), ("id", "a:b"));
        assert_eq!(name_member_owner("noid"), ("", "noid"));
    }
}
