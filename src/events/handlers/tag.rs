use crate::db::graph::exec::exec_single_row;
use crate::events::uri::ParsedUri;
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PubkyAppTag;
use crate::models::user::PubkyId;
use crate::queries;
use crate::reindex::reindex_post_tags;
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

    reindex_post_tags(&user_id_slice, &author_id_slice, &post_id, &tag_label).await?;

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
