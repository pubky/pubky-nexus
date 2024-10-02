use crate::db::kv::index::json::JsonAction;
use crate::models::notification::Notification;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppFollow;
use crate::models::user::{Followers, Following};
use crate::models::user::{PubkyId, UserCounts, UserFollows};
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(
    follower_id: PubkyId,
    followee_id: PubkyId,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new follow: {} -> {}", follower_id, followee_id);

    // TODO: Deserialize and validate content of follow data (not needed, but we could validate the timestamp)
    let _follow = <PubkyAppFollow as Validatable>::try_from(&blob).await?;

    sync_put(follower_id, followee_id).await
}

pub async fn sync_put(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // SAVE TO GRAPH
    Followers::put_to_graph(&follower_id, &followee_id).await?;

    // SAVE TO INDEX
    // (follower_id)-[:FOLLOWS]->(followee_id)
    Followers(vec![follower_id.to_string()])
        .put_to_index(&followee_id)
        .await?;
    Following(vec![followee_id.to_string()])
        .put_to_index(&follower_id)
        .await?;

    let new_friend = update_follows_counts(&follower_id, &followee_id, JsonAction::Increment(1)).await?;

    // Notify the followee
    Notification::new_follow(&follower_id, &followee_id, new_friend).await?;

    Ok(())
}

pub async fn del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);
    // Maybe we could do it here but lets follow the naming convention 
    sync_del(follower_id, followee_id).await
    
}

pub async fn sync_del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // DELETE FROM GRAPH
    Followers::del_from_graph(&follower_id, &followee_id).await?;

    // REMOVE FROM INDEX
    Following(vec![followee_id.to_string()])
        .del_from_index(&follower_id)
        .await?;
    Followers(vec![follower_id.to_string()])
        .del_from_index(&followee_id)
        .await?;

    let were_friends = update_follows_counts(&follower_id, &followee_id, JsonAction::Decrement(1)).await?;

    // Notify the followee
    Notification::lost_follow(&follower_id, &followee_id, were_friends).await?;

    Ok(())
}

async fn update_follows_counts(
    follower_id: &str,
    followee_id: &str,
    counter: JsonAction
) -> Result<bool, Box<dyn Error + Sync + Send>> {
    // Update UserCount related indexes
    UserCounts::update_index_field(&follower_id, "following", counter.clone()).await?;
    UserCounts::update(&followee_id, "followers", counter.clone()).await?;

    // Checks whether the followee was following the follower (Is this a new friendship?)
    let are_friends = Followers::check(&follower_id, &followee_id).await?;
    if are_friends {
        UserCounts::update_index_field(&follower_id, "friends", counter.clone()).await?;
        UserCounts::update_index_field(&followee_id, "friends", counter.clone()).await?;
    }
    Ok(are_friends)
}
