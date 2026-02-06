use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;
use crate::handle_indexing_results;
use nexus_common::db::queries::get::post_is_safe_to_delete;
use nexus_common::db::{exec_single_row, execute_graph_operation, OperationOutcome};
use nexus_common::db::{queries, RedisOps};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::notification::{Notification, PostChangedSource, PostChangedType};
use nexus_common::models::post::{
    PostCounts, PostDetails, PostRelationships, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
use nexus_common::models::user::UserCounts;
use nexus_common::types::DynError;
use pubky_app_specs::{
    post_uri_builder, ParsedUri, PubkyAppPost, PubkyAppPostKind, PubkyId, Resource,
};
use tracing::debug;

use super::utils::post_relationships_is_reply;

pub async fn sync_put(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
) -> Result<(), DynError> {
    debug!("Indexing new post: {}/{}", author_id, post_id);
    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post.clone(), &author_id, &post_id).await?;
    // We avoid indexing replies into global feed sorted sets
    let is_reply = post.parent.is_some();
    // PRE-INDEX operation, identify the post relationship
    let mut post_relationships = PostRelationships::from_homeserver(&post);

    let existed = match post_details.put_to_graph(&post_relationships).await? {
        OperationOutcome::CreatedOrDeleted => false,
        OperationOutcome::Updated => true,
        OperationOutcome::MissingDependency => {
            let mut dependency_event_keys = Vec::new();
            if let Some(replied_to_uri) = &post_relationships.replied {
                let reply_dependency = RetryEvent::generate_index_key_from_uri(replied_to_uri);
                dependency_event_keys.push(reply_dependency);

                if let Err(e) = Homeserver::maybe_ingest_for_post(replied_to_uri).await {
                    tracing::error!("Failed to ingest homeserver: {e}");
                }
            }
            if let Some(reposted_uri) = &post_relationships.reposted {
                let reply_dependency = RetryEvent::generate_index_key_from_uri(reposted_uri);
                dependency_event_keys.push(reply_dependency);

                if let Err(e) = Homeserver::maybe_ingest_for_post(reposted_uri).await {
                    tracing::error!("Failed to ingest homeserver: {e}");
                }
            }
            if dependency_event_keys.is_empty() {
                let key = RetryEvent::generate_index_key_from_uri(&author_id.to_uri());
                dependency_event_keys.push(key);
            }
            return Err(EventProcessorError::missing_dependencies(dependency_event_keys).into());
        }
    };

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

    // IMPORTANT: Handle the mentions before traverse the graph (reindex_post) for that post
    // Handle "MENTIONED" relationships
    put_mentioned_relationships(
        &author_id,
        &post_id,
        &post_details.content,
        &mut post_relationships,
    )
    .await?;

    // We only consider the first mentioned (tagged) user, to mitigate DoS attacks against Nexus
    // whereby posts with many (inexistent) tagged PKs can cause Nexus to spend a lot of time trying to resolve them
    if let Some(mentioned_user_id) = &post_relationships.mentioned.first() {
        if let Err(e) = Homeserver::maybe_ingest_for_user(mentioned_user_id).await {
            tracing::error!("Failed to ingest homeserver: {e}");
        }
    }

    // SAVE TO INDEX - PHASE 1, update post counts
    let indexing_results = tokio::join!(
        // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
        async {
            // Create post counts index
            // If new post (no existing counts) save a new PostCounts.
            if PostCounts::get_from_index(&author_id, &post_id)
                .await?
                .is_none()
            {
                PostCounts::default()
                    .put_to_index(&author_id, &post_id, is_reply)
                    .await?
            }
            Ok::<(), DynError>(())
        },
        // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
        // Update user counts with the new post
        UserCounts::increment(&author_id, "posts", None),
        async {
            if is_reply {
                UserCounts::increment(&author_id, "replies", None).await?;
            };
            Ok::<(), DynError>(())
        }
    );

    handle_indexing_results!(indexing_results.0, indexing_results.1, indexing_results.2);

    // Use that index wrapper to add a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;

    // PHASE 2: Process POST REPLIES indexes
    if let Some(replied_uri) = &post_relationships.replied {
        let parent_author_id = replied_uri.user_id.clone();
        let parent_post_id = match replied_uri.resource.clone() {
            Resource::Post(id) => id,
            _ => return Err("Replied URI is not a Post resource".into()),
        };
        let replied_uri_str = replied_uri.try_to_uri_str()?;

        // Define the reply parent key to index the reply later
        reply_parent_post_key_wrapper =
            Some((parent_author_id.to_string(), parent_post_id.clone()));

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];

        let indexing_results = tokio::join!(
            PostCounts::increment_index_field(parent_post_key_parts, "replies", None),
            async {
                if !post_relationships_is_reply(&parent_author_id, &parent_post_id).await? {
                    PostStream::increment_score_index_sorted_set(
                        &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                        parent_post_key_parts,
                    )
                    .await?;
                }
                Ok::<(), DynError>(())
            },
            PostStream::add_to_post_reply_sorted_set(
                parent_post_key_parts,
                &author_id,
                &post_id,
                post_details.indexed_at,
            ),
            Notification::new_post_reply(
                &author_id,
                &replied_uri_str,
                &post_details.uri,
                &parent_author_id,
            )
        );

        handle_indexing_results!(
            indexing_results.0,
            indexing_results.1,
            indexing_results.2.map_err(DynError::from),
            indexing_results.3
        );
    }

    // PHASE 3: Process POST REPOSTS indexes
    if let Some(reposted_uri) = &post_relationships.reposted {
        let parent_author_id = reposted_uri.user_id.clone();
        let parent_post_id = match reposted_uri.resource.clone() {
            Resource::Post(id) => id,
            _ => return Err("Reposted uri is not a Post resource".into()),
        };
        let reposted_uri_str = reposted_uri.try_to_uri_str()?;

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];
        let indexing_results = tokio::join!(
            PostCounts::increment_index_field(parent_post_key_parts, "reposts", None),
            async {
                // Post replies cannot be included in the total engagement index after they receive a reply
                if !post_relationships_is_reply(&parent_author_id, &parent_post_id).await? {
                    PostStream::increment_score_index_sorted_set(
                        &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                        parent_post_key_parts,
                    )
                    .await?;
                }
                Ok::<(), DynError>(())
            },
            Notification::new_repost(
                &author_id,
                &reposted_uri_str,
                &post_details.uri,
                &parent_author_id,
            )
        );

        handle_indexing_results!(indexing_results.0, indexing_results.1, indexing_results.2);
    }

    // PHASE 4: Add post related content
    let indexing_results = tokio::join!(
        post_relationships.put_to_index(&author_id, &post_id),
        post_details.put_to_index(&author_id, reply_parent_post_key_wrapper, false)
    );

    handle_indexing_results!(
        indexing_results.0.map_err(DynError::from),
        indexing_results.1.map_err(DynError::from)
    );

    Ok(())
}

