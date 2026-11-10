use crate::events::EventProcessorError;
use chrono::Utc;
use nexus_common::db::kv::ScoreAction;
use nexus_common::db::{
    fetch_key_from_graph, fetch_row_from_graph, queries, OperationOutcome, RedisOps,
};
use nexus_common::models::event::resource_changes::{ResourceChange, ResourceChangeOp};
use nexus_common::models::notification::Notification;
use nexus_common::models::post::search::PostsByTagSearch;
use nexus_common::models::post::{PostCounts, PostStream};
use nexus_common::models::resource::tag::TagResource;
use nexus_common::models::tag::post::TagPost;
use nexus_common::models::tag::search::TagSearch;
use nexus_common::models::tag::traits::{TagCollection, TaggersCollection};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::user::{UserIngestor, UsersByTagSearch};
use nexus_common::types::Pagination;
use nexus_common::universal_tag::normalize::{normalize_uri, resource_id};
use pubky_social_specs::PubkyId;
use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::LazyLock;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{debug, Instrument};

use super::utils::{
    bounded, fail_on_permanent_ingest_error, post_kind, post_relationships_is_reply,
};
use crate::events::inputs::{Reference, ReferenceTarget, ResourceKey, TagInput};

/// Locks shared by the handlers of resource tags: see [`lock_tag`].
const TAG_LOCK_STRIPES: usize = 1024;
static TAG_LOCKS: LazyLock<Vec<Mutex<()>>> =
    LazyLock::new(|| (0..TAG_LOCK_STRIPES).map(|_| Mutex::new(())).collect());

/// The lock of one tag file, keyed by its tagger and tag id, which every handler of the file
/// knows before it reads the graph. A resource tag's PUT, its DEL and a takedown of it hold it
/// across their graph write and their resource changes feed append, so the feed lists one file's
/// changes in the order the graph took them. The locks are a fixed set of stripes: two files
/// that share one only wait for each other, and nothing is kept per file. A crash releases it.
/// It orders the handlers of one watcher process.
pub async fn lock_tag(tagger_id: &str, tag_id: &str) -> MutexGuard<'static, ()> {
    TAG_LOCKS[tag_lock_stripe(tagger_id, tag_id)].lock().await
}

/// The locks of several tag files of one tagger, as the flip's tags step takes them for the
/// edges it moves, and a DEL for both ids of an edge the flip gave a new one: see
/// [`lock_tag`]. Each stripe is taken once, in ascending order, so files that share a stripe do
/// not wait for themselves, and two callers never wait for each other in a cycle: a caller
/// holds either one stripe, waiting for none, or stripes it took in this order.
pub async fn lock_tags<'a>(
    tagger_id: &str,
    tag_ids: impl IntoIterator<Item = &'a str>,
) -> Vec<MutexGuard<'static, ()>> {
    lock_tag_files(tag_ids.into_iter().map(|tag_id| (tagger_id, tag_id))).await
}

/// The locks of several tag files, of any taggers, `(tagger_id, tag_id)` pairs, as the
/// publisher of the flip's moves takes them for a page of edges: see [`lock_tags`], whose
/// order it keeps across taggers.
pub async fn lock_tag_files<T: AsRef<str>, I: AsRef<str>>(
    files: impl IntoIterator<Item = (T, I)>,
) -> Vec<MutexGuard<'static, ()>> {
    let mut stripes: Vec<usize> = files
        .into_iter()
        .map(|(tagger_id, tag_id)| tag_lock_stripe(tagger_id.as_ref(), tag_id.as_ref()))
        .collect();
    stripes.sort_unstable();
    stripes.dedup();
    let mut guards = Vec::with_capacity(stripes.len());
    for stripe in stripes {
        guards.push(TAG_LOCKS[stripe].lock().await);
    }
    guards
}

/// The id of the tag file at `tag_uri`, the last segment of its path, which its handlers
/// lock it by.
fn tag_id_of(tag_uri: &str) -> &str {
    let file = tag_uri.rsplit('/').next().unwrap_or(tag_uri);
    file.strip_suffix(".json").unwrap_or(file)
}

