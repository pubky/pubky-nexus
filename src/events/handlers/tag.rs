use crate::db::graph::exec::OperationOutcome;
use crate::db::kv::index::json::JsonAction;
use crate::events::error::EventProcessorError;
use crate::models::notification::Notification;
use crate::models::post::{PostCounts, PostStream};
use crate::models::tag::post::TagPost;
use crate::models::tag::search::TagSearch;
use crate::models::tag::stream::Taggers;
use crate::models::tag::traits::{TagCollection, TaggersCollection};
use crate::models::tag::user::TagUser;
use crate::models::user::UserCounts;
use crate::types::DynError;
use crate::ScoreAction;
use chrono::Utc;
use log::debug;
use pubky_app_specs::Resource;
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId};

use super::utils::post_relationships_is_reply;

pub async fn sync_put(
    tag: PubkyAppTag,
    tagger_id: PubkyId,
    tag_id: String,
) -> Result<(), DynError> {
    debug!("Indexing new tag: {} -> {}", tagger_id, tag_id);

    // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
    let parsed_uri = ParsedUri::try_from(tag.uri.as_str())?;
    let indexed_at = Utc::now().timestamp_millis();

    match parsed_uri.resource {
        // If post_id is in the tagged URI, we place tag to a post.
        Resource::Post(post_id) => {
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
        Resource::User => {
            put_sync_user(tagger_id, parsed_uri.user_id, tag_id, tag.label, indexed_at).await
        }
        other => Err(format!(
            "The tagged resource is not Post or User resource. Tagged resource: {:?}",
            other
        )
        .into()),
    }
}

/// Handles the synchronization of a tagged post by updating the graph, indexes, and related counts.
/// # Arguments
/// - `tagger_user_id` - The `PubkyId` of the user tagging the post.
/// - `author_id` - The `PubkyId` of the author of the tagged post.
/// - `post_id` - A `String` representing the unique identifier of the post being tagged.
/// - `tag_id` - A `String` representing the unique identifier of the tag.
/// - `tag_label` - A `String` representing the label of the tag.
/// - `post_uri` - A `String` representing the homeserver URI of the tagged post.
/// - `indexed_at` - A 64-bit integer representing the timestamp when the post was indexed.
///
async fn put_sync_post(
    tagger_user_id: PubkyId,
    author_id: PubkyId,
    post_id: String,
    tag_id: String,
    tag_label: String,
    post_uri: String,
    indexed_at: i64,
) -> Result<(), DynError> {
    match TagPost::put_to_graph(
        &tagger_user_id,
        &author_id,
        Some(&post_id),
        &tag_id,
        &tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            // Ensure that dependencies follow the same format as the RetryManager keys
            let dependency = vec![format!("{author_id}:posts:{post_id}")];
            Err(EventProcessorError::MissingDependency { dependency }.into())
        }
        OperationOutcome::CreatedOrDeleted => {
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
                TagPost::add_tagger_to_index(
                    &author_id,
                    Some(&post_id),
                    &tagger_user_id,
                    &tag_label
                ),
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

            // Post replies cannot be included in the total engagement index once they have been tagged
            if !post_relationships_is_reply(&author_id, &post_id).await? {
                // Increment in one post global engagement
                PostStream::update_index_score(&author_id, &post_id, ScoreAction::Increment(1.0))
                    .await?;
            }

            // Add post to global label timeline
            TagSearch::put_to_index(&author_id, &post_id, &tag_label).await?;

            // Save new notification
            Notification::new_post_tag(&tagger_user_id, &author_id, &tag_label, &post_uri).await?;

            Ok(())
        }
    }
}

