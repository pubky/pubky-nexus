use crate::db::graph::exec::exec_single_row;
use crate::events::uri::ParsedUri;
use crate::models::notification::Notification;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppTag;
use crate::models::user::PubkyId;
use crate::queries;
use crate::reindex::{ingest_post_tag, ingest_user_tag};
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
    let tag = <PubkyAppTag as Validatable>::try_from(&blob).await?;

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
                tag.uri,
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
    post_uri: String,
    indexed_at: i64,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Save new post tag to graph
    let query = queries::write::create_post_tag(
        &user_id, &author_id, &post_id, &tag_id, &tag_label, indexed_at,
    );
    exec_single_row(query).await?;

    // Save new post tag to indices
    ingest_post_tag(&user_id, &author_id, &post_id, &tag_label).await?;

    // Save new notification
    Notification::new_post_tag(&user_id, &author_id, &tag_label, &post_uri).await?;

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

    // Save new user tag to indices
    ingest_user_tag(&user_id, &tagged_user_id, &tag_label).await?;

    // Save new notification
    Notification::new_user_tag(&user_id, &tagged_user_id, &tag_label).await?;

    Ok(())
}

pub async fn del(user_id: PubkyId, tag_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    debug!("Deleting tag: {} -> {}", user_id, tag_id);

    // Delete the tag relationship from the graph
    let query = queries::write::delete_tag(&user_id, &tag_id);
    exec_single_row(query).await?;

    Ok(())
}