/// Which of the locks a tag file takes.
pub fn tag_lock_stripe(tagger_id: &str, tag_id: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    (tagger_id, tag_id).hash(&mut hasher);
    hasher.finish() as usize % TAG_LOCK_STRIPES
}

/// `tag.uri` is the address the tag is stored under, kept as the edge `uri`; `tag.tagged` is
/// what it tags. Only a post or a user can be tagged here; any other target is
/// [`EventProcessorError::UnsupportedTarget`].
#[tracing::instrument(
    name = "tag.put",
    skip_all,
    fields(user_id = %key.owner_id, tag_id = %key.resource_id)
)]
pub async fn sync_put(
    tag: TagInput,
    key: ResourceKey,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    debug!("Indexing tag");
    let ResourceKey {
        owner_id: tagger_id,
        resource_id: tag_id,
    } = key;
    let TagInput {
        uri: tag_uri,
        tagged: Reference {
            uri: tagged_uri,
            target,
        },
        label,
    } = tag;
    let indexed_at = Utc::now().timestamp_millis();

    match target {
        ReferenceTarget::Post(ResourceKey {
            owner_id: author_id,
            resource_id: post_id,
        }) => {
            put_sync_post(
                tagger_id,
                author_id,
                &post_id,
                &tag_id,
                &tag_uri,
                &label,
                &tagged_uri,
                indexed_at,
                ingestor,
            )
            .await
        }
        ReferenceTarget::User(user_id) => {
            put_sync_user(
                tagger_id, user_id, &tag_id, &tag_uri, &label, indexed_at, ingestor,
            )
            .await
        }
        other @ (ReferenceTarget::File(_)
        | ReferenceTarget::Tag { .. }
        | ReferenceTarget::Foreign
        | ReferenceTarget::Other) => Err(EventProcessorError::UnsupportedTarget(format!(
            "The tagged resource {} is not Post or User, instead is: {}",
            bounded(&tagged_uri),
            bounded(&format!("{other:?}"))
        ))),
    }
}

/// Indexes a tag on a foreign resource or a file (at either epoch's address) as a generic
/// `Resource` tag, keyed by the target's normalized address: a file is never waited for, and a
/// v0 file and its v1 copy are two targets.
#[tracing::instrument(
    name = "tag.put_resource",
    skip_all,
    fields(user_id = %key.owner_id, tag_id = %key.resource_id)
)]
pub async fn sync_put_resource(tag: TagInput, key: ResourceKey) -> Result<(), EventProcessorError> {
    debug!("Indexing resource tag");
    let _tag_lock = lock_tag(key.owner_id.as_ref(), &key.resource_id).await;
    let (normalized, scheme) =
        normalize_uri(&tag.tagged.uri).map_err(EventProcessorError::generic)?;

    put_sync_resource(
        key.owner_id,
        &resource_id(&normalized),
        &normalized,
        &scheme,
        &key.resource_id,
        &tag.uri,
        &tag.label,
        Utc::now().timestamp_millis(),
    )
    .await
}

