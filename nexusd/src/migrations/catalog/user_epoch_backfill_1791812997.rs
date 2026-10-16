use crate::migrations::manager::Migration;
use async_trait::async_trait;
use nexus_common::db::fetch_key_from_graph;
use nexus_common::db::graph::Query;
use nexus_common::types::DynError;
use tracing::info;

/// Mark every user indexed before epochs existed as `v0`, so they don't wait for their next
/// event to get one.
///
/// # What this does
/// Sets `epoch = 'v0'` on every `:User` without one, tombstones and stubs included, then
/// fails if any `:User` is still without one. It used to rewrite the cached `User:Details`
/// documents as well; user details are read from the graph now, and the documents are gone.
///
/// # Batching
/// Nothing indexes `:User(epoch)`, so the backfill scans once and lets the server commit per
/// batch via `IN TRANSACTIONS`, instead of re-scanning per batch.
///
/// # Idempotency
/// Only users without an epoch match, so a failed run is safe to repeat. `is_multi_staged()`
/// is `false`, so the manager marks this Done after one backfill; running it again needs the
/// id in `migrations_backfill_ready`.
///
/// # Deploy ordering
/// Run it after the binaries that give users an epoch are deployed, and before the flip ships,
/// with every watcher stopped: a pre-epoch watcher could create a user after the graph step,
/// and the verify only catches that if the user lands before the check.
pub struct UserEpochBackfill1791812997;

/// Fails when a `:User` is still without an epoch, so a partial graph step can't pass unnoticed.
async fn verify_every_user_has_epoch() -> Result<(), DynError> {
    let verify = Query::new(
        "user_epoch_backfill_verify",
        "MATCH (u:User)
         WHERE u.epoch IS NULL
         RETURN count(u) AS remaining",
    );

    let remaining: i64 = fetch_key_from_graph(verify, "remaining")
        .await?
        .unwrap_or_default();
    if remaining != 0 {
        return Err(format!(
            "UserEpochBackfill migration: backfill did not drain — {remaining} users still have \
             no epoch. Confirm every pre-epoch watcher is stopped, then re-run the migration."
        )
        .into());
    }
    Ok(())
}

#[async_trait]
impl Migration for UserEpochBackfill1791812997 {
    fn id(&self) -> &'static str {
        "UserEpochBackfill1791812997"
    }

    fn is_multi_staged(&self) -> bool {
        false
    }

    async fn dual_write(_data: Box<dyn std::any::Any + Send + 'static>) -> Result<(), DynError> {
        Ok(())
    }

    async fn backfill(&self) -> Result<(), DynError> {
        // MATCH stays outside the subquery so IN TRANSACTIONS batches the rows it feeds in;
        // the trailing count forces the batches to run.
        let query = Query::new(
            "user_epoch_backfill",
            "MATCH (u:User)
             WHERE u.epoch IS NULL
             CALL (u) {
                 SET u.epoch = 'v0'
             } IN TRANSACTIONS OF 10000 ROWS
             RETURN count(u) AS processed",
        );

        let processed: i64 = fetch_key_from_graph(query, "processed")
            .await?
            .unwrap_or_default();
        info!("UserEpochBackfill migration: {processed} users set to v0 in the graph");

        verify_every_user_has_epoch().await?;
        info!("UserEpochBackfill migration: complete — {processed} users set to v0 in the graph");

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
    use futures::FutureExt;
    use nexus_common::db::exec_single_row;
    use nexus_common::{StackConfig, StackManager};
    use std::panic::AssertUnwindSafe;

    const SEEDED_IDS: [&str; 2] = [
        "MigrationTest:UserEpochBackfill:Seeded1",
        "MigrationTest:UserEpochBackfill:Seeded2",
    ];
    /// A user left without an epoch for the verify to catch.
    const NO_EPOCH_ID: &str = "MigrationTest:UserEpochBackfill:NoEpoch";

    async fn delete_fixture(user_ids: &[&str]) -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "user_epoch_backfill_test_cleanup",
                "MATCH (u:User) WHERE u.id IN $ids DETACH DELETE u",
            )
            .param("ids", user_ids.to_vec()),
        )
        .await?;
        Ok(())
    }

    /// Creates `:User` nodes in the shape written before epochs existed.
    async fn seed_pre_epoch_users(user_ids: &[&str]) -> Result<(), DynError> {
        exec_single_row(
            Query::new(
                "user_epoch_backfill_test_seed",
                "UNWIND $ids AS id
                 CREATE (:User {id: id, name: 'migration test', indexed_at: 1})",
            )
            .param("ids", user_ids.to_vec()),
        )
        .await?;
        Ok(())
    }

    async fn graph_epoch(user_id: &str) -> Result<Option<String>, DynError> {
        let query = Query::new(
            "user_epoch_backfill_test_epoch",
            "MATCH (u:User {id: $user_id}) RETURN u.epoch AS epoch",
        )
        .param("user_id", user_id);
        Ok(fetch_key_from_graph::<Option<String>>(query, "epoch")
            .await?
            .flatten())
    }

    /// Runs the backfill twice and checks the seeded users after each run.
    async fn backfill_twice_and_check() -> Result<(), DynError> {
        for run in 1..=2 {
            UserEpochBackfill1791812997.backfill().await?;
            for user_id in SEEDED_IDS {
                assert_eq!(
                    graph_epoch(user_id).await?.as_deref(),
                    Some("v0"),
                    "run {run}: {user_id} graph epoch"
                );
            }
        }
        Ok(())
    }

    /// Every seeded user gets `v0`, and a second run changes nothing.
    ///
    /// The backfill also sets `v0` on the mock users without an epoch, which reads the same.
    #[tokio_shared_rt::test(shared)]
    async fn test_backfill_marks_every_user_v0() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture(&SEEDED_IDS).await?;
        seed_pre_epoch_users(&SEEDED_IDS).await?;

        let outcome = AssertUnwindSafe(backfill_twice_and_check())
            .catch_unwind()
            .await;

        delete_fixture(&SEEDED_IDS).await?;
        outcome.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    }

    /// A `:User` still without an epoch after the graph step fails the migration.
    #[tokio_shared_rt::test(shared)]
    async fn test_verify_fails_on_a_user_without_epoch() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        delete_fixture(&[NO_EPOCH_ID]).await?;

        seed_pre_epoch_users(&[NO_EPOCH_ID]).await?;
        let result = verify_every_user_has_epoch().await;

        delete_fixture(&[NO_EPOCH_ID]).await?;
        let err = result.expect_err("a user without an epoch must fail the verify");
        assert!(err.to_string().contains("still have no epoch"), "{err}");
        Ok(())
    }
}
