use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;

use nexus_common::db::kv::JsonAction;
use nexus_common::db::OperationOutcome;
use nexus_common::models::follow::{Followers, Following, Friends, UserFollows};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::notification::Notification;
use nexus_common::models::user::UserCounts;
use pubky_app_specs::PubkyId;
use tracing::debug;

pub async fn sync_put(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new follow: {} -> {}", follower_id, followee_id);
    // SAVE TO GRAPH
    // (follower_id)-[:FOLLOWS]->(followee_id)
    match Followers::put_to_graph(&follower_id, &followee_id).await? {
        // Do not duplicate the follow relationship
        OperationOutcome::Updated => return Ok(()),
        OperationOutcome::MissingDependency => {
            if let Err(e) = Homeserver::maybe_ingest_for_user(followee_id.as_str()).await {
                tracing::error!("Failed to ingest homeserver: {e}");
            }

            let key = RetryEvent::generate_index_key_from_uri(&followee_id.to_uri());
            let dependency = vec![key];
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
            let indexing_results = tokio::join!(
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

pub async fn del(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), EventProcessorError> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);
    // Maybe we could do it here but lets follow the naming convention
    sync_del(follower_id, followee_id).await
}

pub async fn sync_del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), EventProcessorError> {
    match Followers::del_from_graph(&follower_id, &followee_id).await? {
        // Both users exists but they do not have that relationship
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => Err(EventProcessorError::SkipIndexing),
        OperationOutcome::CreatedOrDeleted => {
            // Check if the users are friends. Is this a break? :(
            let were_friends = Friends::check(&follower_id, &followee_id).await?;

            // REMOVE FROM INDEX
            let followers = Followers(vec![follower_id.to_string()]);
            let following = Following(vec![followee_id.to_string()]);

            let indexing_results = tokio::join!(
                // Remove a follower to the followee index
                followers.del_from_index(&followee_id),
                // Remove from the Following:follower_id index a followee user
                following.del_from_index(&follower_id),
                update_follow_counts(
                    &follower_id,
                    &followee_id,
                    JsonAction::Decrement(1),
                    were_friends,
                ),
                // Notify the followee
                Notification::lost_follow(&follower_id, &followee_id, were_friends)
            );
            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;

            Ok(())
        }
    }
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
