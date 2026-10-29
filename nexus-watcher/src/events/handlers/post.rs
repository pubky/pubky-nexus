use crate::events::EventProcessorError;

use nexus_common::db::kv::ScoreAction;
use nexus_common::db::queries;
use nexus_common::db::queries::get::post_is_safe_to_delete;
use nexus_common::db::{exec_single_row, execute_graph_operation, OperationOutcome};
use nexus_common::models::notification::{Notification, PostChangedSource, PostChangedType};
use nexus_common::models::post::{
    collection_item_keys, sync_collected_edges, PostCounts, PostDetails, PostParent, PostParents,
    PostRelationships, PostStream,
};
use nexus_common::models::user::UserIngestor;
use pubky_social_specs::legacy_v0::PubkyAppPostKind;
use pubky_social_specs::PubkyId;
use tracing::{debug, Instrument};

use super::utils::{
    bounded, fail_on_permanent_ingest_error, post_kind, post_relationships_is_reply,
};
use crate::events::inputs::{PostInput, PostKind, Reference, ReferenceTarget, ResourceKey};

pub async fn sync_put(
    post: PostInput,
    key: ResourceKey,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    reject_unsupported_parent(&post)?;
    let post_details = post.clone().into_details(&key);
    let ResourceKey {
        owner_id: author_id,
        resource_id: post_id,
    } = key;
    sync_put_details(post, post_details, author_id, post_id, ingestor).await
}

/// Rejects a reply to a file, a profile or a `pubky.app` tag as
/// [`EventProcessorError::UnsupportedTarget`], never retried: no reply edge can ever point at
/// one. A reply to a URL or another app's object, its tags included, is indexed with no reply
/// edge.
fn reject_unsupported_parent(post: &PostInput) -> Result<(), EventProcessorError> {
    match &post.parent {
        Some(Reference {
            uri,
            target:
                target @ (ReferenceTarget::File(_)
                | ReferenceTarget::User(_)
                | ReferenceTarget::Tag { app: None, .. }),
        }) => Err(EventProcessorError::UnsupportedTarget(format!(
            "The replied resource {} is not a Post, instead is: {}",
            bounded(uri),
            bounded(&format!("{target:?}"))
        ))),
        _ => Ok(()),
    }
}

/// Rejects an edit whose reply parent or repost embed names a different post than the post's
/// edges, as [`EventProcessorError::PostParentChanged`], never retried. Both are set when the
/// post is created, so delete can read them back from the graph. Compares keys: a parent written
/// differently but naming the same post, or any parent that isn't a post, is no change.
fn reject_parent_change(
    named: &PostParents,
    stored: &PostParents,
) -> Result<(), EventProcessorError> {
    let (named, stored) = (parent_keys(named), parent_keys(stored));
    if named == stored {
        return Ok(());
    }
    Err(EventProcessorError::PostParentChanged(bounded(&format!(
        "The edit names (parent, embed) {named:?}, but the post was created with {stored:?}"
    ))))
}

/// The keys of the reply parent and the repost embed.
fn parent_keys(parents: &PostParents) -> (Option<&ResourceKey>, Option<&ResourceKey>) {
    (
        parents.replied.as_ref().map(|parent| &parent.key),
        parents.reposted.as_ref().map(|parent| &parent.key),
    )
}

