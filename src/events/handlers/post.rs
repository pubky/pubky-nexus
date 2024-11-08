use crate::db::graph::exec::{exec_boolean_row, exec_single_row};
use crate::db::kv::index::json::JsonAction;
use crate::events::uri::ParsedUri;
use crate::models::notification::{Notification, PostChangedSource, PostChangedType};
use crate::models::post::{
    PostCounts, PostRelationships, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
use crate::models::pubky_app::traits::Validatable;
use crate::models::pubky_app::PostKind;
use crate::models::user::UserCounts;
use crate::models::{post::PostDetails, pubky_app::PubkyAppPost};
use crate::queries::get::post_is_safe_to_delete;
use crate::types::PubkyId;
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
    let post = <PubkyAppPost as Validatable>::try_from(&blob, &post_id).await?;

    sync_put(post, author_id, post_id).await
}

pub async fn sync_put(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post.clone(), &author_id, &post_id).await?;

    // We avoid indexing replies into feed sorted sets
    let add_to_feeds = post.parent.is_none();

    // SAVE TO GRAPH
    let existed = post_details.put_to_graph().await?;

    if existed {
        // If the post existed, let's confirm this is an edit. Is the content different?
        let existing_details = PostDetails::get_from_index(&author_id, &post_id)
            .await?
            .ok_or("An existing post in graph, could not be retrieved from index")?;
        if existing_details.content != post_details.content {
            sync_edit(post, author_id, post_id, post_details).await?;
        }
        return Ok(());
    }

    // PRE-INDEX operations
    let interactions = resolve_post_type_interaction(&post, &author_id, &post_id).await?;
    // IMPORTANT: Handle the mentions before traverse the graph (reindex_post) for that post
    // Handle "MENTIONED" relationships
    let mentioned_users =
        put_mentioned_relationships(&author_id, &post_id, &post_details.content).await?;

    // SAVE TO INDEX
    // Create post counts index
    // If new post (no existing counts) save a new PostCounts.
    if PostCounts::get_from_index(&author_id, &post_id)
        .await?
        .is_none()
    {
        PostCounts::default()
            .put_to_index(&author_id, &post_id, add_to_feeds)
            .await?
    }
    // Update user counts with the new post
    UserCounts::update(&author_id, "posts", JsonAction::Increment(1)).await?;

    let mut interaction_url: (Option<String>, Option<String>) = (None, None);
    // Use that index wrapper to add a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;

    // Post creation from an interaction: REPLY or REPOST
    for (action, parent_uri) in interactions {
        let parsed_uri = ParsedUri::try_from(parent_uri)?;

        let parent_author_id = parsed_uri.user_id;
        let parent_post_id = parsed_uri.post_id.ok_or("Missing post ID")?;

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];

        PostCounts::update_index_field(parent_post_key_parts, action, JsonAction::Increment(1))
            .await?;

        PostStream::put_score_index_sorted_set(
            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
            parent_post_key_parts,
            ScoreAction::Increment(1.0),
        )
        .await?;

        if action == "replies" {
            // Populate the reply parent keys to after index the reply
            reply_parent_post_key_wrapper =
                Some((parent_author_id.to_string(), parent_post_id.clone()));

            PostStream::add_to_post_reply_sorted_set(
                parent_post_key_parts,
                &author_id,
                &post_id,
                post_details.indexed_at,
            )
            .await?;
            Notification::new_post_reply(
                &author_id,
                parent_uri,
                &post_details.uri,
                &parent_author_id,
            )
            .await?;
            interaction_url.0 = Some(String::from(parent_uri));
        } else {
            Notification::new_repost(&author_id, parent_uri, &post_details.uri, &parent_author_id)
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

    post_details
        .put_to_index(&author_id, reply_parent_post_key_wrapper, false)
        .await?;

    Ok(())
}