/// Creates a Resource tag in the graph and adds its label to the tag search index.
#[allow(clippy::too_many_arguments)]
async fn put_sync_resource(
    tagger_id: PubkyId,
    resource_id: &str,
    uri: &str,
    scheme: &str,
    tag_id: &str,
    tag_uri: &str,
    tag_label: &str,
    indexed_at: i64,
) -> Result<(), EventProcessorError> {
    let outcome = TagResource::put_to_graph_resource(
        &tagger_id,
        resource_id,
        uri,
        scheme,
        tag_id,
        tag_uri,
        tag_label,
        indexed_at,
    )
    .await?;
    if !matches!(outcome, OperationOutcome::MissingDependency) {
        // A retry still publishes, so a graph commit followed by a Redis failure
        // cannot lose the reference. Consumers upsert by (author, object_path).
        // A file that lands on an edge created by another file (an equivalent tag,
        // or the other folder's copy of the same tag) publishes nothing: the edge and
        // its feed line keep the first file's path, and the delete retracts that one.
        let at = if matches!(outcome, OperationOutcome::Updated) {
            let row = fetch_row_from_graph(queries::get::get_resource_tag_indexed_at(
                &tagger_id,
                resource_id,
                tag_label,
            ))
            .await?
            .ok_or_else(|| EventProcessorError::generic("Resource tag disappeared during retry"))?;
            let owner: String = row.get("tag_uri").map_err(EventProcessorError::generic)?;
            (owner == tag_uri)
                .then(|| row.get("tag_indexed_at"))
                .transpose()
                .map_err(EventProcessorError::generic)?
        } else {
            Some(indexed_at)
        };
        if let Some(at) = at {
            ResourceChange::tag(
                ResourceChangeOp::Put,
                resource_id.to_owned(),
                uri.to_owned(),
                tagger_id.to_string(),
                tag_label.to_owned(),
                tag_uri,
                at,
            )?
            .store()
            .await?;
        }
    }
    match outcome {
        OperationOutcome::MissingDependency => {
            // Tagger user not yet indexed
            let dependency = vec![format!("{tagger_id}")];
            Err(EventProcessorError::MissingDependency { dependency })
        }
        // Updated is a retry finding the edge already written: the index write is
        // idempotent, so it runs again to recover from a partial failure
        OperationOutcome::CreatedOrDeleted | OperationOutcome::Updated => {
            TagSearch::put_to_index(&[tag_label.to_string()])
                .instrument(tracing::info_span!("index.write", phase = "tag_resource"))
                .await?;
            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn put_sync_post(
    tagger_user_id: PubkyId,
    author_id: PubkyId,
    post_id: &str,
    tag_id: &str,
    tag_uri: &str,
    tag_label: &str,
    post_uri_str: &str,
    indexed_at: i64,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    match TagPost::put_to_graph(
        &tagger_user_id,
        &author_id,
        Some(post_id),
        tag_id,
        tag_uri,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => {
            // Re-run idempotent ops to recover from partial failure (graph wrote, Redis didn't)
            let tag_label_slice = &[tag_label.to_string()];
            let post_key_slice: &[&str] = &[&author_id, post_id];
            let idempotent_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_post_retry");
                TagPost::add_tagger_to_index(&author_id, Some(post_id), &tagger_user_id, tag_label),
                PostsByTagSearch::put_to_index(&author_id, post_id, tag_label),
                TagSearch::put_to_index(tag_label_slice),
                // TAGGED edge already durable; invalidate-only so the next read
                // recomputes tags/unique_tags from graph. Safe on missing key.
                PostCounts::invalidate(post_key_slice)
            );
            idempotent_results.0?;
            idempotent_results.1?;
            idempotent_results.2?;
            idempotent_results.3?;
            Ok(())
        }
        OperationOutcome::MissingDependency => {
            // Drop the tag (non-retryable) if the tagged post's author is on a blacklisted HS.
            fail_on_permanent_ingest_error(ingestor.maybe_ingest_user(&author_id).await)?;
            Err(EventProcessorError::MissingDependency {
                dependency: vec![post_uri_str.to_owned()],
            })
        }
        OperationOutcome::CreatedOrDeleted => {
            // SAVE TO INDEXES
            let post_key_slice: &[&str] = &[&author_id, post_id];
            let tag_label_slice = &[tag_label.to_string()];

            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_post");
                // Invalidate the cached post counts.
                PostCounts::invalidate(post_key_slice),
                // Increment the label count to post
                TagPost::update_index_score(
                    &author_id,
                    Some(post_id),
                    tag_label,
                    ScoreAction::Increment(1.0),
                ),
                TagPost::add_tagger_to_index(&author_id, Some(post_id), &tagger_user_id, tag_label),
                PostsByTagSearch::update_index_score(
                    &author_id,
                    post_id,
                    tag_label,
                    ScoreAction::Increment(1.0),
                ),
                async {
                    if !post_relationships_is_reply(&author_id, post_id).await? {
                        PostStream::update_index_score(
                            &author_id,
                            post_id,
                            ScoreAction::Increment(1.0),
                        )
                        .await?;
                    }
                    Ok::<(), EventProcessorError>(())
                },
                PostsByTagSearch::put_to_index(&author_id, post_id, tag_label),
                // Kind lookup rides the join, not the critical path before it.
                async {
                    let tagged_post_kind = post_kind(&author_id, post_id).await?;
                    Notification::new_post_tag(
                        &tagger_user_id,
                        &author_id,
                        tag_label,
                        post_uri_str,
                        tagged_post_kind,
                    )
                    .await?;
                    Ok::<(), EventProcessorError>(())
                },
                // Add tag to search index
                TagSearch::put_to_index(tag_label_slice)
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;
            indexing_results.4?;
            indexing_results.5?;
            indexing_results.6?;
            indexing_results.7?;

            Ok(())
        }
    }
}

/// Handles the synchronization of a tagged user by updating the graph and indexes.
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
    tag_uri: &str,
    tag_label: &str,
    indexed_at: i64,
    ingestor: &UserIngestor,
) -> Result<(), EventProcessorError> {
    match TagUser::put_to_graph(
        &tagger_user_id,
        &tagged_user_id,
        None,
        tag_id,
        tag_uri,
        tag_label,
        indexed_at,
    )
    .await?
    {
        OperationOutcome::Updated => {
            // Re-run idempotent ops to recover from partial failure (graph wrote, Redis didn't)
            let tag_label_slice = &[tag_label.to_string()];
            let idempotent_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_user_retry");
                TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, tag_label),
                TagSearch::put_to_index(tag_label_slice)
            );
            idempotent_results.0?;
            idempotent_results.1?;

            UsersByTagSearch::sync_index_score(&tagged_user_id, tag_label).await?;
            Ok(())
        }
        OperationOutcome::MissingDependency => {
            // Drop the tag (non-retryable) if the tagged user is on a blacklisted HS.
            fail_on_permanent_ingest_error(ingestor.maybe_ingest_user(&tagged_user_id).await)?;

            let dependency = vec![tagged_user_id.to_string()];
            Err(EventProcessorError::MissingDependency { dependency })
        }
        OperationOutcome::CreatedOrDeleted => {
            let tag_label_slice = &[tag_label.to_string()];

            // SAVE TO INDEX
            let indexing_results = nexus_common::traced_join!(
                tracing::info_span!("index.write", phase = "tag_user");
                // Add label count to the user profile tag
                TagUser::update_index_score(&tagged_user_id, None, tag_label, ScoreAction::Increment(1.0)),
                // Add tagger to the user taggers list
                TagUser::add_tagger_to_index(&tagged_user_id, None, &tagger_user_id, tag_label),
                // Save new notification
                Notification::new_user_tag(&tagger_user_id, &tagged_user_id, tag_label),
                // Add tag to search index
                TagSearch::put_to_index(tag_label_slice)
            );

            indexing_results.0?;
            indexing_results.1?;
            indexing_results.2?;
            indexing_results.3?;

            // After the taggers SADD settled: the users-by-tag score is derived
            // from that set (SCARD), so first attempts and retries converge alike.
            UsersByTagSearch::sync_index_score(&tagged_user_id, tag_label).await?;

            Ok(())
        }
    }
}

