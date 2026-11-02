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
///    the tombstones, discovery stubs (no `uri`, named after their key) leave
///    both indexes, and each tombstone is evicted from the users-by-tag index,
///    where a tag event between deploy and this run could have re-added it.
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
