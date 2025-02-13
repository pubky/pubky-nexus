use crate::db::graph::exec::OperationOutcome;
use crate::db::kv::index::json::JsonAction;
use crate::events::error::EventProcessorError;
use crate::events::retry::event::RetryEvent;
use crate::handle_indexing_results;
use crate::models::follow::{Followers, Following, Friends, UserFollows};
use crate::models::notification::Notification;
use crate::models::user::UserCounts;
use crate::types::DynError;
use log::debug;
use pubky_app_specs::{user_uri_builder, PubkyId};

pub async fn sync_put(follower_id: PubkyId, followee_id: PubkyId) -> Result<(), DynError> {
    debug!("Indexing new follow: {} -> {}", follower_id, followee_id);
    // SAVE TO GRAPH
    // (follower_id)-[:FOLLOWS]->(followee_id)
    match Followers::put_to_graph(&follower_id, &followee_id).await? {
        // Do not duplicate the follow relationship
        OperationOutcome::Updated => return Ok(()),
        OperationOutcome::MissingDependency => {
            if let Some(key) =
                RetryEvent::generate_index_key(&user_uri_builder(followee_id.to_string()))
            {
                let dependency = vec![key];
                return Err(EventProcessorError::MissingDependency { dependency }.into());
            }
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

            handle_indexing_results!(
                indexing_results.0,
                indexing_results.1,
                indexing_results.2,
                indexing_results.3
            );
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
    match Followers::del_from_graph(&follower_id, &followee_id).await? {
        // Both users exists but they do not have that relationship
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => Err(EventProcessorError::SkipIndexing.into()),
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
            handle_indexing_results!(
                indexing_results.0,
                indexing_results.1,
                indexing_results.2,
                indexing_results.3
            );

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
