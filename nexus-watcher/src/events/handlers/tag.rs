use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;
use crate::handle_indexing_results;
use chrono::Utc;
use nexus_common::db::kv::{JsonAction, ScoreAction};
use nexus_common::db::OperationOutcome;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::notification::Notification;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::post::{PostCounts, PostStream};
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::user::UserCounts;
use nexus_common::types::{DynError, Pagination};
use pubky_app_specs::{ParsedUri, PubkyAppTag, PubkyId, Resource};
use tracing::debug;

use super::utils::post_relationships_is_reply;

pub async fn sync_put(
    tag: PubkyAppTag,
    tagger_id: PubkyId,
    tag_id: String,
) -> Result<(), DynError> {
    debug!("Indexing new tag: {} -> {}", tagger_id, tag_id);

    // Parse the embeded URI to extract author_id and post_id using parse_tagged_post_uri
    let parsed_uri = ParsedUri::try_from(tag.uri.as_str())?;
    let user_id = parsed_uri.user_id;
    let indexed_at = Utc::now().timestamp_millis();

    match parsed_uri.resource {
        // If post_id is in the tagged URI, we place tag to a post.
        Resource::Post(post_id) => {
            // Place the tag on post
            put_sync_post(
                tagger_id, user_id, &post_id, &tag_id, &tag.label, &tag.uri, indexed_at,
            )
            .await
        }
        // If no post_id in the tagged URI, we place tag to a user.
        Resource::User => put_sync_user(tagger_id, user_id, &tag_id, &tag.label, indexed_at).await,
        other => {
            Err(format!("The tagged resource is not Post or User, instead is: {other:?}").into())
        }
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
    post_id: &str,
    tag_id: &str,
    tag_label: &str,
    post_uri: &str,
    indexed_at: i64,
) -> Result<(), DynError> {
    match TagPost::put_to_graph(
        &tagger_user_id,
        &author_id,
        Some(post_id),
        tag_id,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            // Ensure that dependencies follow the same format as the RetryManager keys
            let dependency = vec![format!("{author_id}:posts:{post_id}")];
            if let Ok(referenced_post_uri) = ParsedUri::try_from(post_uri) {
                if let Err(e) = Homeserver::maybe_ingest_for_post(&referenced_post_uri).await {
                    tracing::error!("Failed to ingest homeserver: {e}");
                }
            }
            Err(EventProcessorError::MissingDependency { dependency }.into())
        }
        OperationOutcome::CreatedOrDeleted => {
            // SAVE TO INDEXES
            let post_key_slice: &[&str] = &[&author_id, post_id];
            let tag_label_slice = &[tag_label.to_string()];

            let indexing_results = tokio::join!(
                // Update user counts for tagger
                UserCounts::update(&tagger_user_id, "tagged", JsonAction::Increment(1), None),
                // Increment in one the post tags
                PostCounts::update_index_field(
                    post_key_slice,
                    "tags",
                    JsonAction::Increment(1),
                    None
                ),
                async {
                    // Increase unique_tags if the tag does not exist already
                    // NOTE: To update that field, it cannot exist in TagPost SORTED SET the tag. Thats why it has to be executed
                    // before TagPost operation
                    PostCounts::update_index_field(
                        post_key_slice,
                        "unique_tags",
                        JsonAction::Increment(1),
                        Some(tag_label),
                    )
                    .await?;
                    // Increment the label count to post
                    TagPost::update_index_score(
                        &author_id,
                        Some(post_id),
                        tag_label,
                        ScoreAction::Increment(1.0),
                    )
                    .await?;
                    Ok::<(), DynError>(())
                },
                // Add user tag in post
                TagPost::add_tagger_to_index(&author_id, Some(post_id), &tagger_user_id, tag_label),
                // Add post to label total engagement
                PostsByTagSearch::update_index_score(
                    &author_id,
                    post_id,
                    tag_label,
                    ScoreAction::Increment(1.0)
                ),
                async {
                    // Post replies cannot be included in the total engagement index once they have been tagged
                    if !post_relationships_is_reply(&author_id, post_id).await? {
                        // Increment in one post global engagement
                        PostStream::update_index_score(
                            &author_id,
                            post_id,
                            ScoreAction::Increment(1.0),
                        )
                        .await?;
                    }
                    Ok::<(), DynError>(())
                },
                // Add post to global label timeline
                PostsByTagSearch::put_to_index(&author_id, post_id, tag_label),
                // Save new notification
                Notification::new_post_tag(&tagger_user_id, &author_id, tag_label, post_uri),
                TagSearch::put_to_index(tag_label_slice)
            );

            handle_indexing_results!(
                indexing_results.0,
                indexing_results.1,
                indexing_results.2,
                indexing_results.3,
                indexing_results.4,
                indexing_results.5,
                indexing_results.6,
                indexing_results.7,
                indexing_results.8
            );

            Ok(())
        }
    }
}

