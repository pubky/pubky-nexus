use crate::events::EventProcessorError;

use nexus_common::db::kv::{guards, JsonAction};
use nexus_common::db::OperationOutcome;
use nexus_common::models::follow::{Followers, Following, Friends, UserFollows};
use nexus_common::models::notification::Notification;
use nexus_common::models::user::{UserCounts, UserIngestor};
use pubky_app_specs::PubkyId;
use tracing::debug;

use super::utils::{fail_on_blacklisted_hs, follow_deletion_guard_key, DELETION_GUARD_TTL_SECS};

#[tracing::instrument(name = "follow.put", skip_all, fields(follower_id = %follower_id, followee_id = %followee_id))]
pub async fn sync_put(
    follower_id: PubkyId,
    followee_id: PubkyId,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new follow: {} -> {}", follower_id, followee_id);
    // SAVE TO GRAPH
    // (follower_id)-[:FOLLOWS]->(followee_id)
    match Followers::put_to_graph(&follower_id, &followee_id).await? {
        OperationOutcome::Updated => {
            // Retry / duplicate: graph edge already exists.
            // Re-run idempotent index writes (SADD is a no-op for existing members)
            // to recover from partial failures where graph wrote but indexes didn't.
            // Skip counters and notifications (prefer 0 over N).
            let followers = Followers(vec![follower_id.to_string()]);
            let following = Following(vec![followee_id.to_string()]);
            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write");
                followers.put_to_index(&followee_id),
                following.put_to_index(&follower_id)
            );
            indexing_results.0?;
            indexing_results.1?;
            return Ok(());
        }
        OperationOutcome::MissingDependency => {
            // Drop the follow (non-retryable) if the followee's HS is blacklisted.
            fail_on_blacklisted_hs(ingestor.maybe_ingest_user(&followee_id).await)?;

            let followee_uri = followee_id
                .to_uri()
                .try_to_uri_str()
                .map_err(EventProcessorError::generic)?;
            let dependency = vec![followee_uri];
            return Err(EventProcessorError::MissingDependency { dependency });
        }
        // The relationship did not exist, create all related indexes
        OperationOutcome::CreatedOrDeleted => {
            // Checks whether the followee was following the follower (Is this a new friendship?)
            let will_be_friends =
                is_followee_following_follower(&follower_id, &followee_id).await?;

            let followers = Followers(vec![follower_id.to_string()]);
            let following = Following(vec![followee_id.to_string()]);

            // SAVE TO INDEX
            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write");
                // Add new follower to the followee index
                followers.put_to_index(&followee_id),
                // Add in the Following:follower_id index a followee user
                following.put_to_index(&follower_id),
                update_follow_counts(
                    &follower_id,
                    &followee_id,
                    JsonAction::Increment(1),
                    will_be_friends
                ),
                // Notify the followee
                Notification::new_follow(&follower_id, &followee_id, will_be_friends)
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;
        }
    };

    Ok(())
}

#[tracing::instrument(name = "follow.del", skip_all, fields(follower_id = %follower_id, followee_id = %followee_id))]
pub async fn del(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);
    // Maybe we could do it here but lets follow the naming convention
    sync_del(follower_id, followee_id).await
}

pub async fn sync_del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), EventProcessorError> {
    // Check friendship while Redis follow sets are still populated
    let were_friends = Friends::check(&follower_id, &followee_id).await?;

    // Guard counters/notifications: only run if still in Redis index (first attempt).
    // On retry (Redis already cleaned, graph edge still present), skip non-idempotent ops.
    let still_indexed = Followers::check_in_index(&followee_id, &follower_id).await?;

    // SETNX deletion tombstone: `still_indexed` alone is not retry-safe because
    // read-through can resurrect the follow sets (see `follow_deletion_guard_key`).
    // Acquired only now, after all reads, so a transient read failure stays
    // retryable with side effects intact.
    let deletion_guard_key = follow_deletion_guard_key(&follower_id, &followee_id);
    let first_attempt = guards::try_acquire(&deletion_guard_key, DELETION_GUARD_TTL_SECS).await?;

    // Non-idempotent side effects run only when both gates agree this is the
    // first attempt.
    let run_side_effects = still_indexed && first_attempt;

    let followers = Followers(vec![follower_id.to_string()]);
    let following = Following(vec![followee_id.to_string()]);

    // Redis cleanup first — SREM is idempotent
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete");
        followers.del_from_index(&followee_id),
        following.del_from_index(&follower_id)
    );
    indexing_results.0?;
    indexing_results.1?;

    // Only after indexes are confirmed clean: non-idempotent ops
    if run_side_effects {
        update_follow_counts(
            &follower_id,
            &followee_id,
            JsonAction::Decrement(1),
            were_friends,
        )
        .await?;
        Notification::lost_follow(&follower_id, &followee_id, were_friends).await?;
    }

    // Graph deletion LAST — on retry, we re-enter here with indexes already clean.
    // MissingDependency means the resource is already gone — deletion is complete.
    Followers::del_from_graph(&follower_id, &followee_id).await?;

    // The delete completed: drop the tombstone. Best-effort: the TTL backstops
    // a leaked key, and failing the event after a successful graph delete would
    // only force a useless retry cycle.
    if let Err(e) = guards::release(&deletion_guard_key).await {
        tracing::warn!("failed to release deletion guard {deletion_guard_key}: {e}");
    }
    Ok(())
}

async fn update_follow_counts(
    follower_id: &str,
    followee_id: &str,
    counter: JsonAction,
    update_friend_relationship: bool,
) -> Result<(), EventProcessorError> {
    // Update UserCount related indexes
    UserCounts::update_index_field(follower_id, "following", counter.clone()).await?;
    UserCounts::update(followee_id, "followers", counter.clone(), None).await?;

    if update_friend_relationship {
        UserCounts::update_index_field(follower_id, "friends", counter.clone()).await?;
        UserCounts::update_index_field(followee_id, "friends", counter.clone()).await?;
    }
    Ok(())
}

pub async fn is_followee_following_follower(
    user_a_id: &str,
    user_b_id: &str,
) -> Result<bool, EventProcessorError> {
    let (a_follows_b, b_follows_a) = tokio::try_join!(
        Following::check_in_index(user_a_id, user_b_id),
        Following::check_in_index(user_b_id, user_a_id),
    )?;
    // Cannot exist any previous relationship between A and B. If not, it would be duplicate event
    // (A)-[:FOLLOWS]->(B)
    Ok(!a_follows_b && b_follows_a)
}
