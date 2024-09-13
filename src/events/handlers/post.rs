use crate::db::graph::exec::exec_single_row;
use crate::events::uri::ParsedUri;
use crate::models::pubky_app::traits::Validatable;
use crate::models::{post::PostDetails, pubky_app::PubkyAppPost, user::PubkyId};
use crate::queries;
use crate::reindex::{ingest_post, reindex_post};
use axum::body::Bytes;
use log::debug;
use std::error::Error;

pub async fn put(
    author_id: PubkyId,
    post_id: String,
    blob: Bytes,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Process Post resource and update the databases
    debug!("Indexing new post: {}/{}", author_id, post_id);

    // Serialize and validate
    let post = <PubkyAppPost as Validatable>::try_from(&blob).await?;

    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post.clone(), &author_id, &post_id).await?;

    // SAVE TO GRAPH
    // Add new post node into the graph
    post_details.put_to_graph().await?;

    let mut interaction: Vec<(&str, ParsedUri)> = Vec::new();

    // Handle "REPLIED" relationship and counts if `parent` is Some
    if let Some(parent_uri) = &post.parent {
        put_reply_relationship(&author_id, &post_id, parent_uri).await?;
        let parsed_uri = ParsedUri::try_from(parent_uri.as_str())?;
        interaction.push(("replies", parsed_uri));
    }
    // Handle "REPOSTED" relationship and counts if `embed.uri` is Some
    if let Some(embed) = &post.embed {
        put_repost_relationship(&author_id, &post_id, &embed.uri).await?;
        let parsed_uri = ParsedUri::try_from(embed.uri.as_str())?;
        interaction.push(("reposts", parsed_uri));
    }

    // SAVE TO INDEX
    // Reindex to sorted sets and other indexes
    reindex_post(&author_id, &post_id).await?;
    // Ingest the post data
    ingest_post(&author_id, interaction).await?;

    Ok(())
}

// Helper function to handle "REPLIED" relationship
async fn put_reply_relationship(
    author_id: &PubkyId,
    post_id: &str,
    parent_uri: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let parsed_uri = ParsedUri::try_from(parent_uri)?;
    if let (parent_author_id, Some(parent_post_id)) = (parsed_uri.user_id, parsed_uri.post_id) {
        exec_single_row(queries::write::create_reply_relationship(
            &author_id.0,
            post_id,
            &parent_author_id.0,
            &parent_post_id,
        ))
        .await?;
    }
    Ok(())
}

// Helper function to handle "REPOSTED" relationship
async fn put_repost_relationship(
    author_id: &PubkyId,
    post_id: &str,
    embed_uri: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let parsed_uri = ParsedUri::try_from(embed_uri)?;
    if let (reposted_author_id, Some(reposted_post_id)) = (parsed_uri.user_id, parsed_uri.post_id) {
        exec_single_row(queries::write::create_repost_relationship(
            &author_id.0,
            post_id,
            &reposted_author_id.0,
            &reposted_post_id,
        ))
        .await?;
    }
    Ok(())
}

pub async fn del(author_id: PubkyId, post_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    // TODO: handle deletion of Post resource from databases
    debug!("Deleting post: {}/{}", author_id, post_id);
    // Implement logic here
    Ok(())
}