async fn sync_edit(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
    post_details: PostDetails,
) -> Result<(), DynError> {
    // Construct the URI of the post that changed
    let changed_uri = post_uri_builder(author_id.to_string(), post_id.clone());

    // Update content of PostDetails!
    if let Err(e) = post_details.put_to_index(&author_id, None, true).await {
        return Err(EventProcessorError::IndexWriteFailed {
            message: format!("post edit failed - {:?}", e.to_string()),
        }
        .into());
    };

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

/// Helper function to handle "MENTIONED" relationships on the post content
pub async fn put_mentioned_relationships(
    author_id: &PubkyId,
    post_id: &str,
    content: &str,
    relationships: &mut PostRelationships,
) -> Result<(), DynError> {
    // TODO Deprecate, drop support for pk: support in an upcoming release
    // Backwards compatibility: identify user references with "pk:" prefix
    put_mentioned_relationships_for_prefix(author_id, post_id, content, relationships, "pk:")
        .await?;

    // Support new pubkey display: identify user references with "pubky" prefix
    put_mentioned_relationships_for_prefix(author_id, post_id, content, relationships, "pubky")
        .await?;

    Ok(())
}

async fn put_mentioned_relationships_for_prefix(
    author_id: &PubkyId,
    post_id: &str,
    content: &str,
    relationships: &mut PostRelationships,
    prefix: &str,
) -> Result<(), DynError> {
    let user_id_len = 52;

    let found_pubky_ids = content.match_indices(prefix).filter_map(|(start_idx, _)| {
        let user_id_start = start_idx + prefix.len();
        content
            .get(user_id_start..user_id_start + user_id_len)
            .and_then(|candidate| PubkyId::try_from(candidate).ok())
    });

    for pubky_id in found_pubky_ids {
        // Create the MENTIONED relationship in the graph
        let query = queries::put::create_mention_relationship(author_id, post_id, &pubky_id);
        exec_single_row(query).await?;

        let maybe_mentioned_id = Notification::new_mention(author_id, &pubky_id, post_id).await?;
        if let Some(mentioned_user_id) = maybe_mentioned_id {
            relationships.mentioned.push(mentioned_user_id);
        }
    }

    Ok(())
}

pub async fn del(author_id: PubkyId, post_id: String) -> Result<(), DynError> {
    debug!("Deleting post: {}/{}", author_id, post_id);

    // Graph query to check if there is any edge at all to this post other than AUTHORED, is a reply or is a repost.
    let query = post_is_safe_to_delete(&author_id, &post_id);

    // If there is none other relationship (OperationOutcome::CreatedOrDeleted), we delete from graph and redis.
    // But if there is any (OperationOutcome::Updated), then we simply update the post with keyword content [DELETED].
    // A deleted post is a post whose content is EXACTLY `"[DELETED]"`
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => sync_del(author_id, post_id).await?,
        OperationOutcome::Updated => {
            let existing_relationships = PostRelationships::get_by_id(&author_id, &post_id).await?;
            let parent = existing_relationships
                .and_then(|rel| rel.replied)
                .and_then(|replied_uri| replied_uri.try_to_uri_str().ok());

            // We store a dummy that is still a reply if it was one already.
            let dummy_deleted_post = PubkyAppPost {
                content: "[DELETED]".to_string(),
                parent,
                embed: None,
                kind: PubkyAppPostKind::Short,
                attachments: None,
            };

            sync_put(dummy_deleted_post, author_id, post_id).await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing.into()),
    };

    Ok(())
}

