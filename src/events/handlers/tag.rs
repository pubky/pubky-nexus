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
use crate::models::user::{PubkyId, UserCounts, UserTags};
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
    // Update user counts in the user
    UserCounts::update(&tagged_user_id, "tags", JsonAction::Increment(1)).await?;

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
    // DELETE FROM GRAPH
    let tag_details = TagUser::del_from_graph("7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso", "2Z1N9KW8ABXG0").await?;
    // CHOOSE THE EVENT TYPE
    match tag_details {
        Some(tagged) => {
            match tagged.1 {
                Some(post_id) => del_sync_post(user_id, &post_id, &tagged.0, &tag_id).await?,
                None => del_sync_user(user_id, &tagged.0, &tag_id).await?
            }
        },
        None => println!("EXIT")
    }

    //let (user_id, post_id) = TagUser::del_from_graph("bb3xoth8hijfn6z6zahkutb6cfea5fzzs67gtziud74iusr1whgo", "2Z1N9M5M80YG0").await?;
    //let (user_id, post_id) = TagUser::del_from_graph("7kbjzgcx3xygokesys6jso13tt9u5n995p9q54a1co7cai9ujcso", "2Z1N9KW8ABXG0").await?;
    Ok(())
}

async fn del_sync_user(
    tagger_id: PubkyId,
    tagged_id: &str,
    tag_id: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    println!("TAG USER DEL: {:?}", tagger_id);
    Ok(())
}

async fn del_sync_post(
    tagger_id: PubkyId,
    post_id: &str,
    author_id: &str,
    tag_id: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    println!("TAG POST DEL: {:?}", tagger_id);
    Ok(())
}
