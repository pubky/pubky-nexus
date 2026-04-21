use crate::events::retry::event::RetryEvent;
use crate::events::EventProcessorError;

use nexus_common::db::queries::get::post_is_safe_to_delete;
use nexus_common::db::{exec_single_row, execute_graph_operation, OperationOutcome};
use nexus_common::db::{queries, RedisOps};
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::notification::{Notification, PostChangedSource, PostChangedType};
use nexus_common::models::post::{
    PostCounts, PostDetails, PostRelationships, PostStream, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
use nexus_common::models::user::UserCounts;
use pubky_app_specs::{
    post_uri_builder, ParsedUri, PubkyAppPost, PubkyAppPostKind, PubkyId, Resource,
};
use tracing::{debug, Instrument};

use super::utils::post_relationships_is_reply;

#[tracing::instrument(name = "post.put", skip_all, fields(user_id = %author_id, post_id = %post_id))]
pub async fn sync_put(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
) -> Result<(), EventProcessorError> {
    debug!("Indexing new post: {}/{}", author_id, post_id);
    // Create PostDetails object
    let post_details = PostDetails::from_homeserver(post.clone(), &author_id, &post_id);
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
            return Err(EventProcessorError::missing_dependencies(
                dependency_event_keys,
            ));
        }
    };

    if existed {
        // If the post existed, let's confirm this is an edit. Is the content different?
        match PostDetails::get_from_index(&author_id, &post_id).await? {
            Some(existing_details) => {
                if existing_details.content != post_details.content {
                    sync_edit(post, author_id, post_id, post_details).await?;
                }
            }
            None => {
                // Partial-failure recovery: graph already had the post but Redis is
                // missing PostDetails. A previous sync_put attempt wrote the graph node
                // but failed before completing the index writes. Re-run idempotent
                // index writes only — counters/scores/notifications are intentionally
                // skipped (prefer drift over duplicates).
                recover_post_index_state(&author_id, &post_id).await?;
            }
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
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.write", phase = "post_counts");
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
            Ok::<(), EventProcessorError>(())
        },
        // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
        // Update user counts with the new post
        UserCounts::increment(&author_id, "posts", None),
        async {
            if is_reply {
                UserCounts::increment(&author_id, "replies", None).await?;
            };
            Ok::<(), EventProcessorError>(())
        }
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;

    // Use that index wrapper to add a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;

    // PHASE 2: Process POST REPLIES indexes
    if let Some(replied_uri) = &post_relationships.replied {
        let parent_author_id = replied_uri.user_id.clone();
        let parent_post_id = match replied_uri.resource.clone() {
            Resource::Post(id) => id,
            _ => {
                return Err(EventProcessorError::generic(
                    "Replied URI is not a Post resource",
                ))
            }
        };
        let replied_uri_str = replied_uri
            .try_to_uri_str()
            .map_err(EventProcessorError::generic)?;

        // Define the reply parent key to index the reply later
        reply_parent_post_key_wrapper =
            Some((parent_author_id.to_string(), parent_post_id.clone()));

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];

        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.write", phase = "reply_parent");
            PostCounts::increment_index_field(parent_post_key_parts, "replies", None),
            async {
                if !post_relationships_is_reply(&parent_author_id, &parent_post_id).await? {
                    PostStream::increment_score_index_sorted_set(
                        &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                        parent_post_key_parts,
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
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

        indexing_results.0?;
        indexing_results.1?;
        indexing_results.2?;
        indexing_results.3?;
    }

    // PHASE 3: Process POST REPOSTS indexes
    if let Some(reposted_uri) = &post_relationships.reposted {
        let parent_author_id = reposted_uri.user_id.clone();
        let parent_post_id = match reposted_uri.resource.clone() {
            Resource::Post(id) => id,
            _ => {
                return Err(EventProcessorError::generic(
                    "Reposted uri is not a Post resource",
                ))
            }
        };
        let reposted_uri_str = reposted_uri
            .try_to_uri_str()
            .map_err(EventProcessorError::generic)?;

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];
        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.write", phase = "repost_parent");
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
                Ok::<(), EventProcessorError>(())
            },
            Notification::new_repost(
                &author_id,
                &reposted_uri_str,
                &post_details.uri,
                &parent_author_id,
            )
        );

        indexing_results.0?;
        indexing_results.1?;
        indexing_results.2?;
    }

    // PHASE 4: Add post related content
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.write", phase = "post_details");
        post_relationships.put_to_index(&author_id, &post_id),
        post_details.put_to_index(&author_id, reply_parent_post_key_wrapper, false)
    );

    indexing_results.0?;
    indexing_results.1?;

    Ok(())
}

