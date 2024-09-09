use crate::db::kv::flush::clear_redis;
use crate::models::post::{Bookmark, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS};
use crate::models::tag::post::{TagPost, POST_TAGS_KEY_PARTS};
use crate::models::tag::search::{TagSearch, TAG_GLOBAL_POST_ENGAGEMENT, TAG_GLOBAL_POST_TIMELINE};
use crate::models::tag::stream::{HotTags, Taggers, TAG_GLOBAL_HOT};
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::traits::Collection;
use crate::models::user::{Followers, Following, UserDetails, UserFollows};
use crate::{RedisOps, ScoreAction};
use crate::{
    db::connectors::neo4j::get_neo4j_graph,
    models::post::{PostCounts, PostDetails, PostRelationships},
    models::user::UserCounts,
};
use log::info;
use neo4rs::query;
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
    //TODO use collections for every other model

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

    HotTags::set_global_tag_scores()
        .await
        .expect("Failed to store the global hot tags");

    TagSearch::index_post_tags_from_graph()
        .await
        .expect("Failed to store the global post tags");

    info!("Reindexing completed successfully.");
}

pub async fn reindex_user(user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::try_join!(
        Bookmark::index_all_from_graph(user_id),
        UserCounts::get_from_graph(user_id),
        Followers::get_from_graph(user_id, Some(0), Some(100)),
        Following::get_from_graph(user_id, Some(0), Some(100)),
        TagUser::get_from_graph(user_id, None)
    )?;

    Ok(())
}

pub async fn reindex_post(
    author_id: &str,
    post_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::try_join!(
        PostDetails::get_from_graph(author_id, post_id),
        PostCounts::get_from_graph(author_id, post_id),
        PostRelationships::get_from_graph(author_id, post_id),
        TagPost::get_from_graph(author_id, Some(post_id))
    )?;

    Ok(())
}

pub async fn reindex_post_tags(
    user_id: &str, author_id: &str, post_id: &str, tag_label: &str
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: Carefull with that operation, we might lost data consistency, if one of that fails
    // we might need to enforce data consistency
    let author_post_slice: &[&str] = &[&author_id, &post_id];
    let user_id_slice = [user_id];
    let tag_label_slice = [tag_label];
    let user_post_slice = [author_id, post_id, tag_label];
    // Add post to label total engagement
    let tag_global_engagement_key_parts = [&TAG_GLOBAL_POST_ENGAGEMENT[..], &[&tag_label]].concat();
    let post_tags_key_parts = [&POST_TAGS_KEY_PARTS[..], author_post_slice].concat();


    tokio::try_join!(
        // Increment in one the post tags
        // TODO: the increment has to be generic, also has to rest
        PostCounts::put_param_index_json(author_post_slice, "tags"),
        // Add user tag in post
        TagPost::put_index_set(
            &user_post_slice, 
            &user_id_slice
        ),
        // Increment in one post global engagement
        PostStream::put_score_index_sorted_set(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, author_post_slice, ScoreAction::Increment(1.0)),
        // Add post to label total engagement
        TagSearch::put_score_index_sorted_set(&tag_global_engagement_key_parts, author_post_slice, ScoreAction::Increment(1.0)),
        // Add label to hot tags
        Taggers::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &tag_label_slice, ScoreAction::Increment(1.0)),
        // Add user to post taggers
        Taggers::put_index_set(
            &tag_label_slice, 
            &user_id_slice
        ),
        // Add label to post
        TagSearch::put_score_index_sorted_set(&post_tags_key_parts, &tag_label_slice, ScoreAction::Increment(1.0))
    )?;

    // Add post to global label timeline
    let key_parts = [&TAG_GLOBAL_POST_TIMELINE[..], &[&tag_label]].concat();
    let res = TagSearch::check_sorted_set_member(&key_parts, &author_post_slice).await.unwrap();
    if res == None {
        let option = PostDetails::try_from_index_json(&author_post_slice).await?;
        if let Some(post_details) = option {
            println!("IndexedAt: {:?}", post_details.indexed_at);
            let member_key = author_post_slice.join(":");
            TagSearch::put_index_sorted_set(&key_parts, &[(post_details.indexed_at as f64, &member_key)]).await?;
        }
    }
    // TODO: Maybe work in the else
    
    Ok(())
}

async fn get_all_user_ids() -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let query = query("MATCH (u:User) RETURN u.id AS id");

        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }

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
    let mut result;
    {
        let graph = get_neo4j_graph()?;
        let query =
            query("MATCH (u:User)-[:AUTHORED]->(p:Post) RETURN u.id AS author_id, p.id AS post_id");

        let graph = graph.lock().await;
        result = graph.execute(query).await?;
    }

    let mut post_ids = Vec::new();
    while let Some(row) = result.next().await? {
        if let (Some(author_id), Some(post_id)) = (row.get("author_id")?, row.get("post_id")?) {
            post_ids.push((author_id, post_id));
        }
    }

    Ok(post_ids)
}
