use crate::db::graph::exec::exec_single_row;
use crate::db::kv::index::json::JsonAction;
use crate::events::uri::ParsedUri;
use crate::models::notification::Notification;
use crate::models::post::{
    PostCounts, PostRelationships, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
use crate::models::pubky_app::traits::Validatable;
use crate::models::user::UserCounts;
use crate::models::{post::PostDetails, pubky_app::PubkyAppPost, user::PubkyId};
use crate::{queries, RedisOps, ScoreAction};
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

    sync_put(post, author_id, post_id).await
}

pub async fn sync_put(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post.clone(), &author_id, &post_id).await?;

    // SAVE TO GRAPH
    post_details.put_to_graph().await?;

    // PRE-INDEX operations
    let interactions = resolve_post_type_interaction(&post, &author_id, &post_id).await?;
    // IMPORTANT: Handle the mentions before traverse the graph (reindex_post) for that post
    // Handle "MENTIONED" relationships
    let mentioned_users =
        put_mentioned_relationships(&author_id, &post_id, &post_details.content).await?;

    // SAVE TO INDEX
    // Create post counts index
    PostCounts::default()
        .put_to_index(&author_id, &post_id)
        .await?;
    // Update user counts with the new post
    UserCounts::update(&author_id, "posts", JsonAction::Increment(1)).await?;

    let mut interaction_url: (Option<String>, Option<String>) = (None, None);

    // Post creation from an interaction: REPLY or REPOST
    for (action, parent_uri) in interactions {
        let parsed_uri = ParsedUri::try_from(parent_uri)?;
        let parent_post_key_parts: &[&str] = &[
            &parsed_uri.user_id,
            &parsed_uri.post_id.ok_or("Missing post ID")?,
        ];
        PostCounts::update_index_field(parent_post_key_parts, action, JsonAction::Increment(1))
            .await?;
        PostStream::put_score_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            parent_post_key_parts,
            ScoreAction::Increment(1.0),
        )
        .await?;

        if action == "replies" {
            Notification::new_post_reply(
                &author_id,
                parent_uri,
                &post_details.uri,
                &parsed_uri.user_id,
            )
            .await?;
            interaction_url.0 = Some(String::from(parent_uri));
        } else {
            Notification::new_repost(
                &author_id,
                parent_uri,
                &post_details.uri,
                &parsed_uri.user_id,
            )
            .await?;
            interaction_url.1 = Some(String::from(parent_uri));
        }
    }

    PostRelationships {
        replied: interaction_url.0,
        reposted: interaction_url.1,
        mentioned: mentioned_users,
    }
    .put_to_index(&author_id, &post_id)
    .await?;

    post_details.put_to_index(&author_id).await?;

    Ok(())
}

async fn resolve_post_type_interaction<'a>(
    post: &'a PubkyAppPost,
    author_id: &str,
    post_id: &str,
) -> Result<Vec<(&'a str, &'a str)>, Box<dyn Error + Sync + Send>> {
    let mut interaction: Vec<(&str, &str)> = Vec::new();

    // Handle "REPLIED" relationship and counts if `parent` is Some
    if let Some(parent_uri) = &post.parent {
        put_reply_relationship(author_id, post_id, parent_uri).await?;
        interaction.push(("replies", parent_uri.as_str()));
    }
    // Handle "REPOSTED" relationship and counts if `embed.uri` is Some
    if let Some(embed) = &post.embed {
        put_repost_relationship(author_id, post_id, &embed.uri).await?;
        interaction.push(("reposts", embed.uri.as_str()));
    }
    Ok(interaction)
}

// Helper function to handle "REPLIED" relationship
async fn put_reply_relationship(
    author_id: &str,
    post_id: &str,
    parent_uri: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let parsed_uri = ParsedUri::try_from(parent_uri)?;
    if let (parent_author_id, Some(parent_post_id)) = (parsed_uri.user_id, parsed_uri.post_id) {
        exec_single_row(queries::write::create_reply_relationship(
            author_id,
            post_id,
            &parent_author_id,
            &parent_post_id,
        ))
        .await?;
    }
    Ok(())
}

// Helper function to handle "REPOSTED" relationship
async fn put_repost_relationship(
    author_id: &str,
    post_id: &str,
    embed_uri: &str,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let parsed_uri = ParsedUri::try_from(embed_uri)?;
    if let (reposted_author_id, Some(reposted_post_id)) = (parsed_uri.user_id, parsed_uri.post_id) {
        exec_single_row(queries::write::create_repost_relationship(
            author_id,
            post_id,
            &reposted_author_id,
            &reposted_post_id,
        ))
        .await?;
    }
    Ok(())
}

// Helper function to handle "MENTIONED" relationships on the post content
pub async fn put_mentioned_relationships(
    author_id: &PubkyId,
    post_id: &str,
    content: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let prefix = "pk:";
    let user_id_len = 52;
    let mut mention_users = Vec::new();

    for (start_idx, _) in content.match_indices(prefix) {
        let user_id_start = start_idx + prefix.len();

        // Try to extract and validate the user_id_candidate
        if let Some(user_id_candidate) = content.get(user_id_start..user_id_start + user_id_len) {
            if let Ok(pubky_id) = PubkyId::try_from(user_id_candidate) {
                // Create the MENTIONED relationship in the graph
                let query =
                    queries::write::create_mention_relationship(author_id, post_id, &pubky_id);
                exec_single_row(query).await?;
                if let Some(mentioned_user_id) =
                    Notification::new_mention(author_id, &pubky_id, post_id).await?
                {
                    mention_users.push(mentioned_user_id);
                }
            }
        }
    }

    Ok(mention_users)
}

pub async fn del(author_id: PubkyId, post_id: String) -> Result<(), Box<dyn Error + Sync + Send>> {
    // TODO: handle deletion of Post resource from databases
    debug!("Deleting post: {}/{}", author_id, post_id);
    // Implement logic here
    Ok(())
}