/// Re-runs idempotent post index writes when a previous `sync_put` attempt
/// successfully wrote the graph node but failed before persisting the Redis
/// index entries. MENTIONED edges are also re-merged because the original
/// mention loop may have crashed mid-way; reindex reads from the graph, so
/// any gap would otherwise be unrecoverable.
///
/// Counters are recomputed from graph truth via the canonical `reindex`
/// functions — graph `post_counts` computes counts live from edges, so any
/// concurrent tag/bookmark/reply handler that also went through graph is
/// already reflected.
///
/// Notifications are intentionally NOT re-run (0 > N duplicates on retry).
async fn recover_post_index_state(
    author_id: &PubkyId,
    post_id: &str,
) -> Result<(), EventProcessorError> {
    debug!(
        "Recovering post index state from graph: {}/{}",
        author_id, post_id
    );

    // Fetch post details from the graph once — used both to drive mention
    // edge recovery (needs the content) and to re-populate the PostDetails
    // index below (avoids a second round-trip through `PostDetails::reindex`).
    let (post_details, reply) = PostDetails::get_from_graph(author_id, post_id)
        .await?
        .ok_or_else(|| {
            EventProcessorError::generic(
                "Post recovery: graph reported existing post but get_from_graph returned None",
            )
        })?;

    // Re-merge any MENTIONED graph edges that the original mention loop
    // didn't finish. Skips notifications (0 > N on retry).
    merge_mention_edges(author_id, post_id, &post_details.content).await?;

    // Reindex all Redis state from graph truth.
    let (details_result, relationships_result, counts_result) = nexus_common::traced_join!(
        tracing::info_span!("index.write", phase = "post_recovery");
        post_details.put_to_index(author_id, reply, false),
        PostRelationships::reindex(author_id, post_id),
        PostCounts::reindex(author_id, post_id)
    );

    details_result?;
    relationships_result?;
    counts_result?;
    Ok(())
}

async fn sync_edit(
    post: PubkyAppPost,
    author_id: PubkyId,
    post_id: String,
    post_details: PostDetails,
) -> Result<(), EventProcessorError> {
    // Construct the URI of the post that changed
    let changed_uri = post_uri_builder(author_id.to_string(), post_id.clone());

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
        let parsed_parent =
            ParsedUri::try_from(parent.as_str()).map_err(EventProcessorError::generic)?;
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
) -> Result<(), EventProcessorError> {
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
) -> Result<(), EventProcessorError> {
    for pubky_id in find_mentioned_ids(content, prefix) {
        // Create the MENTIONED relationship in the graph
        let query = queries::put::create_mention_relationship(author_id, post_id, &pubky_id);
        exec_single_row(query)
            .await
            .map_err(EventProcessorError::graph_query_failed)?;

        let maybe_mentioned_id = Notification::new_mention(author_id, &pubky_id, post_id).await?;
        if let Some(mentioned_user_id) = maybe_mentioned_id {
            relationships.mentioned.push(mentioned_user_id);
        }
    }

    Ok(())
}

