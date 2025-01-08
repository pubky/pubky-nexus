use crate::db::graph::exec::OperationOutcome;
use crate::db::kv::index::json::JsonAction;
use crate::models::follow::{Followers, Following, Friends, UserFollows};
use crate::models::notification::Notification;
use crate::models::user::UserCounts;
use crate::types::DynError;
use crate::types::PubkyId;
use axum::body::Bytes;
use log::debug;

pub async fn put(follower_id: PubkyId, followee_id: PubkyId, _blob: Bytes) -> Result<(), DynError> {
    debug!("Indexing new follow: {} -> {}", follower_id, followee_id);

    // TODO: in case we want to validate the content of this homeserver object or its `created_at` timestamp
    // let _follow = <PubkyAppFollow as Validatable>::try_from(&blob, &followee_id).await?;

    sync_put(follower_id, followee_id).await
}

pub async fn sync_put(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), DynError> {
    // SAVE TO GRAPH
    // (follower_id)-[:FOLLOWS]->(followee_id)
    match Followers::put_to_graph(&follower_id, &followee_id).await? {
        // Do not duplicate the follow relationship
        OperationOutcome::Updated => return Ok(()),
        // TODO: Should return an error that should be processed by RetryManager
        // WIP: Create a custom error type to pass enough info to the RetryManager
        OperationOutcome::Pending => {
            return Err("WATCHER: Missing some dependency to index the model".into())
        }
        // The relationship did not exist, create all related indexes
        OperationOutcome::Created => {
            // Checks whether the followee was following the follower (Is this a new friendship?)
            let will_be_friends =
                is_followee_following_follower(&follower_id, &followee_id).await?;

            // SAVE TO INDEX
            // Add new follower to the followee index
            Followers(vec![follower_id.to_string()])
                .put_to_index(&followee_id)
                .await?;
            // Add in the Following:follower_id index a followee user
            Following(vec![followee_id.to_string()])
                .put_to_index(&follower_id)
                .await?;

            update_follow_counts(
                &follower_id,
                &followee_id,
                JsonAction::Increment(1),
                will_be_friends,
            )
            .await?;

            // Notify the followee
            Notification::new_follow(&follower_id, &followee_id, will_be_friends).await?;
        }
    };

    Ok(())
}

pub async fn del(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), DynError> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);
    // Maybe we could do it here but lets follow the naming convention
    sync_del(follower_id, followee_id).await
}

pub async fn sync_del(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), DynError> {
    // DELETE FROM GRAPH
    match Followers::del_from_graph(&follower_id, &followee_id).await? {
        // Both users exists but they do not have that relationship
        OperationOutcome::Created => Ok(()),
        OperationOutcome::Pending => {
            Err("WATCHER: Missing some dependency to index the model".into())
        }
        OperationOutcome::Updated => {
            // Check if the users are friends. Is this a break? :(
            let were_friends = Friends::check(&follower_id, &followee_id).await?;

            // REMOVE FROM INDEX
            // Remove a follower to the followee index
            Followers(vec![follower_id.to_string()])
                .del_from_index(&followee_id)
                .await?;
            // Remove from the Following:follower_id index a followee user
            Following(vec![followee_id.to_string()])
                .del_from_index(&follower_id)
                .await?;

            update_follow_counts(
                &follower_id,
                &followee_id,
                JsonAction::Decrement(1),
                were_friends,
            )
            .await?;

            // Notify the followee
            Notification::lost_follow(&follower_id, &followee_id, were_friends).await?;

            Ok(())
        }
    }
}

async fn update_follow_counts(
    follower_id: &str,
    followee_id: &str,
    counter: JsonAction,
    update_friend_relationship: bool,
) -> Result<(), DynError> {
    // Update UserCount related indexes
    UserCounts::update_index_field(follower_id, "following", counter.clone()).await?;
    UserCounts::update(followee_id, "followers", counter.clone()).await?;

    if update_friend_relationship {
        UserCounts::update_index_field(follower_id, "friends", counter.clone()).await?;
        UserCounts::update_index_field(followee_id, "friends", counter.clone()).await?;
    }
    Ok(())
}

pub async fn is_followee_following_follower(
    user_a_id: &str,
    user_b_id: &str,
) -> Result<bool, DynError> {
    let (a_follows_b, b_follows_a) = tokio::try_join!(
        Following::check(user_a_id, user_b_id),
        Following::check(user_b_id, user_a_id),
    )?;
    // Cannot exist any previous relationship between A and B. If not, it would be duplicate event
    // (A)-[:FOLLOWS]->(B)
    Ok(!a_follows_b && b_follows_a)
}
