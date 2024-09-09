use crate::db::graph::exec::exec_single_row;
use crate::events::uri::ParsedUri;
use crate::models::post::{PostCounts, PostDetails, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS};
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppTag;
use crate::models::tag::post::{TagPost, POST_TAGS_KEY_PARTS};
use crate::models::tag::search::{TagSearch, TAG_GLOBAL_POST_ENGAGEMENT, TAG_GLOBAL_POST_TIMELINE};
use crate::models::tag::stream::{Taggers, TAG_GLOBAL_HOT};
use crate::models::user::PubkyId;
use crate::{queries, RedisOps, ScoreAction};
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

pub async fn put(
    user_id: PubkyId,
    tag_id: String,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new tag: {} -> {}", user_id, tag_id);

    // Deserialize and validate tag
    let tag = <PubkyAppTag as Validatable>::try_from(&blob)?;

    // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
    let parsed_uri = ParsedUri::try_from(tag.uri.as_str())?;
    let indexed_at = Utc::now().timestamp_millis();

    match parsed_uri.post_id {
        // If post_id is in the tagged URI, we place tag to a post.
        Some(post_id) => {
            put_post_tag(
                user_id,
                parsed_uri.user_id,
                post_id,
                tag_id,
                tag.label,
                indexed_at,
            )
            .await
        }
        // If no post_id in the tagged URI, we place tag to a user.
        None => put_user_tag(user_id, parsed_uri.user_id, tag_id, tag.label, indexed_at).await,
    }
}

async fn put_post_tag(
    user_id: PubkyId,
    author_id: PubkyId,
    post_id: String,
    tag_id: String,
    tag_label: String,
    indexed_at: i64,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Save new post tag to graph
    let query = queries::write::create_post_tag(
        &user_id, &author_id, &post_id, &tag_id, &tag_label, indexed_at,
    );
    exec_single_row(query).await?;

    let user_id_slice = user_id.to_string();
    let author_id_slice = author_id.to_string();
    let post_id_slice = post_id.to_string();

    // TODO: index TAG to Redis and add to sorted sets
    // TODO: Carefull with that operation, we might lost data consistency, if one of that fails
    // we might need to enforce data consistency
    //
    let author_post_slice: &[&str] = &[&author_id_slice, &post_id_slice];
    // TODO: the increment has to be generic, also has to rest
    // Increment in one the post tags
    PostCounts::put_param_index_json(author_post_slice, "tags").await?;
    // Add user tag in post
    TagPost::put_index_set(
        &[&author_id_slice, &post_id_slice, &tag_label], 
        &vec![user_id_slice.clone()]
    ).await?;
    // Increment in one post global engagement
    PostStream::put_score_index_sorted_set(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, author_post_slice, ScoreAction::Increment(1.0)).await?;
    // Add post to global label timeline
    let key_parts = [&TAG_GLOBAL_POST_TIMELINE[..], &[&tag_label]].concat();
    let res = TagSearch::check_sorted_set_member(&key_parts, &author_post_slice).await.unwrap();
    if res == None {
        let option = PostDetails::try_from_index_json(&author_post_slice).await?;
        if let Some(post_details) = option {
            println!("{:?}", post_details.indexed_at);
            let member_key = author_post_slice.join(":");
            TagSearch::put_index_sorted_set(&key_parts, &[(post_details.indexed_at as f64, &member_key)]).await?;
        }
    }
    // Add post to label total engagement
    let key_parts = [&TAG_GLOBAL_POST_ENGAGEMENT[..], &[&tag_label]].concat();
    TagSearch::put_score_index_sorted_set(&key_parts, author_post_slice, ScoreAction::Increment(1.0)).await?;

    // Add label to hot tags
    Taggers::put_score_index_sorted_set(&TAG_GLOBAL_HOT, &[&tag_label], ScoreAction::Increment(1.0)).await?;

    // Add user to post taggers
    Taggers::put_index_set(
        &[&tag_label], 
        &vec![user_id_slice.clone()]
    ).await?;

    // Add label to post
    let key_parts = [&POST_TAGS_KEY_PARTS[..], author_post_slice].concat();
    TagSearch::put_score_index_sorted_set(&key_parts, &[&tag_label], ScoreAction::Increment(10.0)).await?;

    Ok(())
}

async fn put_user_tag(
    user_id: PubkyId,
    tagged_user_id: PubkyId,
    tag_id: String,
    tag_label: String,
    indexed_at: i64,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Save new post tag to graph
    let query =
        queries::write::create_user_tag(&user_id, &tagged_user_id, &tag_id, &tag_label, indexed_at);
    exec_single_row(query).await?;

    // TODO: index TAG to Redis and add to sorted sets

    Ok(())
}

pub async fn del(user_id: PubkyId, tag_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting tag: {} -> {}", user_id, tag_id);

    // Delete the tag relationship from the graph
    let query = queries::write::delete_tag(&user_id, &tag_id);
    exec_single_row(query).await?;

    Ok(())
}