fn find_mentioned_ids(content: &str, prefix: &str) -> Vec<PubkyId> {
    let user_id_len = 52;
    let mut seen = std::collections::HashSet::new();
    content
        .match_indices(prefix)
        .filter_map(|(start_idx, _)| {
            let user_id_start = start_idx + prefix.len();
            content
                .get(user_id_start..user_id_start + user_id_len)
                .and_then(|candidate| PubkyId::try_from(candidate).ok())
        })
        .filter(|id| seen.insert(id.to_string()))
        .collect()
}

/// Idempotent MERGE of every MENTIONED edge for the post. No notifications,
/// no Redis — safe to re-run from recovery.
async fn merge_mention_edges(
    author_id: &PubkyId,
    post_id: &str,
    content: &str,
) -> Result<(), EventProcessorError> {
    for prefix in ["pk:", "pubky"] {
        for pubky_id in find_mentioned_ids(content, prefix) {
            let query = queries::put::create_mention_relationship(author_id, post_id, &pubky_id);
            exec_single_row(query).await?
        }
    }
    Ok(())
}

#[tracing::instrument(name = "post.del", skip_all, fields(user_id = %author_id, post_id = %post_id))]
pub async fn del(author_id: PubkyId, post_id: String) -> Result<(), EventProcessorError> {
    debug!("Deleting post: {}/{}", author_id, post_id);

    // Graph query to check if there is any edge at all to this post other than AUTHORED, is a reply or is a repost.
    let query = post_is_safe_to_delete(&author_id, &post_id);

    // If there is none other relationship (OperationOutcome::CreatedOrDeleted), we delete from graph and redis.
    // But if there is any (OperationOutcome::Updated), then we simply update the post with keyword content [DELETED].
    // A deleted post is a post whose content is EXACTLY `"[DELETED]"`
    match execute_graph_operation(query)
        .await
        .map_err(EventProcessorError::graph_query_failed)?
    {
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
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    };

    Ok(())
}

