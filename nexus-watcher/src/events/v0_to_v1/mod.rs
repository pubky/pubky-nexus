//! The internal migration from `pubky.app` (`v0`) to `social/v1`: at the flip, what Nexus
//! already holds for a user under `pubky.app` is rewritten to its `social/v1` form, derived from
//! the graph, before the user's epoch moves to `v1`. Not a `nexusd` migration, and not the
//! app's migrator, which copies the user's files; this one rewrites the index.
//!
//! Each step pages through the user's data in batches, each batch its own transaction, and a
//! rerun over partly migrated data gives the same result. A failed step is logged with the user
//! and the step and returned as [`EventProcessorError::FlipFailed`] naming both, whatever the
//! cause: a graph or Redis error, or one of the migration's own guards (a tags page whose write
//! changed nothing, a stalled keyset scan). The flip goes through the normal retry, which runs
//! it whole again while the user is still `v0`; a user whose data cannot be migrated is never
//! reported as `v1`. The epoch write and the details cache refresh that end the flip
//! ([`crate::events::handlers::user::flip`]) go through the same [`step`].

pub mod attachments;
pub mod files;
#[cfg(test)]
mod migrator_tests;
pub mod posts;
pub mod resource_changes;
pub mod tags;

use crate::events::EventProcessorError;
use files::FileMap;
use pubky_social_specs::PubkyId;
use std::future::Future;
use tracing::error;

/// Runs every step of `user_id`'s migration, in order: the file map first, then the posts
/// whose attachments it resolves, then the tags whose file targets it resolves. The resource
/// changes feed lines of the tags it moves are published after the flip's epoch write, and
/// never fail the flip: see [`resource_changes`].
#[tracing::instrument(name = "user.migrate", skip_all, fields(user_id = %user_id))]
pub async fn run(user_id: &PubkyId) -> Result<(), EventProcessorError> {
    let files = step(user_id, "files", FileMap::load(user_id)).await?;
    step(
        user_id,
        "posts",
        posts::migrate(user_id, &files, posts::BATCH_SIZE),
    )
    .await?;
    step(
        user_id,
        "tags",
        tags::migrate(user_id, &files, tags::BATCH_SIZE),
    )
    .await
}

/// Runs one step of the flip. A failure is logged with the user, the step and the cause, and
/// returned as [`EventProcessorError::FlipFailed`] naming the step and keeping the cause, so
/// its retry class is the flip's whatever the cause's.
pub(crate) async fn step<T, E>(
    user_id: &PubkyId,
    name: &'static str,
    task: impl Future<Output = Result<T, E>>,
) -> Result<T, EventProcessorError>
where
    E: Into<EventProcessorError>,
{
    task.await.map_err(|e| {
        let source = e.into();
        error!(%user_id, step = %name, error = %source, "Flip step failed");
        EventProcessorError::FlipFailed {
            step: name,
            source: Box::new(source),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_common::db::GraphError;

    const USER: &str = "operrr8wsbpr3ue9d4qj41ge1kcc6r7fdiy6o3ugjrrhi4y77rdo";

    /// A failed step comes out as `FlipFailed` naming the step and keeping the cause, a graph
    /// or a Redis error alike, and its message names both. A step may fail with the graph's
    /// own error, as the epoch write does: converted, then wrapped like any other.
    #[tokio::test]
    async fn a_failed_step_comes_out_as_flip_failed_naming_the_step_and_the_cause() {
        let user_id = PubkyId::try_from(USER).unwrap();
        let failures = [
            (
                "posts",
                EventProcessorError::GraphQueryFailed(false, "boom".to_string()),
            ),
            (
                "tags",
                EventProcessorError::IndexOperationFailed(true, "connection refused".to_string()),
            ),
        ];
        for (name, failure) in failures {
            let cause = failure.to_string();

            let result: Result<(), _> = step(&user_id, name, async { Err(failure) }).await;

            let err = result.unwrap_err();
            match &err {
                EventProcessorError::FlipFailed { step, source } => {
                    assert_eq!(*step, name);
                    assert_eq!(source.to_string(), cause);
                }
                other => panic!("expected FlipFailed, got {other:?}"),
            }
            let message = err.to_string();
            assert!(message.contains(&format!("step `{name}`")), "{message}");
            assert!(message.contains(&cause), "{message}");
        }

        let stalled = GraphError::KeysetScanStalled {
            context: "migration.posts".to_string(),
            detail: "cursor did not advance".to_string(),
        };

        let result: Result<(), _> = step(&user_id, "epoch", async { Err(stalled) }).await;

        assert!(
            matches!(
                result,
                Err(EventProcessorError::FlipFailed { step: "epoch", ref source })
                    if matches!(**source, EventProcessorError::DataAnomaly(_))
            ),
            "{result:?}"
        );
    }

    /// A failed step is logged with the user, the step and the cause.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn a_failed_step_is_logged_with_the_user_the_step_and_the_cause() {
        let user_id = PubkyId::try_from(USER).unwrap();
        let failure = EventProcessorError::GraphQueryFailed(false, "boom".to_string());

        let result: Result<(), _> = step(&user_id, "posts", async { Err(failure) }).await;

        assert!(result.is_err());
        assert!(logs_contain("Flip step failed"));
        assert!(logs_contain(&format!("user_id={USER}")));
        assert!(logs_contain("step=posts"));
        assert!(logs_contain("boom"));
    }

    /// A step that passes logs nothing.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn a_passing_step_logs_no_error() {
        let user_id = PubkyId::try_from(USER).unwrap();

        step(&user_id, "posts", async {
            Ok::<(), EventProcessorError>(())
        })
        .await
        .unwrap();

        assert!(!logs_contain("Flip step failed"));
    }
}