pub async fn sync_del(author_id: PubkyId, post_id: String) -> Result<(), DynError> {
    let deleted_uri = post_uri_builder(author_id.to_string(), post_id.clone());

    let post_relationships = PostRelationships::get_by_id(&author_id, &post_id).await?;
    // If the post is reply, cannot delete from the main feeds
    // In the main feed, we just include the root posts and reposts
    // It could be a situation that relationship would not exist and we will treat the post as a not reply
    let is_reply =
        matches!(&post_relationships, Some(relationship) if relationship.replied.is_some());

    // DELETE TO INDEX - PHASE 1, decrease post counts
    let indexing_results = tokio::join!(
        PostCounts::delete(&author_id, &post_id, !is_reply),
        UserCounts::decrement(&author_id, "posts", None),
        async {
            if is_reply {
                UserCounts::decrement(&author_id, "replies", None).await?;
            };
            Ok::<(), DynError>(())
        }
    );

    handle_indexing_results!(indexing_results.0, indexing_results.1, indexing_results.2);

    // Use that index wrapper to delete a post reply
    let mut reply_parent_post_key_wrapper: Option<[String; 2]> = None;

    if let Some(relationships) = post_relationships {
        // PHASE 2: Process POST REPLIES indexes
        // Decrement counts for parent post if replied
        if let Some(replied_uri) = relationships.replied {
            let parent_user_id = replied_uri.user_id.clone();
            let parent_post_id = match replied_uri.resource.clone() {
                Resource::Post(id) => id,
                _ => return Err("Replied uri is not a Post resource".into()),
            };
            let replied_uri_str = replied_uri.try_to_uri_str()?;

            let parent_post_key_parts: [&str; 2] = [&parent_user_id, &parent_post_id];
            reply_parent_post_key_wrapper =
                Some([parent_user_id.to_string(), parent_post_id.clone()]);

            let indexing_results = tokio::join!(
                PostCounts::decrement_index_field(&parent_post_key_parts, "replies", None),
                async {
                    // Post replies cannot be included in the total engagement index after the reply is deleted
                    if !post_relationships_is_reply(&parent_user_id, &parent_post_id).await? {
                        PostStream::decrement_score_index_sorted_set(
                            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                            &parent_post_key_parts,
                        )
                        .await?;
                    }
                    Ok::<(), DynError>(())
                },
                // Notification: "A reply to your post was deleted"
                Notification::post_children_changed(
                    &author_id,
                    &replied_uri_str,
                    &parent_user_id,
                    &deleted_uri,
                    PostChangedSource::Reply,
                    &PostChangedType::Deleted,
                )
            );

            handle_indexing_results!(indexing_results.0, indexing_results.1, indexing_results.2);
        }
        // PHASE 3: Process POST REPOSTED indexes
        // Decrement counts for resposted post if existed
        if let Some(reposted_uri) = relationships.reposted {
            let parent_post_id = match reposted_uri.resource.clone() {
                Resource::Post(id) => id,
                _ => return Err("Reposted uri is not a Post resource".into()),
            };
            let reposted_uri_str = reposted_uri.try_to_uri_str()?;

            let parent_post_key_parts: &[&str] = &[&reposted_uri.user_id, &parent_post_id];

            let indexing_results = tokio::join!(
                PostCounts::decrement_index_field(parent_post_key_parts, "reposts", None),
                async {
                    // Post replies cannot be included in the total engagement index after the repost is deleted
                    if !post_relationships_is_reply(&reposted_uri.user_id, &parent_post_id).await? {
                        PostStream::decrement_score_index_sorted_set(
                            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                            parent_post_key_parts,
                        )
                        .await?;
                    }
                    Ok::<(), DynError>(())
                },
                // Notification: "A repost of your post was deleted"
                Notification::post_children_changed(
                    &author_id,
                    &reposted_uri_str,
                    &reposted_uri.user_id,
                    &deleted_uri,
                    PostChangedSource::Repost,
                    &PostChangedType::Deleted,
                )
            );

            handle_indexing_results!(indexing_results.0, indexing_results.1, indexing_results.2);
        }
    }
    let indexing_results = tokio::join!(
        PostDetails::delete(&author_id, &post_id, reply_parent_post_key_wrapper),
        PostRelationships::delete(&author_id, &post_id)
    );

    handle_indexing_results!(indexing_results.0, indexing_results.1);

    Ok(())
}
