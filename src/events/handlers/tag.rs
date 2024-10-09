use crate::db::graph::exec::exec_single_row;
use crate::db::kv::index::json::JsonAction;
use crate::events::uri::ParsedUri;
use crate::models::notification::Notification;
use crate::models::post::{PostCounts, PostStream};
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppTag;
use crate::models::tag::post::TagPost;
use crate::models::tag::search::TagSearch;
use crate::models::tag::stream::Taggers;
use crate::models::tag::traits::TagCollection;
use crate::models::tag::user::TagUser;
use crate::models::user::{PubkyId, UserCounts};
use crate::{queries, ScoreAction};
use axum::body::Bytes;
use chrono::Utc;
use log::debug;
use std::error::Error;

pub async fn put(
    tagger_id: PubkyId,
    tag_id: String,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Indexing new tag: {} -> {}", tagger_id, tag_id);

    // Deserialize and validate tag
    let tag = <PubkyAppTag as Validatable>::try_from(&blob, &tag_id).await?;

    // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
    let parsed_uri = ParsedUri::try_from(tag.uri.as_str())?;
    let indexed_at = Utc::now().timestamp_millis();

    match parsed_uri.post_id {
        // If post_id is in the tagged URI, we place tag to a post.
        Some(post_id) => {
            put_sync_post(
                tagger_id,
                parsed_uri.user_id,
                post_id,
                tag_id,
                tag.label,
                tag.uri,
                indexed_at,
            )
            .await
        }
        // If no post_id in the tagged URI, we place tag to a user.
        None => put_sync_user(tagger_id, parsed_uri.user_id, tag_id, tag.label, indexed_at).await,
    }
}

async fn put_sync_post(
    tagger_user_id: PubkyId,
    author_id: PubkyId,
    post_id: String,
    tag_id: String,
    tag_label: String,
    post_uri: String,
    indexed_at: i64,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // SAVE TO GRAPH
    TagPost::put_to_graph(
        &tagger_user_id,
        &author_id,
        Some(&post_id),
        &tag_id,
        &tag_label,
        indexed_at,
    )
    .await?;

    // SAVE TO INDEXES
    let post_key_slice: &[&str] = &[&author_id, &post_id];

    // TODO: Handle the errors
    let _ = tokio::join!(
        // Update user counts for tagger
        UserCounts::update(&tagger_user_id, "tags", JsonAction::Increment(1)),
        // Increment in one the post tags
        PostCounts::update_index_field(post_key_slice, "tags", JsonAction::Increment(1)),
        // Add label to post
        TagPost::update_index_score(
            &author_id,
            Some(&post_id),
            &tag_label,
            ScoreAction::Increment(1.0)
        ),
        // Add user tag in post
        TagPost::add_tagger_to_index(&author_id, Some(&post_id), &tagger_user_id, &tag_label),
        // Increment in one post global engagement
        PostStream::update_index_score(&author_id, &post_id, ScoreAction::Increment(1.0)),
        // Add post to label total engagement
        TagSearch::update_index_score(
            &author_id,
            &post_id,
            &tag_label,
            ScoreAction::Increment(1.0)
        ),
        // Add label to hot tags
        Taggers::update_index_score(&tag_label, ScoreAction::Increment(1.0)),
        // Add tagger to post taggers
        Taggers::put_to_index(&tag_label, &tagger_user_id)
    );

    // Add post to global label timeline
    TagSearch::add_to_timeline_sorted_set(&author_id, &post_id, &tag_label).await?;

    // Save new notification
    Notification::new_post_tag(&tagger_user_id, &author_id, &tag_label, &post_uri).await?;

    Ok(())
}

async fn put_sync_user(
    tagger_user_id: PubkyId,
    tagged_user_id: PubkyId,
    tag_id: String,
    tag_label: String,
    indexed_at: i64,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // SAVE TO GRAPH
    TagUser::put_to_graph(
        &tagger_user_id,
        &tagged_user_id,
        None,
        &tag_id,
        &tag_label,
        indexed_at,
    )
    .await?;

    // SAVE TO INDEX
    // Update user counts for the tagged user
    UserCounts::update(&tagged_user_id, "tagged", JsonAction::Increment(1)).await?;

    // Update user counts for the tagger user
    UserCounts::update(&tagger_user_id, "tags", JsonAction::Increment(1)).await?;

    // Add tagger to the user taggers list
    TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, &tag_label).await?;

    // Add label count to the user profile tag
    TagUser::update_index_score(
        &tagged_user_id,
        None,
        &tag_label,
        ScoreAction::Increment(1.0),
    )
    .await?;

    // Save new notification
    Notification::new_user_tag(&tagger_user_id, &tagged_user_id, &tag_label).await?;
    Ok(())
}

pub async fn del(user_id: PubkyId, tag_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting tag: {} -> {}", user_id, tag_id);

    // Delete the tag relationship from the graph
    let query = queries::del::delete_tag(&user_id, &tag_id);
    exec_single_row(query).await?;

    Ok(())
}