pub async fn sync_del(author_id: PubkyId, post_id: String) -> Result<(), EventProcessorError> {
    let deleted_uri = post_uri_builder(author_id.to_string(), post_id.clone());

    // 1. Read PostRelationships from index — captures both the gate and the
    //    parent (replied/reposted) URIs needed for parent cleanup.
    //    NOTE: deliberately NOT using `get_by_id`, which would re-populate the
    //    index from the graph and defeat the gate.
    let mut post_relationships_opt =
        PostRelationships::get_from_index(&author_id, &post_id).await?;
    let post_in_index = post_relationships_opt.is_some();

    // 2. Atomically commit the cleanup decision: remove the gate as the very
    //    first mutation. Subsequent retries will observe `post_in_index = false`
    //    and skip non-idempotent ops (counters, scores, notifications).
    if post_in_index {
        PostRelationships::delete(&author_id, &post_id).await?;
    }

    // 3. On retry (gate already gone), fall back to the graph for parent info
    //    so that idempotent index cleanup can still target the right sorted sets.
    //    The graph is guaranteed to still have the post because the graph delete
    //    runs LAST.
    if post_relationships_opt.is_none() {
        post_relationships_opt = PostRelationships::get_from_graph(&author_id, &post_id).await?;
    }

    // If the post is a reply, cannot delete from the main feeds
    // In the main feed, we just include the root posts and reposts
    // It could be a situation that relationship would not exist and we will treat the post as a not reply
    let is_reply =
        matches!(&post_relationships_opt, Some(relationship) if relationship.replied.is_some());

    // DELETE TO INDEX - PHASE 1, decrease post counts
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete", phase = "post_counts");
        // Idempotent: JSON DEL + ZREM from engagement sorted set.
        PostCounts::delete(&author_id, &post_id, !is_reply),
        // Guarded: skip on retry to avoid double-decrement.
        async {
            if post_in_index {
                UserCounts::decrement(&author_id, "posts", None).await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        async {
            if post_in_index && is_reply {
                UserCounts::decrement(&author_id, "replies", None).await?;
            };
            Ok::<(), EventProcessorError>(())
        }
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;

    // Use that index wrapper to delete a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;

    if let Some(relationships) = post_relationships_opt {
        // PHASE 2: Process POST REPLIES indexes
        // Decrement counts for parent post if replied
        if let Some(replied_uri) = relationships.replied {
            let parent_user_id = replied_uri.user_id.clone();
            let parent_post_id = match replied_uri.resource.clone() {
                Resource::Post(id) => id,
                _ => {
                    return Err(EventProcessorError::generic(
                        "Replied uri is not a Post resource",
                    ))
                }
            };
            let replied_uri_str = replied_uri
                .try_to_uri_str()
                .map_err(EventProcessorError::generic)?;

            let parent_post_key_parts: [&str; 2] = [&parent_user_id, &parent_post_id];
            reply_parent_post_key_wrapper =
                Some((parent_user_id.to_string(), parent_post_id.clone()));

            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.delete", phase = "reply_parent");
                async {
                    if post_in_index {
                        PostCounts::decrement_index_field(&parent_post_key_parts, "replies", None).await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                async {
                    // Post replies cannot be included in the total engagement index after the reply is deleted
                    if post_in_index
                        && !post_relationships_is_reply(&parent_user_id, &parent_post_id).await?
                    {
                        PostStream::decrement_score_index_sorted_set(
                            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                            &parent_post_key_parts,
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                // Notification: "A reply to your post was deleted" — guarded to
                // prevent duplicate notifications on retry.
                async {
                    if post_in_index {
                        Notification::post_children_changed(
                            &author_id,
                            &replied_uri_str,
                            &parent_user_id,
                            &deleted_uri,
                            PostChangedSource::Reply,
                            &PostChangedType::Deleted,
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                }
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
        }
        // PHASE 3: Process POST REPOSTED indexes
        // Decrement counts for resposted post if existed
        if let Some(reposted_uri) = relationships.reposted {
            let parent_post_id = match reposted_uri.resource.clone() {
                Resource::Post(id) => id,
                _ => {
                    return Err(EventProcessorError::generic(
                        "Reposted uri is not a Post resource",
                    ))
                }
            };
            let reposted_uri_str = reposted_uri
                .try_to_uri_str()
                .map_err(EventProcessorError::generic)?;

            let parent_post_key_parts: &[&str] = &[&reposted_uri.user_id, &parent_post_id];

            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.delete", phase = "repost_parent");
                async {
                    if post_in_index {
                        PostCounts::decrement_index_field(parent_post_key_parts, "reposts", None).await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                async {
                    // Post replies cannot be included in the total engagement index after the repost is deleted
                    if post_in_index
                        && !post_relationships_is_reply(&reposted_uri.user_id, &parent_post_id).await?
                    {
                        PostStream::decrement_score_index_sorted_set(
                            &POST_TOTAL_ENGAGEMENT_KEY_PARTS,
                            parent_post_key_parts,
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                // Notification: "A repost of your post was deleted" — guarded.
                async {
                    if post_in_index {
                        Notification::post_children_changed(
                            &author_id,
                            &reposted_uri_str,
                            &reposted_uri.user_id,
                            &deleted_uri,
                            PostChangedSource::Repost,
                            &PostChangedType::Deleted,
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                }
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
        }
    }

    // PHASE 4: Final Redis cleanup of PostDetails (idempotent JSON DEL + ZREM).
    PostDetails::delete_from_index(&author_id, &post_id, reply_parent_post_key_wrapper)
        .instrument(tracing::info_span!("index.delete", phase = "post_details"))
        .await?;

    // PHASE 5: Graph deletion LAST — survives until all Redis cleanup completes,
    // so a partial failure leaves the graph node available for retry to re-enter
    // `post::del` -> `CreatedOrDeleted` -> `sync_del`.
    exec_single_row(queries::del::delete_post(&author_id, &post_id))
        .instrument(tracing::info_span!("graph.delete", phase = "post_graph"))
        .await?;

    Ok(())
}
