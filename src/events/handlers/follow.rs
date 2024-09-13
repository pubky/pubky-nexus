use crate::db::graph::exec::exec_single_row;
use crate::models::notification::Notification;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppFollow;
use crate::models::user::PubkyId;
use crate::models::user::{Followers, Following};
use crate::reindex::ingest_follow;
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

    // Save new relationship on graph
    let indexed_at = Utc::now().timestamp_millis();
    let query = queries::write::create_follow(&follower_id, &followee_id, indexed_at);
    exec_single_row(query).await?;

    // Notify the followee
    Notification::new_follow(&follower_id, &followee_id, false).await?;// SAVE TO INDEX
    // Update follow data
    ingest_follow(follower_id, followee_id).await
}

pub async fn del(
    follower_id: PubkyId,
    followee_id: PubkyId,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting follow: {} -> {}", follower_id, followee_id);

    // Delete the follow relationship from Neo4j
    let query = queries::write::delete_follow(&follower_id, &followee_id);
    exec_single_row(query).await?;

    // Update follow indexes
    Following(vec![followee_id.to_string()])
        .remove_from_index_set(&[&follower_id])
        .await?;
    Followers(vec![follower_id.0])
        .remove_from_index_set(&[&followee_id])
        .await?;

    Ok(())
}
