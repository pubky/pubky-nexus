use std::error::Error;

use crate::db::kv::flush::clear_redis;
use crate::models::post::Bookmark;
use crate::models::tag::stream::GLOBAL_HOT_TAGS;
use crate::models::user::{Followers, Following, UserDetails, UserFollows};
use crate::models::traits::Collection;
use crate::{queries, RedisOps};
use crate::{
    db::connectors::neo4j::get_neo4j_graph,
    models::post::{PostCounts, PostDetails, PostRelationships},
    models::user::UserCounts,
};
use log::info;
use neo4rs::query;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;



pub async fn reindex() {
    // Clear Redis database
    if let Err(e) = clear_redis().await {
        log::error!("Failed to clear Redis: {:?}", e);
        return;
    }

    let mut user_tasks = JoinSet::new();
    let mut post_tasks = JoinSet::new();

    let user_ids: Vec<String> = get_all_user_ids().await.expect("Failed to get user IDs");
    let user_ids_refs: Vec<&str> = user_ids.iter().map(|id| id.as_str()).collect();

    UserDetails::from_graph(&user_ids_refs)
        .await
        .expect("Failed indexing User Details");
    //TODO use collections

    for user_id in user_ids {
        user_tasks.spawn(async move {
            if let Err(e) = reindex_user(&user_id).await {
                log::error!("Failed to reindex user {}: {:?}", user_id, e);
            }
        });
    }

    let post_ids = get_all_post_ids().await.expect("Failed to get post IDs");
    for (author_id, post_id) in post_ids {
        post_tasks.spawn(async move {
            if let Err(e) = reindex_post(&author_id, &post_id).await {
                log::error!("Failed to reindex post {}: {:?}", post_id, e);
            }
        });
    }

    while let Some(res) = user_tasks.join_next().await {
        if let Err(e) = res {
            log::error!("User reindexing task failed: {:?}", e);
        }
    }

    while let Some(res) = post_tasks.join_next().await {
        if let Err(e) = res {
            log::error!("Post reindexing task failed: {:?}", e);
        }
    }

    retrive_global_hot_tags().await.expect("Failed to store the global hot tags");

    info!("Reindexing completed successfully.");
}

async fn reindex_user(user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::try_join!(
        Bookmark::index_all_from_graph(user_id),
        UserCounts::get_from_graph(user_id),
        Followers::get_from_graph(user_id, Some(0), Some(100)),
        Following::get_from_graph(user_id, Some(0), Some(100)),
    )?;

    Ok(())
}

async fn reindex_post(
    author_id: &str,
    post_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::try_join!(
        PostDetails::get_from_graph(author_id, post_id),
        PostCounts::get_from_graph(author_id, post_id),
        PostRelationships::get_from_graph(author_id, post_id)
    )?;

    Ok(())
}

async fn get_all_user_ids() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let query = query("MATCH (u:User) RETURN u.id AS id");

    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;

    let mut user_ids = Vec::new();
    while let Some(row) = result.next().await? {
        if let Some(id) = row.get("id")? {
            user_ids.push(id);
        }
    }

    Ok(user_ids)
}

async fn get_all_post_ids(
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let graph = get_neo4j_graph()?;
    let query =
        query("MATCH (u:User)-[:AUTHORED]->(p:Post) RETURN u.id AS author_id, p.id AS post_id");

    let graph = graph.lock().await;
    let mut result = graph.execute(query).await?;

    let mut post_ids = Vec::new();
    while let Some(row) = result.next().await? {
        if let (Some(author_id), Some(post_id)) = (row.get("author_id")?, row.get("post_id")?) {
            post_ids.push((author_id, post_id));
        }
    }

    Ok(post_ids)
}

// Create a fake struct to impl RedisOps
#[derive(Serialize, Deserialize, Debug)]
pub struct HotTagsScore {}

impl RedisOps for HotTagsScore {}

async fn retrive_global_hot_tags() -> Result<(), Box<dyn Error + Send + Sync>> {
    info!("Retrieving tags global score...");
    let graph = get_neo4j_graph()?;
    let graph = graph.lock().await;

    let query = queries::get_global_hot_tags_scores();
    let mut result = graph.execute(query).await?;

    if let Some(row) = result.next().await? {
        let hot_tags: Vec<(f64, &str)> = row.get("hot_tags")?;
        HotTagsScore::put_index_sorted_set(
            &GLOBAL_HOT_TAGS,
            hot_tags.as_slice()
        )
        .await?
    }
    Ok(())
}