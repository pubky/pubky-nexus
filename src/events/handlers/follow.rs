use crate::db::graph::exec::exec_single_row;
use crate::db::kv::index::json::JsonAction;
use crate::models::notification::Notification;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppFollow;
use crate::models::user::{Followers, Following, Friends};
use crate::models::user::{PubkyId, UserCounts, UserFollows};
use crate::{queries, RedisOps};
use axum::body::Bytes;
use chrono::Utc;
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
    let indexed_at = Utc::now().timestamp_millis();
    let query = queries::write::create_follow(&follower_id, &followee_id, indexed_at);
    exec_single_row(query).await?;

    // SAVE TO INDEX
    // Update follow indexes
    // (follower_id)-[:FOLLOWS]->(followee_id)
    Followers(vec![follower_id.to_string()])
        .put_to_index(&followee_id)
        .await?;
    Following(vec![followee_id.to_string()])
        .put_to_index(&follower_id)
        .await?;

    // Update UserCount related indexes
    UserCounts::update_index_field(&follower_id, "following", JsonAction::Increment(1)).await?;
    UserCounts::update(&followee_id, "followers", JsonAction::Increment(1)).await?;

    // Checks whether the followee was following the follower (Is this a new friendship?)
    let new_friend = Followers::check(&follower_id, &followee_id).await?;
    if new_friend {
        UserCounts::update_index_field(&follower_id, "friends", JsonAction::Increment(1)).await?;
        UserCounts::update_index_field(&followee_id, "friends", JsonAction::Increment(1)).await?;
    }

    // Notify the followee
    Notification::new_follow(&follower_id, &followee_id, new_friend).await?;

    Ok(())
}

pub async fn del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);

    // Delete the follow relationship from Neo4j
    let query = queries::write::delete_follow(&follower_id, &followee_id);
    exec_single_row(query).await?;

    // Checks whether the follower and followee were friends
    let were_friends = Friends::check(&followee_id, &follower_id).await?;

    // Update follow indexes
    Following(vec![followee_id.to_string()])
        .remove_from_index_set(&[&follower_id])
        .await?;
    Followers(vec![follower_id.to_string()])
        .remove_from_index_set(&[&followee_id])
        .await?;

    // Notify the followee
    Notification::lost_follow(&follower_id, &followee_id, were_friends).await?;

    Ok(())
}
