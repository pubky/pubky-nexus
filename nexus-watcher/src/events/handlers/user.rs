use crate::events::{v0_to_v1, EventProcessorError};

use nexus_common::db::queries::get::user_is_safe_to_delete;
use nexus_common::db::{exec_single_row, execute_graph_operation, queries, OperationOutcome};
use nexus_common::models::user::{Epoch, UserDetails, UserSearch, UsersByTagSearch};
use pubky_social_specs::PubkyId;
use std::future::Future;
use std::path::Path;
use tracing::{debug, error, info};

use crate::events::inputs::UserInput;

#[tracing::instrument(name = "user.put", skip_all, fields(user_id = %user_id))]
pub async fn sync_put(user: UserInput, user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Indexing user profile");

    // Step 1: Create `UserDetails` object
    let uri = user.uri.clone();
    let mut user_details = user.into_details(&user_id);

    // Step 2: Save to graph, with the address the profile is stored under. The node keeps its
    // epoch.
    user_details.put_profile_to_graph(&uri).await?;

    // Step 3: Index the profile: drops the previous name member and clears the
    // tombstone. Retry-safe, the previous member comes from the search state
    // itself.
    UserSearch::index_profile(&user_details).await?;

    // The search write above cleared the tombstone, so re-derive the user's
    // per-label search entries afterwards: a recreated profile gets its
    // retained tags back.
    UsersByTagSearch::sync_user_labels(&user_id).await?;

    Ok(())
}

/// The flip step: moves a user to `v1` on their `social/v1` profile PUT, before the profile is
/// fetched, so the move does not wait for the homeserver.
///
/// What Nexus holds for the user at `pubky.app` is migrated first ([`v0_to_v1`]), their files
/// copied to their `social/v1` folders under `files_path` included, then the epoch is written
/// ([`move_to_v1`]). Every failure there, the epoch write's included, comes
/// out of [`v0_to_v1::step`] as [`EventProcessorError::FlipFailed`] naming the step and the
/// cause, leaves the user at `v0` and logs one summary line saying so: the retry runs the whole
/// step again over its partial work, without limit while the user is `v0`. The API reads the
/// user's details from the graph, so it reports `v1` once the epoch write lands, before the
/// profile fetch that follows. The profile itself is the ordinary `v1` profile PUT's ([`sync_put`]).
/// A user whose first event is this PUT has no node yet: the epoch write creates a stub, which
/// the profile write fills in.
pub async fn flip(user_id: &PubkyId, files_path: &Path) -> Result<(), EventProcessorError> {
    run_flip(user_id, move_to_v1(user_id, files_path)).await
}

/// The flip's log lines over `moved` (steps 1 and 2, [`move_to_v1`]). Taking it as a future
/// lets a test fail it without the stack.
#[tracing::instrument(name = "user.flip", skip_all, fields(user_id = %user_id))]
async fn run_flip(
    user_id: &PubkyId,
    moved: impl Future<Output = Result<(), EventProcessorError>>,
) -> Result<(), EventProcessorError> {
    // Until the epoch write lands the user is `v0`
    moved.await.inspect_err(|e| {
        error!(
            user_id = %user_id,
            error = %e,
            "Flip failed, the user stays at v0"
        )
    })?;

    // One line per flip, so flips can be counted and found by user.
    info!("User flipped to v1");

    Ok(())
}

/// The part of the flip that moves the user: the internal migration of their indexed data to
/// its `social/v1` form while they are still `v0` (step 1), then the epoch write (step 2).
async fn move_to_v1(user_id: &PubkyId, files_path: &Path) -> Result<(), EventProcessorError> {
    v0_to_v1::run(user_id, files_path).await?;
    v0_to_v1::step(user_id, "epoch", Epoch::set_v1_in_graph(user_id)).await
}

