use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppFollow;
use crate::models::user::PubkyId;
use crate::models::user::{Followers, Following};
use crate::{queries, RedisOps};
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

pub async fn put(
    user_id: PubkyId,
    followee_id: &str,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new follow: {} -> {}", user_id, followee_id);

    // TODO: Deserialize and validate content of follow data (not needed, but we could validate the timestamp)
    let _follow = <PubkyAppFollow as Validatable>::try_from(&blob)?;

    // Save new relationship on graph
    let indexed_at = Utc::now().timestamp_millis();
    let query = queries::write::create_follow(&user_id, followee_id, indexed_at);
    exec_single_row(query).await?;

    // Update follow indexes
    Followers(vec![followee_id.to_string()])
        .put_index_set(&[user_id.as_ref()])
        .await?;
    Following(vec![user_id.to_string()])
        .put_index_set(&[followee_id])
        .await?;

    Ok(())
}

pub async fn del(user_id: PubkyId, followee_id: &str) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting follow: {} -> {}", user_id, followee_id);

    // Delete the follow relationship from Neo4j
    let query = queries::write::delete_follow(&user_id, followee_id);
    exec_single_row(query).await?;

    // Update follow indexes
    Following(vec![followee_id.to_string()])
        .remove_from_index_set(&[user_id.as_ref()])
        .await?;
    Followers(vec![user_id.to_string()])
        .remove_from_index_set(&[followee_id])
        .await?;

    Ok(())
}