async fn put_sync_user(
    tagger_user_id: PubkyId,
    tagged_user_id: PubkyId,
    tag_id: String,
    tag_label: String,
    indexed_at: i64,
) -> Result<(), DynError> {
    match TagUser::put_to_graph(
        &tagger_user_id,
        &tagged_user_id,
        None,
        &tag_id,
        &tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            // Ensure that dependencies follow the same format as the RetryManager keys
            let dependency = vec![format!("{tagged_user_id}:user:profile.json")];
            Err(EventProcessorError::MissingDependency { dependency }.into())
        }
        OperationOutcome::CreatedOrDeleted => {
            // SAVE TO INDEX
            // Update user counts for the tagged user
            UserCounts::update(&tagged_user_id, "tagged", JsonAction::Increment(1)).await?;

            // Update user counts for the tagger user
            UserCounts::update(&tagger_user_id, "tags", JsonAction::Increment(1)).await?;

            // Add tagger to the user taggers list
            TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, &tag_label)
                .await?;

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
    }
}

pub async fn del(user_id: PubkyId, tag_id: String) -> Result<(), DynError> {
    debug!("Deleting tag: {} -> {}", user_id, tag_id);
    let tag_details = TagUser::del_from_graph(&user_id, &tag_id).await?;
    // CHOOSE THE EVENT TYPE
    if let Some((tagged_user_id, post_id, author_id, label)) = tag_details {
        match (tagged_user_id, post_id, author_id) {
            // Delete user related indexes
            (Some(tagged_id), None, None) => {
                del_sync_user(user_id, &tagged_id, &label).await?;
            }
            // Delete post related indexes
            (None, Some(post_id), Some(author_id)) => {
                del_sync_post(user_id, &post_id, &author_id, &label).await?;
            }
            // Handle other unexpected cases
            _ => {
                debug!("DEL-Tag: Unexpected combination of tag details");
            }
        }
    } else {
        return Err(EventProcessorError::SkipIndexing.into());
    }
    Ok(())
}

async fn del_sync_user(
    tagger_id: PubkyId,
    tagged_id: &str,
    tag_label: &str,
) -> Result<(), DynError> {
    // Update user counts in the tagged
    UserCounts::update(tagged_id, "tagged", JsonAction::Decrement(1)).await?;

    // Update user counts in the tagger
    UserCounts::update(&tagger_id, "tags", JsonAction::Decrement(1)).await?;

    // Remove tagger to the user taggers list
    TagUser(vec![tagger_id.to_string()])
        .del_from_index(tagged_id, None, tag_label)
        .await?;

    // Decrement label count to the user profile tag
    TagUser::update_index_score(tagged_id, None, tag_label, ScoreAction::Decrement(1.0)).await?;
    Ok(())
}

async fn del_sync_post(
    tagger_id: PubkyId,
    post_id: &str,
    author_id: &str,
    tag_label: &str,
) -> Result<(), DynError> {
    // SAVE TO INDEXES
    let post_key_slice: &[&str] = &[author_id, post_id];
    let tag_post = TagPost(vec![tagger_id.to_string()]);
    let tagger = Taggers(vec![tagger_id.to_string()]);

    // TODO: Handle the errors
    let _ = tokio::join!(
        // Update user counts for tagger
        UserCounts::update(&tagger_id, "tags", JsonAction::Decrement(1)),
        // Decrement in one the post tags
        PostCounts::update_index_field(post_key_slice, "tags", JsonAction::Decrement(1)),
        // Decrement label score in the post
        TagPost::update_index_score(
            author_id,
            Some(post_id),
            tag_label,
            ScoreAction::Decrement(1.0)
        ),
        tag_post.del_from_index(author_id, Some(post_id), tag_label),
        // Decrease post from label total engagement
        TagSearch::update_index_score(author_id, post_id, tag_label, ScoreAction::Decrement(1.0)),
        // Decrease the score of hot tags
        Taggers::update_index_score(tag_label, ScoreAction::Decrement(1.0)),
        // Delete tagger from global post tags
        tagger.del_from_index(tag_label)
    );

    // Post replies cannot be included in the total engagement index once the tag have been deleted
    if !post_relationships_is_reply(author_id, post_id).await? {
        // Decrement in one post global engagement
        PostStream::update_index_score(author_id, post_id, ScoreAction::Decrement(1.0)).await?;
    }

    // Delete post from global label timeline
    TagSearch::del_from_index(author_id, post_id, tag_label).await?;

    Ok(())
}