/// [`sync_put`] with the details built by the caller, so [`del`] can write a
/// tombstone through the same edit path.
#[tracing::instrument(name = "post.put", skip_all, fields(user_id = %author_id, post_id = %post_id))]
async fn sync_put_details(
    post: PostInput,
    post_details: PostDetails,
    author_id: PubkyId,
    post_id: String,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    debug!("Indexing post");
    // We avoid indexing replies into global feed sorted sets
    let is_reply = post.parent.is_some();
    let is_collection = post.kind == PostKind::Collection;
    // PRE-INDEX operation, identify the post relationship. These parents carry the addresses as
    // the author wrote them: only their keys reach the graph, and the addresses only the
    // missing-dependency diagnostics.
    let named = post.parents();
    // `None` for a new post. An existing one keeps the parents it was created with.
    let stored = PostParents::get_from_graph(&author_id, &post_id).await?;
    if let Some(stored) = &stored {
        reject_parent_change(&named, stored)?;
    }

    let (replied_key, reposted_key) = parent_keys(&named);
    let existed = match post_details.put_to_graph(replied_key, reposted_key).await? {
        OperationOutcome::CreatedOrDeleted => false,
        OperationOutcome::Updated => true,
        OperationOutcome::MissingDependency => {
            let mut dependency_event_keys = Vec::new();
            if let Some(parent) = &named.replied {
                dependency_event_keys.push(parent.uri.clone());

                // Drop the reply (non-retryable) if the replied-to post's author is on a blacklisted HS.
                fail_on_permanent_ingest_error(
                    ingestor.maybe_ingest_user(&parent.key.owner_id).await,
                )?;
            }
            if let Some(parent) = &named.reposted {
                dependency_event_keys.push(parent.uri.clone());

                // Drop the repost (non-retryable) if the reposted post's author is on a blacklisted HS.
                fail_on_permanent_ingest_error(
                    ingestor.maybe_ingest_user(&parent.key.owner_id).await,
                )?;
            }
            if dependency_event_keys.is_empty() {
                dependency_event_keys.push(author_id.to_string());
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
                let was_collection = existing_details.kind == PubkyAppPostKind::Collection;
                let collection_toggled = was_collection != is_collection;
                // `is_different_than` ignores kind, so refresh on a kind-only edit too.
                let kind_changed = existing_details.kind != post_details.kind;
                // Before `sync_edit` overwrites the Redis details: a retry after a
                // Collection -> Short flip would then read Short on both sides, skip
                // this, and leave the old edges in the graph for good.
                if was_collection || is_collection {
                    let items = post.collected_posts();
                    sync_collected_edges(&author_id, &post_id, &items, Some(&post_details)).await?;
                }
                if existing_details.is_different_than(&post_details) || kind_changed {
                    // A lock- or kind-only toggle refreshes the cache but must not notify.
                    // A deletion must, even of a repost with no content to clear.
                    let deleted_toggled = existing_details.deleted != post_details.deleted;
                    let notify = existing_details.content_differs_from(&post_details)
                        || collection_toggled
                        || deleted_toggled;
                    sync_edit(
                        &post,
                        stored.unwrap_or_default().replied,
                        author_id.clone(),
                        post_id.clone(),
                        post_details,
                        ingestor,
                        notify,
                        existing_details.kind.clone(),
                    )
                    .await?;
                }
            }
            None => {
                // Partial-failure recovery: graph already had the post but Redis is
                // missing PostDetails. A previous sync_put attempt wrote the graph node
                // but failed before completing the index writes. Re-run idempotent
                // index writes only — scores/notifications are intentionally
                // skipped (prefer drift over duplicates).
                recover_post_index_state(&author_id, &post_id).await?;
            }
        }
        return Ok(());
    }

    // Read the parents back from the edges just written, so the relationships and the reply and
    // repost notifications carry each parent's stored address, the one a graph rebuild returns.
    let PostParents { replied, reposted } = PostParents::get_from_graph(&author_id, &post_id)
        .await?
        .unwrap_or_default();
    let mut post_relationships = PostRelationships {
        replied: replied.as_ref().map(|parent| parent.uri.clone()),
        reposted: reposted.as_ref().map(|parent| parent.uri.clone()),
        mentioned: Vec::new(),
    };

    // IMPORTANT: Handle the mentions before traverse the graph (reindex_post) for that post
    // Handle "MENTIONED" relationships
    put_mentioned_relationships(
        &author_id,
        &post_id,
        &post_details.uri,
        &post_details.content,
        &mut post_relationships,
        post_details.kind.clone(),
    )
    .await?;

    // We only consider the first mentioned (tagged) user, to mitigate DoS attacks against Nexus
    // whereby posts with many (inexistent) tagged PKs can cause Nexus to spend a lot of time trying to resolve them
    if let Some(mentioned_user_id) = &post_relationships.mentioned.first() {
        // Best-effort: failures (incl. a blacklisted HS) must not fail the post itself,
        // which is indexed regardless; the MENTIONED edge is simply not materialized.
        if let Err(e) = ingestor.maybe_ingest_user(mentioned_user_id).await {
            tracing::warn!("Failed to ingest user {mentioned_user_id}: {e}");
        }
    }

    ingest_collection_item_authors(&post, ingestor).await;
    if is_collection {
        let items = post.collected_posts();
        sync_collected_edges(&author_id, &post_id, &items, Some(&post_details)).await?;
    }

    // SAVE TO INDEX - PHASE 1, create the post counts
    // TODO: Use SCARD on a set for unique tag count to avoid race conditions in parallel processing
    async {
        if PostCounts::get_from_index(&author_id, &post_id)
            .await?
            .is_none()
        {
            PostCounts::default()
                .put_to_index(&author_id, &post_id, is_reply)
                .await?
        }
        Ok::<(), EventProcessorError>(())
    }
    .instrument(tracing::info_span!("index.write", phase = "post_counts"))
    .await?;

    // Use that index wrapper to add a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;

    // PHASE 2: Process POST REPLIES indexes
    if let Some(PostParent {
        uri: replied_uri_str,
        key:
            ResourceKey {
                owner_id: parent_author_id,
                resource_id: parent_post_id,
            },
    }) = replied
    {
        // Define the reply parent key to index the reply later
        reply_parent_post_key_wrapper =
            Some((parent_author_id.to_string(), parent_post_id.clone()));

        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];

        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.write", phase = "reply_parent");
            PostCounts::invalidate(parent_post_key_parts),
            async {
                // Replies must not enter POST_TOTAL_ENGAGEMENT — ZINCRBY
                // would create the member if absent.
                if !post_relationships_is_reply(&parent_author_id, &parent_post_id).await? {
                    PostStream::update_index_score(
                        &parent_author_id,
                        &parent_post_id,
                        ScoreAction::Increment(1.0),
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
            async {
                let parent_kind = post_kind(&parent_author_id, &parent_post_id).await?;
                Notification::new_post_reply(
                    &author_id,
                    &replied_uri_str,
                    &post_details.uri,
                    &parent_author_id,
                    parent_kind,
                )
                .await?;
                Ok::<(), EventProcessorError>(())
            }
        );

        indexing_results.0?;
        indexing_results.1?;
        indexing_results.2?;
        indexing_results.3?;
    }

    // PHASE 3: Process POST REPOSTS indexes
    if let Some(PostParent {
        uri: reposted_uri_str,
        key:
            ResourceKey {
                owner_id: parent_author_id,
                resource_id: parent_post_id,
            },
    }) = reposted
    {
        let parent_post_key_parts: &[&str; 2] = &[&parent_author_id, &parent_post_id];

        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.write", phase = "repost_parent");
            PostCounts::invalidate(parent_post_key_parts),
            async {
                // Replies must not enter POST_TOTAL_ENGAGEMENT — ZINCRBY
                // would create the member if absent.
                if !post_relationships_is_reply(&parent_author_id, &parent_post_id).await? {
                    PostStream::update_index_score(
                        &parent_author_id,
                        &parent_post_id,
                        ScoreAction::Increment(1.0),
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
            },
            async {
                let embed_kind = post_kind(&parent_author_id, &parent_post_id).await?;
                Notification::new_repost(
                    &author_id,
                    &reposted_uri_str,
                    &post_details.uri,
                    &parent_author_id,
                    embed_kind,
                )
                .await?;
                Ok::<(), EventProcessorError>(())
            }
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
    debug!("Recovering post index state from graph");

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

    // Invalidate the replies/reposts cache of any parent this post points at, so
    // the next read recomputes those counts from the now-durable graph edges.
    // Invalidate-only is sufficient and safe on missing keys. Run before the child
    // reindex below so a failure there still leaves the parent caches refreshed.
    if let Some(PostParents { replied, reposted }) =
        PostParents::get_from_graph(author_id, post_id).await?
    {
        for PostParent { key, .. } in [replied, reposted].into_iter().flatten() {
            PostCounts::invalidate(&[&key.owner_id, &key.resource_id]).await?;
        }
    }

    // Re-merge any MENTIONED graph edges that the original mention loop
    // didn't finish. Skips notifications (0 > N on retry).
    merge_mention_edges(author_id, post_id, &post_details.content).await?;

    // Same for COLLECTED edges; a non-collection also clears edges left by a kind flip.
    let items = stored_curated_items(author_id, post_id, &post_details);
    sync_collected_edges(author_id, post_id, &items, Some(&post_details)).await?;

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

/// `replied` is the post's stored reply parent: the edit can't change it, and the notification
/// links it by its stored address, not the one the edit wrote.
#[allow(clippy::too_many_arguments)]
async fn sync_edit(
    post: &PostInput,
    replied: Option<PostParent>,
    author_id: PubkyId,
    post_id: String,
    post_details: PostDetails,
    ingestor: &UserIngestor,
    notify: bool,
    was_kind: PubkyAppPostKind,
) -> Result<(), EventProcessorError> {
    // Refresh the cached details (always, even for a lock-only toggle).
    post_details.put_to_index(&author_id, None, true).await?;

    // Re-run on edits; `maybe_ingest_user` is a no-op for already-known users.
    ingest_collection_item_authors(post, ingestor).await;

    // A lock-only toggle refreshes the cache and re-runs ingestion above, but is
    // not a content edit, so it must not notify interactors.
    if !notify {
        return Ok(());
    }

    // Notifications
    // Determine the change type
    let change_type = if post_details.deleted {
        PostChangedType::Deleted
    } else {
        PostChangedType::Edited
    };

    // Deletes report the prior kind (the tombstone forges a Short); edits the new kind.
    let changed_kind = match change_type {
        PostChangedType::Deleted => was_kind,
        PostChangedType::Edited => post_details.kind.clone(),
    };

    // Send notifications to users who interacted with the post
    Notification::changed_post(
        &author_id,
        &post_id,
        &post_details.uri,
        &change_type,
        changed_kind.clone(),
    )
    .await?;

    // Handle "A reply to your post was edited/deleted"
    if let Some(parent) = replied {
        Notification::post_children_changed(
            &author_id,
            &parent.uri,
            &parent.key.owner_id,
            &post_details.uri,
            PostChangedSource::Reply,
            &change_type,
            changed_kind,
        )
        .await?;
    };

    Ok(())
}

/// Helper function to handle "MENTIONED" relationships on the post content
pub async fn put_mentioned_relationships(
    author_id: &PubkyId,
    post_id: &str,
    post_uri: &str,
    content: &str,
    relationships: &mut PostRelationships,
    post_kind: PubkyAppPostKind,
) -> Result<(), EventProcessorError> {
    // TODO Deprecate, drop support for pk: support in an upcoming release
    // Backwards compatibility: identify user references with "pk:" prefix
    put_mentioned_relationships_for_prefix(
        author_id,
        post_id,
        post_uri,
        content,
        relationships,
        "pk:",
        post_kind.clone(),
    )
    .await?;

    // Support new pubkey display: identify user references with "pubky" prefix
    put_mentioned_relationships_for_prefix(
        author_id,
        post_id,
        post_uri,
        content,
        relationships,
        "pubky",
        post_kind,
    )
    .await?;

    Ok(())
}

async fn put_mentioned_relationships_for_prefix(
    author_id: &PubkyId,
    post_id: &str,
    post_uri: &str,
    content: &str,
    relationships: &mut PostRelationships,
    prefix: &str,
    post_kind: PubkyAppPostKind,
) -> Result<(), EventProcessorError> {
    for pubky_id in find_mentioned_ids(content, prefix) {
        // Create the MENTIONED relationship in the graph
        let query = queries::put::create_mention_relationship(author_id, post_id, &pubky_id);
        exec_single_row(query).await?;

        let maybe_mentioned_id =
            Notification::new_mention(author_id, &pubky_id, post_uri, post_kind.clone()).await?;
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

/// The post keys a stored Collection curates, parsed from its content; none for any other kind.
///
/// Recovery only: it has no input, so it reads the envelope the graph holds, which may have been
/// written under an older spec. A PUT takes its items from [`PostInput::collected_posts`].
// TODO(specs-migration): the stored envelope is read with the v0 reader; a `social/v1`
// collection's items need their own read path before N3.
fn stored_curated_items(
    author_id: &PubkyId,
    post_id: &str,
    post_details: &PostDetails,
) -> Vec<(PubkyId, String)> {
    if post_details.kind != PubkyAppPostKind::Collection {
        return Vec::new();
    }
    collection_item_keys(&post_details.content)
        .inspect_err(|e| tracing::warn!("Collection {author_id}:{post_id} envelope malformed: {e}"))
        .unwrap_or_default()
}

/// Best-effort ingestion of the owner of every item a Collection lists. No-op for
/// non-Collection posts, whose item list is empty; an item that names no user is skipped,
/// and failures (blacklisted HS) are logged so the Collection is still indexed.
async fn ingest_collection_item_authors(post: &PostInput, ingestor: &UserIngestor) {
    for item in &post.collection_items {
        let Some(owner_id) = item.target.owner_id() else {
            tracing::debug!("Collection user ingestion: skipping item {}", item.uri);
            continue;
        };
        if let Err(e) = ingestor.maybe_ingest_user(owner_id).await {
            tracing::warn!("Failed to ingest collection item author {}: {e}", item.uri);
        }
    }
}

#[tracing::instrument(name = "post.del", skip_all, fields(user_id = %key.owner_id, post_id = %key.resource_id))]
pub async fn del(key: ResourceKey, ingestor: &UserIngestor) -> Result<(), EventProcessorError> {
    debug!("Deleting post");

    // The post's stored address: a tombstone keeps it, and a hard delete reports it.
    // A post missing from the graph has nothing to delete.
    let Some((PostDetails { uri, .. }, _)) =
        PostDetails::get_from_graph(&key.owner_id, &key.resource_id).await?
    else {
        return Err(EventProcessorError::SkipIndexing);
    };

    // Graph query to check if there is any edge at all to this post other than AUTHORED, is a reply or is a repost.
    let query = post_is_safe_to_delete(&key.owner_id, &key.resource_id);

    // If there is none other relationship (OperationOutcome::CreatedOrDeleted), we delete from graph and redis.
    // If there is any (OperationOutcome::Updated), we overwrite the post with a cleared tombstone
    // carrying `deleted = true`. The node survives so its edges stay intact.
    match execute_graph_operation(query).await? {
        OperationOutcome::CreatedOrDeleted => sync_del(key.owner_id, key.resource_id, uri).await?,
        OperationOutcome::Updated => {
            let PostParents { replied, reposted } =
                PostParents::get_from_graph(&key.owner_id, &key.resource_id)
                    .await?
                    .unwrap_or_default();
            let reference = |PostParent { uri, key }| Reference {
                uri,
                target: ReferenceTarget::Post(key),
            };

            // A cleared Short flagged `deleted`, still a reply or a repost if it was one already:
            // an edit can't change either.
            let tombstone = PostInput {
                uri,
                content: String::new(),
                parent: replied.map(reference),
                embed: reposted.map(reference),
                collection_items: Vec::new(),
                kind: PostKind::Short,
                attachments: Vec::new(),
                lock: None,
            };
            let tombstone_details = PostDetails {
                deleted: true,
                ..tombstone.clone().into_details(&key)
            };

            // The tombstone keeps the `parent` of a deleted reply, so re-PUT may
            // still ingest the parent's author; pass on ingestor to enforce the real blacklist.
            sync_put_details(
                tombstone,
                tombstone_details,
                key.owner_id,
                key.resource_id,
                ingestor,
            )
            .await?;
        }
        OperationOutcome::MissingDependency => return Err(EventProcessorError::SkipIndexing),
    };

    Ok(())
}

/// `deleted_uri` is the post's stored address, read by the caller before the graph delete below.
pub async fn sync_del(
    author_id: PubkyId,
    post_id: String,
    deleted_uri: String,
) -> Result<(), EventProcessorError> {
    // 1. The PostRelationships index entry is the gate.
    //    NOTE: deliberately NOT using `get_by_id`, which would re-populate the
    //    index from the graph and defeat the gate.
    let post_in_index = PostRelationships::get_from_index(&author_id, &post_id)
        .await?
        .is_some();

    // Recover the kind BEFORE removing the gate below. PostDetails is still present
    // (graph-delete runs last). Doing this after the gate deletion would let a
    // failed lookup strand the notifications on retry (gate gone, post_in_index false).
    let deleted_kind = if post_in_index {
        post_kind(&author_id, &post_id).await?
    } else {
        PubkyAppPostKind::Unknown
    };

    // The parents to clean up come from the graph, which still has the post because
    // the graph delete runs LAST. Read them before removing the gate, for the same
    // reason as the kind above.
    let PostParents { replied, reposted } = PostParents::get_from_graph(&author_id, &post_id)
        .await?
        .unwrap_or_default();

    // 2. Atomically commit the cleanup decision: remove the gate as the very
    //    first mutation. Subsequent retries will observe `post_in_index = false`
    //    and skip non-idempotent ops (scores, notifications).
    if post_in_index {
        PostRelationships::delete(&author_id, &post_id).await?;
    }

    // If the post is a reply, cannot delete from the main feeds
    // In the main feed, we just include the root posts and reposts
    // It could be a situation that relationship would not exist and we will treat the post as a not reply
    let is_reply = replied.is_some();

    // DELETE TO INDEX - PHASE 1, remove the post counts
    // Idempotent: JSON DEL + ZREM from engagement sorted set.
    PostCounts::delete(&author_id, &post_id, !is_reply)
        .instrument(tracing::info_span!("index.delete", phase = "post_counts"))
        .await?;

    // Use that index wrapper to delete a post reply
    let mut reply_parent_post_key_wrapper: Option<(String, String)> = None;
    // Parent post-count caches to invalidate AFTER the graph delete. Invalidating
    // before the node is gone lets a concurrent read recache the pre-delete count.
    let mut parents_to_invalidate: Vec<[String; 2]> = Vec::new();

    // PHASE 2: Process POST REPLIES indexes
    // Decrement counts for parent post if replied
    if let Some(PostParent {
        uri: replied_uri_str,
        key:
            ResourceKey {
                owner_id: parent_user_id,
                resource_id: parent_post_id,
            },
    }) = replied
    {
        reply_parent_post_key_wrapper = Some((parent_user_id.to_string(), parent_post_id.clone()));
        // Parent reply count changes; invalidated after the graph delete below.
        parents_to_invalidate.push([parent_user_id.to_string(), parent_post_id.clone()]);

        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.delete", phase = "reply_parent");
            async {
                // A decrement never creates a member, so a reply parent,
                // which has none in POST_TOTAL_ENGAGEMENT, is left alone.
                if post_in_index {
                    PostStream::update_index_score(
                        &parent_user_id,
                        &parent_post_id,
                        ScoreAction::Decrement(1.0),
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
            },
            // Notification "a reply to your post was deleted", guarded to
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
                        deleted_kind.clone(),
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
            }
        );

        indexing_results.0?;
        indexing_results.1?;
    }
    // PHASE 3: Process POST REPOSTED indexes
    // Decrement counts for resposted post if existed
    if let Some(PostParent {
        uri: reposted_uri_str,
        key:
            ResourceKey {
                owner_id: parent_user_id,
                resource_id: parent_post_id,
            },
    }) = reposted
    {
        // Parent repost count changes; invalidated after the graph delete below.
        parents_to_invalidate.push([parent_user_id.to_string(), parent_post_id.clone()]);

        let indexing_results = nexus_common::traced_join!(
            tracing::info_span!("index.delete", phase = "repost_parent");
            async {
                // A decrement never creates a member, so a reply parent,
                // which has none in POST_TOTAL_ENGAGEMENT, is left alone.
                if post_in_index {
                    PostStream::update_index_score(
                        &parent_user_id,
                        &parent_post_id,
                        ScoreAction::Decrement(1.0),
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
            },
            // Notification "a repost of your post was deleted", guarded.
            async {
                if post_in_index {
                    Notification::post_children_changed(
                        &author_id,
                        &reposted_uri_str,
                        &parent_user_id,
                        &deleted_uri,
                        PostChangedSource::Repost,
                        &PostChangedType::Deleted,
                        deleted_kind.clone(),
                    )
                    .await?;
                }
                Ok::<(), EventProcessorError>(())
            }
        );

        indexing_results.0?;
        indexing_results.1?;
    }

    // PHASE 4: Final Redis cleanup of PostDetails (idempotent JSON DEL + ZREM).
    PostDetails::delete_from_index(&author_id, &post_id, reply_parent_post_key_wrapper)
        .instrument(tracing::info_span!("index.delete", phase = "post_details"))
        .await?;

    // PHASE 5: Graph deletion LAST — survives until all Redis cleanup completes,
    // so a partial failure leaves the graph node available for retry to re-enter
    // `post::del` -> `CreatedOrDeleted` -> `sync_del`.
    // The COLLECTED edges go first, unconditionally: DETACH DELETE would drop
    // them silently and the items' cached counts would keep this post, and on a
    // retry the kind is no longer recoverable from the index.
    sync_collected_edges(&author_id, &post_id, &[], None).await?;
    exec_single_row(queries::del::delete_post(&author_id, &post_id))
        .instrument(tracing::info_span!("graph.delete", phase = "post_graph"))
        .await?;

    // Now that the graph reflects the deletion, invalidate the parent post-count
    // caches so the next read recomputes the new count instead of the old one.
    for parent in &parents_to_invalidate {
        PostCounts::invalidate(&[&parent[0], &parent[1]]).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    fn author() -> PubkyId {
        PubkyId::try_from(AUTHOR).unwrap()
    }

    fn key(resource_id: &str) -> ResourceKey {
        ResourceKey {
            owner_id: author(),
            resource_id: resource_id.to_string(),
        }
    }

    fn parent(post_id: &str, uri: &str) -> Option<PostParent> {
        Some(PostParent {
            uri: uri.to_string(),
            key: key(post_id),
        })
    }

    /// A Short whose parent, if any, names `target`.
    fn reply_to(target: Option<ReferenceTarget>) -> PostInput {
        PostInput {
            uri: String::new(),
            content: String::new(),
            kind: PostKind::Short,
            parent: target.map(|target| Reference {
                uri: String::new(),
                target,
            }),
            embed: None,
            collection_items: Vec::new(),
            attachments: Vec::new(),
            lock: None,
        }
    }

    #[test]
    fn reply_to_a_post_or_another_apps_object_passes() {
        for target in [
            None,
            Some(ReferenceTarget::Post(key("003286NSMY490"))),
            Some(ReferenceTarget::Other),
            Some(ReferenceTarget::Tag {
                key: key("8Z8CWH8NVYQY39ZEBFGKQWWEKG"),
                app: Some("mapky".to_string()),
            }),
        ] {
            assert!(
                reject_unsupported_parent(&reply_to(target.clone())).is_ok(),
                "{target:?}"
            );
        }
    }

    #[test]
    fn reply_to_a_file_profile_or_pubky_app_tag_is_rejected() {
        for target in [
            ReferenceTarget::File(key("0033SSE3B1FQ0")),
            ReferenceTarget::User(author()),
            ReferenceTarget::Tag {
                key: key("8Z8CWH8NVYQY39ZEBFGKQWWEKG"),
                app: None,
            },
        ] {
            assert!(
                matches!(
                    reject_unsupported_parent(&reply_to(Some(target.clone()))),
                    Err(EventProcessorError::UnsupportedTarget(_))
                ),
                "{target:?}"
            );
        }
    }

    fn parents(replied: Option<PostParent>, reposted: Option<PostParent>) -> PostParents {
        PostParents { replied, reposted }
    }

    #[test]
    fn same_parents_by_key_pass_however_written() {
        let stored = parents(
            parent("003286NSMY490", "stored"),
            parent("003286NSMY491", "s"),
        );
        let named = parents(
            parent("003286NSMY490", "written/extra"),
            parent("003286NSMY491", "w"),
        );
        assert!(reject_parent_change(&named, &stored).is_ok());
        assert!(reject_parent_change(&parents(None, None), &parents(None, None)).is_ok());
    }

    #[test]
    fn added_removed_or_moved_parent_is_rejected() {
        let p1 = || parent("003286NSMY490", "p1");
        let p2 = || parent("003286NSMY491", "p2");
        let cases = [
            (
                "root edited to reply",
                parents(p1(), None),
                parents(None, None),
            ),
            (
                "root edited to repost",
                parents(None, p1()),
                parents(None, None),
            ),
            ("reply moved", parents(p2(), None), parents(p1(), None)),
            ("reply dropped", parents(None, None), parents(p1(), None)),
            ("repost dropped", parents(None, None), parents(None, p1())),
        ];
        for (case, named, stored) in cases {
            assert!(
                matches!(
                    reject_parent_change(&named, &stored),
                    Err(EventProcessorError::PostParentChanged(_))
                ),
                "{case}"
            );
        }
    }

    /// The v0 parser never length-checks a post id, so the message that quotes the named
    /// parent stays bounded whatever the author wrote after `posts/`.
    #[test]
    fn parent_change_message_is_bounded_for_a_long_post_id() {
        let long_id = "A".repeat(4096);
        let named = parents(parent(&long_id, "p1"), None);
        let stored = parents(None, None);
        let Err(EventProcessorError::PostParentChanged(message)) =
            reject_parent_change(&named, &stored)
        else {
            panic!("a reply added by an edit is rejected");
        };
        assert!(!message.contains(&long_id), "the long id is cut");
        assert_eq!(message.chars().count(), 257, "256 chars and the ellipsis");
        assert!(message.ends_with('…'));
    }
}