/// Deletes the tag `key` names when no file address is at hand: see [`del_at`].
pub async fn del(key: ResourceKey) -> Result<(), EventProcessorError> {
    let tag_uri = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        key.owner_id, key.resource_id
    );
    del_at(key, &tag_uri).await
}

/// Deletes the tag `key` names, whichever folder wrote it: a tag is one edge per tagger,
/// target and label, found by its id alone, or by the id it had before the flip gave it a new
/// one. It holds the locks of the edge's ids: see [`lock_tags`]. A resource tag's feed del is
/// keyed by the address stored on the edge, which its put used; `tag_uri`, the address of the
/// file being deleted, is also checked along with both canonical epoch paths on a retry. A
/// resource tag the flip moved, whose move the feed has not published yet, is also retracted
/// at the address it moved from, which the consumer holds: that del is pending with the
/// other and published right after it.
#[tracing::instrument(
    name = "tag.del",
    skip_all,
    fields(user_id = %key.owner_id, tag_id = %key.resource_id)
)]
pub async fn del_at(key: ResourceKey, tag_uri: &str) -> Result<(), EventProcessorError> {
    // Prefix these vars with arg_ to indicate they come from the argument key.
    // Similarly named vars will be used in Step 2 below, reading fields from the query result
    let ResourceKey {
        owner_id: arg_user_id,
        resource_id: arg_tag_id,
    } = key;

    debug!("Deleting tag");

    // 1. Read target from graph WITHOUT deleting the edge. An edge the flip gave a new id is
    // found by either id, and its other handlers lock the other one: hold both.
    let tagger_id: &str = arg_user_id.as_ref();
    let mut locked = BTreeSet::from([arg_tag_id.clone()]);
    let mut _tag_locks = vec![lock_tag(tagger_id, &arg_tag_id).await];
    let row = loop {
        let row =
            fetch_row_from_graph(queries::get::get_tag_target(tagger_id, &arg_tag_id)).await?;
        let ids: BTreeSet<String> = row
            .iter()
            .flat_map(|row| ["tag_id", "v0_id"].map(|id| row.get::<Option<String>>(id)))
            .filter_map(|id| id.ok().flatten())
            .chain([arg_tag_id.clone()])
            .collect();
        if ids.is_subset(&locked) {
            break row;
        }
        // Released before the locks of both ids, which are taken in their own order
        drop(_tag_locks);
        _tag_locks = lock_tags(tagger_id, ids.iter().map(String::as_str)).await;
        locked = ids;
    };
    let Some(row) = row else {
        // The journals a retry finishes. First the one this file's address names, kept under
        // the address of an edge stored elsewhere. Then the journals of this address and of
        // both canonical epoch paths: the edge may have been owned by the other epoch's file,
        // and its URI is gone after the graph commit. Each append-and-clear is atomic;
        // replaying any is harmless.
        let mut journals: Vec<(String, bool)> = Vec::new();
        if let Some(stored_uri) = ResourceChange::aliased_delete(&arg_user_id, tag_uri).await? {
            journals.push((stored_uri, true));
        }
        journals.extend(
            [
                tag_uri.to_owned(),
                pubky_social_specs::legacy_v0::tag_uri_builder(
                    arg_user_id.to_string(),
                    arg_tag_id.clone(),
                ),
                pubky_social_specs::tag_uri_builder(arg_user_id.to_string(), arg_tag_id.clone()),
            ]
            .map(|uri| (uri, false)),
        );
        // A journal's del of the address its edge moved from is dropped if that address is
        // live again, a tag put there since: checked under the lock of that file too.
        let moved_from = loop {
            let mut moved_from = Vec::with_capacity(journals.len());
            for (uri, _) in &journals {
                moved_from.push(ResourceChange::pending_moved_from(&arg_user_id, uri).await?);
            }
            let ids: BTreeSet<String> = moved_from
                .iter()
                .flatten()
                .map(|uri| tag_id_of(uri).to_string())
                .chain([arg_tag_id.clone()])
                .collect();
            if ids.is_subset(&locked) {
                break moved_from;
            }
            drop(_tag_locks);
            _tag_locks = lock_tags(tagger_id, ids.iter().map(String::as_str)).await;
            locked = ids;
        };
        for ((uri, aliased), from) in journals.iter().zip(moved_from) {
            let live = match from {
                Some(from) => fetch_key_from_graph::<bool>(
                    queries::get::tag_address_is_live(tagger_id, &from),
                    "live",
                )
                .await?
                .unwrap_or(false),
                None => false,
            };
            let aliases: &[&str] = if *aliased { &[tag_uri] } else { &[] };
            ResourceChange::finish_aliased_delete(&arg_user_id, uri, aliases, live).await?;
        }
        return Ok(());
    };

    let tagged_user_id: Option<String> = row.get("user_id").unwrap_or(None);
    let post_id: Option<String> = row.get("post_id").unwrap_or(None);
    let author_id: Option<String> = row.get("author_id").unwrap_or(None);
    let post_uri: Option<String> = row.get("post_uri").unwrap_or(None);
    let resource_id: Option<String> = row.get("resource_id").unwrap_or(None);
    let label: String = row
        .get("label")
        .map_err(|e| EventProcessorError::generic(format!("Missing label in delete_tag: {e}")))?;
    // The feed's put used the address stored on the edge; key the del the same way.
    let stored_uri: Option<String> = row.get("tag_uri").unwrap_or(None);
    let event_uri = tag_uri;
    let tag_uri = stored_uri.as_deref().unwrap_or(tag_uri);
    // A retry that no longer finds the edge knows only the event's address: it names the
    // journal kept under the edge's until the del is published.
    let aliased = tag_uri != event_uri;

    // 2. Redis cleanup (guarded by tagger_in_index where non-idempotent)
    // Post-count cache to invalidate AFTER the graph edge is deleted (invalidating
    // before it is gone lets a concurrent read recache the pre-delete tag count).
    let mut post_counts_to_invalidate: Option<[String; 2]> = None;
    let mut resource_change = None;
    match (tagged_user_id, post_id, author_id, resource_id) {
        (Some(tagged_id), None, None, None) => {
            let tagger_in_index =
                TagUser::check_set_member(&[&tagged_id, &label], arg_user_id.as_ref())
                    .await?
                    .1;
            del_sync_user(arg_user_id.clone(), &tagged_id, &label, tagger_in_index).await?;
        }
        (None, Some(post_id), Some(author_id), None) => {
            let tagger_in_index =
                TagPost::check_set_member(&[&author_id, &post_id, &label], arg_user_id.as_ref())
                    .await?
                    .1;
            let post_uri = post_uri.ok_or_else(|| {
                EventProcessorError::generic(format!(
                    "Post {author_id}:{post_id} has no stored uri"
                ))
            })?;
            del_sync_post(
                arg_user_id.clone(),
                &post_id,
                &author_id,
                &post_uri,
                &label,
                tagger_in_index,
            )
            .await?;
            post_counts_to_invalidate = Some([author_id, post_id]);
        }
        // Resource tags keep no Redis index to clean up; the feed's del line
        // is prepared here and published after the graph deletion below.
        (None, None, None, Some(res_id)) => {
            let uri: String = row
                .get("resource_uri")
                .map_err(EventProcessorError::generic)?;
            let at = Utc::now().timestamp_millis();
            let del_line = |tag_uri: &str| {
                ResourceChange::tag(
                    ResourceChangeOp::Del,
                    res_id.clone(),
                    uri.clone(),
                    arg_user_id.to_string(),
                    label.clone(),
                    tag_uri,
                    at,
                )
            };
            // A move still to publish: the consumer holds the address the edge moved from
            let moved_from = match row.get::<Option<i64>>("moved_at").unwrap_or(None) {
                Some(_) => row.get::<Option<String>>("v0_uri").unwrap_or(None),
                None => None,
            };
            resource_change = Some((
                del_line(tag_uri)?,
                moved_from.as_deref().map(del_line).transpose()?,
            ));
        }
        _ => {
            debug!("DEL-Tag: Unexpected combination of tag details");
        }
    }

    if let Some((change, moved)) = &resource_change {
        if aliased {
            change
                .prepare_aliased_delete(event_uri, moved.as_ref())
                .await?;
        } else {
            change.prepare_delete(moved.as_ref()).await?;
        }
    }

    // 3. Graph deletion LAST — ensures data survives for retry if Redis ops fail
    fetch_row_from_graph(queries::del::delete_tag(&arg_user_id, &arg_tag_id)).await?;
    if resource_change.is_some() {
        let aliases: &[&str] = if aliased { &[event_uri] } else { &[] };
        ResourceChange::finish_aliased_delete(&arg_user_id, tag_uri, aliases, false).await?;
        // An earlier attempt of this delete, on an edge stored elsewhere and deleted since, may
        // have left its journal named from this file's address: the file is gone, so is that
        // edge, and its del goes out too
        if !aliased {
            if let Some(earlier) = ResourceChange::aliased_delete(&arg_user_id, event_uri).await? {
                if earlier != tag_uri {
                    ResourceChange::finish_aliased_delete(
                        &arg_user_id,
                        &earlier,
                        &[event_uri],
                        false,
                    )
                    .await?;
                }
            }
        }
    }

    // Now that the TAGGED edge is gone, invalidate the cached post counts so the
    // next read recomputes the lower count instead of recaching the pre-delete one.
    if let Some([author_id, post_id]) = post_counts_to_invalidate {
        PostCounts::invalidate(&[&author_id, &post_id]).await?;
    }

    Ok(())
}