/// Handles the synchronization of a tagged user by updating the graph, indexes, and related counts.
///
/// # Arguments
/// - `tagger_user_id` - The `PubkyId` of the user tagging the user.
/// - `tagged_user_id` - The `PubkyId` of the user being tagged.
/// - `tag_id` - A `String` representing the unique identifier of the tag.
/// - `tag_label` - A `String` representing the label of the tag.
/// - `indexed_at` - A 64-bit integer representing the timestamp when the user was indexed.
async fn put_sync_user(
    tagger_user_id: PubkyId,
    tagged_user_id: PubkyId,
    tag_id: &str,
    tag_label: &str,
    indexed_at: i64,
) -> Result<(), DynError> {
    match TagUser::put_to_graph(
        &tagger_user_id,
        &tagged_user_id,
        None,
        tag_id,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => Ok(()),
        OperationOutcome::MissingDependency => {
            if let Err(e) = Homeserver::maybe_ingest_for_user(tagged_user_id.as_str()).await {
                tracing::error!("Failed to ingest homeserver: {e}");
            }

            let key = RetryEvent::generate_index_key_from_uri(&tagged_user_id.to_uri());
            let dependency = vec![key];
            Err(EventProcessorError::MissingDependency { dependency }.into())
        }
        OperationOutcome::CreatedOrDeleted => {
            let tag_label_slice = &[tag_label.to_string()];

            // SAVE TO INDEX
            let indexing_results = tokio::join!(
                // Update user counts for the tagged user
                UserCounts::update(&tagged_user_id, "tags", JsonAction::Increment(1), None),
                // Update user counts for the tagger user
                UserCounts::update(&tagger_user_id, "tagged", JsonAction::Increment(1), None),
                async {
                    // Increase unique_tags if the tag does not exist already
                    // NOTE: To update that field, it cannot exist in TagUser SORTED SET the tag. Thats why it has to be executed
                    // before TagUser operation
                    UserCounts::update(
                        &tagged_user_id,
                        "unique_tags",
                        JsonAction::Increment(1),
                        Some(tag_label),
                    )
                    .await?;
                    // Add label count to the user profile tag
                    TagUser::update_index_score(
                        &tagged_user_id,
                        None,
                        tag_label,
                        ScoreAction::Increment(1.0),
                    )
                    .await?;
                    Ok::<(), DynError>(())
                },
                // Add tagger to the user taggers list
                TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, tag_label),
                // Save new notification
                Notification::new_user_tag(&tagger_user_id, &tagged_user_id, tag_label),
                TagSearch::put_to_index(tag_label_slice)
            );

            handle_indexing_results!(
                indexing_results.0,
                indexing_results.1,
                indexing_results.2,
                indexing_results.3,
                indexing_results.4,
                indexing_results.5
            );

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
    let indexing_results = tokio::join!(
        // Update user counts in the tagged
        UserCounts::update(tagged_id, "tags", JsonAction::Decrement(1), None),
        // Update user counts in the tagger
        UserCounts::update(&tagger_id, "tagged", JsonAction::Decrement(1), None),
        async {
            // Decrement label count to the user profile tag
            TagUser::update_index_score(tagged_id, None, tag_label, ScoreAction::Decrement(1.0))
                .await?;
            // Decrease unique_tags
            // NOTE: To update that field, we first need to decrement the value in the TagUser SORTED SET associated with that tag
            UserCounts::update(
                tagged_id,
                "unique_tags",
                JsonAction::Decrement(1),
                Some(tag_label),
            )
            .await?;
            Ok::<(), DynError>(())
        },
        async {
            // Remove tagger to the user taggers list
            TagUser(vec![tagger_id.to_string()])
                .del_from_index(tagged_id, None, tag_label)
                .await?;
            Ok::<(), DynError>(())
        }
    );

    handle_indexing_results!(
        indexing_results.0,
        indexing_results.1,
        indexing_results.2,
        indexing_results.3
    );

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

    let indexing_results = tokio::join!(
        // Update user counts for tagger
        UserCounts::update(&tagger_id, "tagged", JsonAction::Decrement(1), None),
        // Decrement in one the post tags
        PostCounts::update_index_field(post_key_slice, "tags", JsonAction::Decrement(1), None),
        async {
            // Decrement label score in the post
            TagPost::update_index_score(
                author_id,
                Some(post_id),
                tag_label,
                ScoreAction::Decrement(1.0),
            )
            .await?;
            // Decrease unique_tag
            // NOTE: To update that field, we first need to decrement the value in the SORTED SET associated with that tag
            PostCounts::update_index_field(
                post_key_slice,
                "unique_tags",
                JsonAction::Decrement(1),
                Some(tag_label),
            )
            .await?;
            Ok::<(), DynError>(())
        },
        // Decrease post from label total engagement
        PostsByTagSearch::update_index_score(
            author_id,
            post_id,
            tag_label,
            ScoreAction::Decrement(1.0)
        ),
        async {
            // Post replies cannot be included in the total engagement index once the tag have been deleted
            if !post_relationships_is_reply(author_id, post_id).await? {
                // Decrement in one post global engagement
                PostStream::update_index_score(author_id, post_id, ScoreAction::Decrement(1.0))
                    .await?;
            }
            Ok::<(), DynError>(())
        },
        async {
            // Delete the tagger from the tag list
            tag_post
                .del_from_index(author_id, Some(post_id), tag_label)
                .await?;
            // NOTE: The tag search index, depends on the post taggers collection to delete
            // Delete post from global label timeline
            PostsByTagSearch::del_from_index(author_id, post_id, tag_label).await?;

            let posts_by_tag =
                PostsByTagSearch::get_by_label(tag_label, None, Pagination::default()).await?;
            let posts_by_tag_found = posts_by_tag.is_some_and(|x| !x.is_empty());
            if !posts_by_tag_found {
                // If we just removed the last post using this tag, remove tag from autocomplete suggestion list
                TagSearch::del_from_index(tag_label).await?;
            }

            Ok::<(), DynError>(())
        }
    );

    handle_indexing_results!(
        indexing_results.0,
        indexing_results.1,
        indexing_results.2,
        indexing_results.3,
        indexing_results.4,
        indexing_results.5
    );

    Ok(())
}