async fn sync_edit(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
    post_details: PostDetails,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    // Construct the URI of the post that changed
    let changed_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}");

    // Update content of PostDetails!
    post_details.put_to_index(&author_id, None, true).await?;

    // Notifications
    // Determine the change type
    let change_type = if post_details.content == *"[DELETED]" {
        PostChangedType::Deleted
    } else {
        PostChangedType::Edited
    };

    // Send notifications to users who interacted with the post
    Notification::changed_post(&author_id, &post_id, &changed_uri, &change_type).await?;

    // Handle "A reply to your post was edited/deleted"
    if let Some(parent) = post.parent {
        let parsed_parent = ParsedUri::try_from(parent.as_str())?;
        Notification::post_children_changed(
            &author_id,
            &parent,
            &parsed_parent.user_id,
            &changed_uri,
            PostChangedSource::Reply,
            &change_type,
        )
        .await?;
    };

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
        exec_single_row(queries::put::create_reply_relationship(
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
        exec_single_row(queries::put::create_repost_relationship(
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
                    queries::put::create_mention_relationship(author_id, post_id, &pubky_id);
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
    debug!("Deleting post: {}/{}", author_id, post_id);

    // Graph query to check if there is any edge at all to this post other than AUTHORED, is a reply or is a repost.
    // If there is none other relationship, we delete from graph and redis.
    // But if there is any, then we simply update the post with keyword content [DELETED].
    // A deleted post is a post whose content is EXACTLY `"[DELETED]"`
    let query = post_is_safe_to_delete(&author_id, &post_id);
    let delete_safe = exec_boolean_row(query).await?;

    match delete_safe {
        true => sync_del(author_id, post_id).await?,
        false => {
            let existing_relationships = PostRelationships::get_by_id(&author_id, &post_id).await?;
            let parent = match existing_relationships {
                Some(relationships) => relationships.replied,
                None => None,
            };

            // We store a dummy that is still a reply if it was one already.
            let dummy_deleted_post = PubkyAppPost {
                content: "[DELETED]".to_string(),
                parent,
                embed: None,
                kind: PostKind::Short,
                attachments: None,
            };

            sync_put(dummy_deleted_post, author_id, post_id).await?;
        }
    };

    // TODO: Notifications for deleted posts

    Ok(())
}

pub async fn sync_del(
    author_id: PubkyId,
    post_id: String,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let deleted_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{post_id}");

    let relationships = PostRelationships::get_by_id(&author_id, &post_id).await?;
    // If the post is reply or repost, cannot delete from the main feeds
    // In the main feed, we just include the root posts
    let remove_from_feeds = can_remove_post_from_feed(&relationships);

    PostCounts::delete(&author_id, &post_id, remove_from_feeds).await?;
    UserCounts::update(&author_id, "posts", JsonAction::Decrement(1)).await?;

    // Use that index wrapper to delete a post reply
    let mut reply_parent_post_key_wrapper: Option<[String; 2]> = None;

    if let Some(relationships) = relationships {
        // Decrement counts for resposted post if existed
        if let Some(reposted) = relationships.reposted {
            let parsed_uri = ParsedUri::try_from(reposted.as_str())?;
            let parent_post_id = parsed_uri.post_id.ok_or("Missing post ID")?;

            let parent_post_key_parts: &[&str] = &[&parsed_uri.user_id, &parent_post_id];

            PostCounts::update_index_field(
                parent_post_key_parts,
                "reposts",
                JsonAction::Decrement(1),
            )
            .await?;
            PostStream::put_score_index_sorted_set(
                &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                parent_post_key_parts,
                ScoreAction::Decrement(1.0),
            )
            .await?;

            // Notification: "A repost of your post was deleted"
            Notification::post_children_changed(
                &author_id,
                &reposted,
                &parsed_uri.user_id,
                &deleted_uri,
                PostChangedSource::Repost,
                &PostChangedType::Deleted,
            )
            .await?;
        }
        // Decrement counts for parent post if replied
        if let Some(replied) = relationships.replied {
            let parsed_uri = ParsedUri::try_from(replied.as_str())?;
            let parent_user_id = parsed_uri.user_id;
            let parent_post_id = parsed_uri.post_id.ok_or("Missing post ID")?;

            let parent_post_key_parts: [&str; 2] = [&parent_user_id, &parent_post_id];
            reply_parent_post_key_wrapper =
                Some([parent_user_id.to_string(), parent_post_id.clone()]);

            PostCounts::update_index_field(
                &parent_post_key_parts,
                "replies",
                JsonAction::Decrement(1),
            )
            .await?;
            PostStream::put_score_index_sorted_set(
                &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                &parent_post_key_parts,
                ScoreAction::Decrement(1.0),
            )
            .await?;

            // Notification: "A reply to your post was deleted"
            Notification::post_children_changed(
                &author_id,
                &replied,
                &parent_user_id,
                &deleted_uri,
                PostChangedSource::Reply,
                &PostChangedType::Deleted,
            )
            .await?;
        }
    }
    PostDetails::delete(&author_id, &post_id, reply_parent_post_key_wrapper).await?;
    PostRelationships::delete(&author_id, &post_id).await?;

    Ok(())
}

// The only posts that has a feed are root posts and reposts. Replies does not belong to that feeds
fn can_remove_post_from_feed(relationship: &Option<PostRelationships>) -> bool {
    matches!(relationship, Some(post_relationship) if post_relationship.replied.is_none()/*&& post_relationship.reposted.is_none()*/)
}