async fn del_sync_user(
    tagger_id: PubkyId,
    tagged_id: &str,
    tag_label: &str,
    tagger_in_index: bool,
) -> Result<(), EventProcessorError> {
    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete", phase = "tag_user");
        // Guarded: Decrement label count to the user profile tag
        async {
            if tagger_in_index {
                TagUser::update_index_score(tagged_id, None, tag_label, ScoreAction::Decrement(1.0)).await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        async {
            // Idempotent: Remove tagger from the user taggers list (SREM)
            TagUser(vec![tagger_id.to_string()])
                .del_from_index(tagged_id, None, tag_label)
                .await?;
            Ok::<(), EventProcessorError>(())
        },
        // Guarded: notification
        async {
            if tagger_in_index {
                Notification::new_user_untag(&tagger_id, tagged_id, tag_label).await?;
            }
            Ok::<(), EventProcessorError>(())
        }
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;

    // After the taggers SREM settled: recompute the derived users-by-tag score,
    // dropping the member once the last tagger is gone.
    UsersByTagSearch::sync_index_score(tagged_id, tag_label).await?;

    Ok(())
}

async fn del_sync_post(
    tagger_id: PubkyId,
    post_id: &str,
    author_id: &str,
    post_uri: &str,
    tag_label: &str,
    tagger_in_index: bool,
) -> Result<(), EventProcessorError> {
    let tag_post = TagPost(vec![tagger_id.to_string()]);

    let indexing_results = nexus_common::traced_join!(
        tracing::info_span!("index.delete", phase = "tag_post");
        async {
            if tagger_in_index {
                // Decrement label score in the post
                TagPost::update_index_score(
                    author_id,
                    Some(post_id),
                    tag_label,
                    ScoreAction::Decrement(1.0),
                )
                .await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        // Guarded: Decrease post from label total engagement
        async {
            if tagger_in_index {
                PostsByTagSearch::update_index_score(
                    author_id,
                    post_id,
                    tag_label,
                    ScoreAction::Decrement(1.0),
                )
                .await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        async {
            if tagger_in_index && !post_relationships_is_reply(author_id, post_id).await? {
                PostStream::update_index_score(author_id, post_id, ScoreAction::Decrement(1.0))
                    .await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        // Guarded: notification
        async {
            if tagger_in_index {
                let tagged_post_kind = post_kind(author_id, post_id).await?;
                Notification::new_post_untag(
                    &tagger_id,
                    author_id,
                    tag_label,
                    post_uri,
                    tagged_post_kind,
                )
                .await?;
            }
            Ok::<(), EventProcessorError>(())
        },
        // Idempotent SREM: drop the tagger from the post taggers set. Kept inside
        // the join so it always clears the `tagger_in_index` gate even when another
        // arm errors, so a retry cannot double-apply the guarded decrements above.
        async {
            tag_post
                .del_from_index(author_id, Some(post_id), tag_label)
                .await?;
            Ok::<(), EventProcessorError>(())
        }
    );

    indexing_results.0?;
    indexing_results.1?;
    indexing_results.2?;
    indexing_results.3?;
    indexing_results.4?;

    // Tag-search cleanup runs AFTER the join so the SREM above and the concurrent
    // label-set ZINCRs have settled; otherwise `get_by_label` can see a phantom
    // member and skip the TagSearch removal (flaky under load). The post-count
    // cache is invalidated by the caller after the graph edge is deleted.
    PostsByTagSearch::del_from_index(author_id, post_id, tag_label).await?;
    let posts_by_tag =
        PostsByTagSearch::get_by_label(tag_label, None, Pagination::default()).await?;
    let posts_by_tag_found = posts_by_tag.is_some_and(|x| !x.is_empty());
    if !posts_by_tag_found {
        // Removed the last post using this tag: drop it from autocomplete suggestions.
        TagSearch::del_from_index(tag_label).await?;
    }

    Ok(())
}
