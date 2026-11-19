use async_trait::async_trait;
use tracing::{info, warn};

use crate::migrations::manager::Migration;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisError;
use nexus_common::db::{fetch_all_rows_from_graph, RedisOps};
use nexus_common::models::user::UserDetails;
use nexus_common::types::DynError;

/// Restores the profiles key discovery overwrote with a stub.
///
/// Key discovery used to write its stub over whatever node it found after its homeserver
/// lookup. A profile indexed during that lookup lost its name, bio, status, links and image
/// in the graph and kept only its `uri`. The `User:Details` document of such a user was not
/// touched, so it still holds the profile.
///
/// # What this does
/// 1. Graph: reads every live `:User` with a `uri` whose name is its own id. No profile is
///    named so: a name is shorter than a key.
/// 2. Redis: reads the `User:Details` document of each. One that holds a profile, a name
///    other than the id and no `deleted` flag, is written back to the node: name, bio,
///    status, links, image and `indexed_at`. The `uri` and the epoch stay.
///
/// A user without such a document is logged with their id and left as they are: their next
/// profile event restores them.
///
/// # Deploy ordering
/// It reads the documents `UserDetailsCacheRetire1792160000` deletes, and
/// `UserSearchStateBackfill1792157548` indexes the names it restores, so it runs before
/// both. Run it with every watcher stopped, like the backfill. Safe to re-run: a restored
/// user is no longer matched.
pub struct OverwrittenProfileRestore1795036051;

/// The live users named after their key who have a profile address.
async fn overwritten_user_ids() -> Result<Vec<String>, DynError> {
    let query = Query::new(
        "overwritten_profile_candidates",
        "MATCH (u:User)
         WHERE u.uri IS NOT NULL AND u.name = u.id AND NOT coalesce(u.deleted, false)
         RETURN u.id AS id
         ORDER BY id",
    );
    let mut ids = Vec::new();
    for row in fetch_all_rows_from_graph(query).await? {
        ids.push(row.get("id")?);
    }
    Ok(ids)
}

/// Whether `cached` holds a profile to restore `user_id` from: a stub's or a tombstone's
/// document does not.
fn holds_profile(user_id: &str, cached: &UserDetails) -> bool {
    !cached.deleted && cached.name != user_id
}

#[async_trait]
impl Migration for OverwrittenProfileRestore1795036051 {
    fn id(&self) -> &'static str {
        "OverwrittenProfileRestore1795036051"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        let user_ids = overwritten_user_ids().await?;
        let mut restored = 0;
        let mut left = 0;

        for user_id in &user_ids {
            let cached = match UserDetails::try_from_index_json(&[user_id], None).await {
                // A document this build cannot read is no profile to restore from
                Err(RedisError::DeserializationFailed(e)) => {
                    warn!(user_id, error = %e, "OverwrittenProfileRestore migration: unreadable cached profile");
                    None
                }
                cached => cached?,
            };
            match cached.filter(|cached| holds_profile(user_id, cached)) {
                // Writes every profile field and keeps the node's `uri` and epoch
                Some(profile) => {
                    profile.put_to_graph().await?;
                    restored += 1;
                }
                None => {
                    left += 1;
                    warn!(
                        user_id,
                        "OverwrittenProfileRestore migration: no cached profile to restore from"
                    );
                }
            }
        }

        info!(
            "OverwrittenProfileRestore migration: complete, {} users named after their key, \
             {restored} restored, {left} left as they are",
            user_ids.len()
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
    use super::holds_profile;
    use nexus_common::models::user::UserDetails;
    use pubky_social_specs::PubkyId;

    /// Only a document with a name of its own and no `deleted` flag is a profile.
    #[test]
    fn holds_profile_refuses_a_stub_and_a_tombstone() {
        let user_id =
            PubkyId::try_from("8um71us3fyw6h8wbcxb5ar3rwusy1a6u49956ikzojg3gcwd1dty").unwrap();
        let stub = UserDetails::from_pubky(user_id.clone());
        let profile = UserDetails {
            name: "Alice".to_string(),
            ..stub.clone()
        };
        let tombstone = UserDetails::tombstone(&user_id);

        assert!(holds_profile(&user_id, &profile));
        assert!(!holds_profile(&user_id, &stub), "named after the key");
        assert!(!holds_profile(&user_id, &tombstone), "deleted");
    }
}