#[tracing::instrument(name = "user.del", skip_all, fields(user_id = %user_id))]
pub async fn del(user_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Deleting user profile");

    // 1. Graph query to check if there is any edge at all to this user.
    let query = user_is_safe_to_delete(&user_id);

    // 2. If there are no relationships (OperationOutcome::CreatedOrDeleted), delete from graph and redis.
    // 3. If there are relationships (OperationOutcome::Updated), overwrite the node with a cleared
    // profile carrying `deleted = true`. The node survives so its edges stay intact.
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => {
            // 1. Drop the user from search; idempotent, the member comes from the search state
            UserSearch::delete(&user_id).await?;

            // 2. Graph deletion LAST
            exec_single_row(queries::del::delete_user(&user_id)).await?;
        }
        OperationOutcome::Updated => {
            // 1. Graph-first: the tombstone keeps the stored `uri` and its epoch.
            let mut tombstone = UserDetails::tombstone(&user_id);
            tombstone.put_tombstone_to_graph().await?;

            // 2. Drop the user from search and mark them deleted, atomically: from
            // here on the users-by-tag sync evicts them, so a concurrent tag event
            // cannot re-add them. A retry finds the same state and changes nothing.
            UserSearch::mark_tombstoned(&user_id).await?;

            // 3. Evict the user from every per-label users-by-tag sorted set.
            UsersByTagSearch::sync_user_labels(&user_id).await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    }

    // TODO notifications for deleted user

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::graph::Query;
    use nexus_common::db::{exec_single_row, queries};
    use nexus_common::types::DynError;
    use nexus_common::utils::test_utils::random_pubky_id;
    use nexus_common::{StackConfig, StackManager};

    /// The flip step of a `v0` user with nothing to migrate: the user is `v1` in the graph, the
    /// profile untouched until the fetch that follows, and one `info` line names the user.
    #[tokio_shared_rt::test(shared)]
    #[tracing_test::traced_test]
    async fn a_flip_moves_the_user_to_v1_and_logs_one_info_line() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user = random_pubky_id();
        UserDetails::from_pubky(user.clone()).put_to_graph().await?;

        let files = tempfile::TempDir::new()?;
        flip(&user, files.path()).await?;

        assert_eq!(Epoch::get_from_graph(&user).await?, Epoch::V1);
        let details = UserDetails::get_by_id(user.as_ref())
            .await?
            .expect("the node stays");
        assert_eq!(details.name, user.to_string(), "no profile fetched here");
        logs_assert(|lines| {
            let flipped: Vec<_> = lines
                .iter()
                .filter(|line| line.contains("User flipped to v1"))
                .collect();
            match flipped.as_slice() {
                [line] if line.contains(" INFO ") && line.contains(user.as_ref()) => Ok(()),
                other => Err(format!(
                    "expected one INFO line naming the user, got {other:?}"
                )),
            }
        });

        exec_single_row(queries::del::delete_user(&user)).await?;
        Ok(())
    }

    /// A failed `moved` part (steps 1 and 2) leaves the user at `v0`: `run_flip` returns its
    /// error unchanged, logs exactly one summary line naming the user and no `info` line.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn a_failed_move_logs_one_summary_line() {
        let user = random_pubky_id();
        let moved = async {
            Err(EventProcessorError::FlipFailed {
                step: "posts",
                source: Box::new(EventProcessorError::GraphQueryFailed(
                    true,
                    "connection refused".into(),
                )),
            })
        };

        let err = run_flip(&user, moved)
            .await
            .expect_err("the move must fail");

        assert!(
            matches!(err, EventProcessorError::FlipFailed { step: "posts", .. }),
            "{err:?}"
        );
        assert!(!logs_contain("User flipped to v1"));
        logs_assert(|lines| {
            let summaries: Vec<_> = lines
                .iter()
                .filter(|line| line.contains("Flip failed, the user stays at v0"))
                .collect();
            match summaries.as_slice() {
                [line] if line.contains(" ERROR ") && line.contains(user.as_ref()) => Ok(()),
                other => Err(format!(
                    "expected one ERROR summary line naming the user, got {other:?}"
                )),
            }
        });
    }

    /// A flip whose posts step fails on a row it cannot read: `flip` returns the step's
    /// `FlipFailed`, the user stays `v0`, and exactly one summary line says so, naming the user.
    #[tokio_shared_rt::test(shared)]
    #[tracing_test::traced_test]
    async fn a_failed_migration_step_leaves_the_user_at_v0_and_logs_one_summary_line(
    ) -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user = random_pubky_id();
        UserDetails::from_pubky(user.clone()).put_to_graph().await?;
        // A post whose attachments are no list of addresses: the posts step cannot read it
        let post_id = "0032SSN7Q4EVG";
        let plant = Query::new(
            "plant_unreadable_post",
            "MATCH (u:User {id: $user_id})
            CREATE (u)-[:AUTHORED]->(:Post {id: $post_id, attachments: 42})",
        )
        .param("user_id", user.to_string())
        .param("post_id", post_id);
        exec_single_row(plant).await?;

        let files = tempfile::TempDir::new()?;
        let err = flip(&user, files.path())
            .await
            .expect_err("the posts step must fail");

        assert!(
            matches!(err, EventProcessorError::FlipFailed { step: "posts", .. }),
            "{err:?}"
        );
        assert_eq!(Epoch::get_from_graph(&user).await?, Epoch::V0);
        logs_assert(|lines| {
            let summaries: Vec<_> = lines
                .iter()
                .filter(|line| line.contains("Flip failed, the user stays at v0"))
                .collect();
            match summaries.as_slice() {
                [line] if line.contains(" ERROR ") && line.contains(user.as_ref()) => Ok(()),
                other => Err(format!(
                    "expected one ERROR summary line naming the user, got {other:?}"
                )),
            }
        });

        exec_single_row(queries::del::delete_post(user.as_ref(), post_id)).await?;
        exec_single_row(queries::del::delete_user(&user)).await?;
        Ok(())
    }
}
