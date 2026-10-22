//! In-scope records that point at a post outside the scope.
//!
//! A reply, repost, tag or bookmark by an in-scope user can target a post whose author
//! is a stub, or a former member that left the scope. That post is never read through
//! its author's stream, so the record would wait for it forever. Instead, that post is
//! fetched from its author's homeserver and indexed, with the out-of-scope posts it
//! depends on; nothing else of those authors' is read.

use std::collections::HashSet;

use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::error::ModelError;
use nexus_common::models::post::{PostDetails, PostRelationships};
use nexus_common::models::takedown::is_taken_down;
use nexus_common::models::user::stub::is_unread;
use nexus_common::models::user::UserIngestor;
use pubky::PubkyResource;
use pubky_app_specs::{ParsedUri, PubkyAppObject, PubkyAppPost, PubkyId, Resource};
use tracing::{debug, warn};

use super::post;
use crate::events::EventProcessorError;
use crate::homeserver::HomeserverClient;

/// Posts fetched for one record in one attempt. A chain longer than this continues
/// on the record's next retry, from where this attempt stopped.
const MAX_FETCHES: u32 = 20;
/// Where a chain too long for one attempt continues: target uri -> first post not fetched.
const RESUME_PREFIX: &str = "Scope:FetchResume";
/// How long a chain's resume point is kept: well past the retries of the record waiting
/// for it, so a record that is given up on doesn't leave it behind for good. Lost
/// earlier, the next attempt just fetches part of the chain again.
const RESUME_TTL_S: u64 = 7 * 24 * 3600;

tokio::task_local! {
    /// Set while a chain is being saved, so the post handler's own call for a missing
    /// target doesn't start a second walk inside the first.
    static WALKING: ();
}

/// A fetched out-of-scope post waiting to be saved.
struct Fetched {
    uri: String,
    author_id: PubkyId,
    post_id: String,
    post: PubkyAppPost,
}

/// Indexes the post at `post_uri`, and the out-of-scope posts it depends on, if its
/// author's stream is unread (a stub, or a member that left) and it isn't indexed yet.
///
/// The chain is collected by walking up through parents and reposted posts, then saved
/// oldest first, so every post's targets exist when it is saved. If the budget runs
/// out, the first post it didn't reach is remembered and the next attempt starts there.
///
/// Returns whether `post_uri` was newly indexed, so the caller can retry its record at
/// once. A failed fetch is logged and returns `false`: the record then waits for the
/// post as it would without a scope.
pub(super) async fn index_out_of_scope_post(
    post_uri: &ParsedUri,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<bool, EventProcessorError> {
    if WALKING.try_with(|_| ()).is_ok() || !is_unread(&post_uri.user_id).await? {
        return Ok(false);
    }
    let target = post_uri
        .try_to_uri_str()
        .map_err(EventProcessorError::generic)?;
    let resume_key = format!("{RESUME_PREFIX}:{target}");

    WALKING
        .scope((), async {
            let mut budget = MAX_FETCHES;
            let mut stopped_at = None;
            let mut redis_conn = get_redis_conn().await?;

            // Finish an earlier attempt's chain first, from where it stopped.
            let resume: Option<String> = redis_conn
                .get(&resume_key)
                .await
                .map_err(EventProcessorError::generic)?;
            if let Some(resume_uri) = resume
                .as_deref()
                .and_then(|uri| ParsedUri::try_from(uri).ok())
            {
                let chain =
                    collect_chain(&resume_uri, ingestor, client, &mut budget, &mut stopped_at)
                        .await?;
                save_chain(chain, ingestor, client).await?;
            }

            let chain =
                collect_chain(post_uri, ingestor, client, &mut budget, &mut stopped_at).await?;
            let indexed = save_chain(chain, ingestor, client).await?.contains(&target);

            let result: Result<(), _> = match (&stopped_at, &resume) {
                (Some(stopped_at), _) => {
                    warn!(%target, %stopped_at, "Chain of out-of-scope posts too long for one attempt; continuing on retry");
                    redis_conn.set_ex(&resume_key, stopped_at, RESUME_TTL_S).await
                }
                (None, Some(_)) => redis_conn.del(&resume_key).await,
                (None, None) => Ok(()),
            };
            result.map_err(EventProcessorError::generic)?;
            Ok(indexed)
        })
        .await
}

/// Fetches `start` and every out-of-scope post it depends on that isn't indexed yet,
/// in the order they were fetched: a post always comes before its targets.
async fn collect_chain(
    start: &ParsedUri,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
    budget: &mut u32,
    stopped_at: &mut Option<String>,
) -> Result<Vec<Fetched>, EventProcessorError> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut pending = vec![start.clone()];

    while let Some(post_uri) = pending.pop() {
        let Resource::Post(post_id) = &post_uri.resource else {
            continue;
        };
        let author_id = &post_uri.user_id;
        let uri = post_uri
            .try_to_uri_str()
            .map_err(EventProcessorError::generic)?;
        if !seen.insert(uri.clone())
            || remove_if_taken_down(&uri, author_id, post_id, ingestor).await?
            || !is_unread(author_id).await?
            || PostDetails::get_by_id(author_id, post_id).await?.is_some()
        {
            continue;
        }
        if *budget == 0 {
            stopped_at.get_or_insert(uri);
            continue;
        }
        // Checked for every fetch, also for an author already known: its homeserver may
        // have been blacklisted since, or it may have moved onto a blacklisted one.
        match ingestor.ensure_hs_not_blacklisted(author_id).await {
            Ok(_) => {}
            Err(ModelError::HsBlacklisted { hs_id }) => {
                debug!(%uri, %hs_id, "Not fetching an out-of-scope post from a blacklisted homeserver");
                continue;
            }
            Err(e) => {
                warn!(%uri, error = %e, "Could not check the homeserver of an out-of-scope post");
                continue;
            }
        }
        *budget -= 1;

        let post = match fetch_post(&uri, &post_uri.resource, client).await {
            Ok(Some(post)) => post,
            Ok(None) => continue,
            Err(e) => {
                warn!(%uri, error = %e, "Could not fetch out-of-scope post");
                continue;
            }
        };
        let targets = PostRelationships::from_homeserver(&post);
        for target in [targets.replied, targets.reposted].into_iter().flatten() {
            if ingestor.maybe_ingest_author_of_post(&target).await.is_ok() {
                pending.push(target);
            }
        }
        chain.push(Fetched {
            uri,
            author_id: author_id.clone(),
            post_id: post_id.clone(),
            post,
        });
    }
    Ok(chain)
}

/// Fetches the post at `uri` through `client`; `None` if the record there isn't a post.
async fn fetch_post(
    uri: &str,
    resource: &Resource,
    client: &dyn HomeserverClient,
) -> Result<Option<PubkyAppPost>, EventProcessorError> {
    let pubky_resource: PubkyResource = uri.parse()?;
    let blob = client.get_resource(&pubky_resource).await?;
    let object = PubkyAppObject::from_resource(resource, blob.as_slice())
        .map_err(|e| EventProcessorError::SpecValidation(e.to_string()))?;
    Ok(match object {
        PubkyAppObject::Post(post) => Some(post),
        _ => None,
    })
}

/// Whether the moderator took the post at `uri` down; if so, a copy indexed anyway (by
/// a save that raced the takedown, whose removal then failed) is removed. Removing a post
/// that isn't indexed does nothing.
async fn remove_if_taken_down(
    uri: &str,
    author_id: &PubkyId,
    post_id: &str,
    ingestor: &UserIngestor,
) -> Result<bool, EventProcessorError> {
    if !is_taken_down(uri).await? {
        return Ok(false);
    }
    // Boxed: the delete handler's future would otherwise nest inside every save's.
    match Box::pin(post::del(author_id.clone(), post_id.to_string(), ingestor)).await {
        Ok(()) | Err(EventProcessorError::SkipIndexing) => Ok(true),
        Err(e) => Err(e),
    }
}

/// Saves a chain oldest first, in passes until a pass saves nothing, so a post whose
/// targets were fetched in an awkward order is saved on a later pass. Returns the uris
/// saved.
///
/// Each post is checked for a takedown again once saved, as events are: one recorded
/// since the chain was fetched removes it again. A removal that fails fails the record,
/// and its retry finishes the removal (see [`remove_if_taken_down`] in [`collect_chain`]).
async fn save_chain(
    chain: Vec<Fetched>,
    ingestor: &UserIngestor,
    client: &dyn HomeserverClient,
) -> Result<HashSet<String>, EventProcessorError> {
    let mut unsaved: Vec<Fetched> = chain.into_iter().rev().collect();
    let mut saved = HashSet::new();

    loop {
        let saved_before = saved.len();
        let mut left = Vec::new();
        for fetched in unsaved {
            // Boxed: the post handler calls back here for a missing target (and returns at
            // once, see `WALKING`), which makes the two futures recursive.
            let result = Box::pin(post::sync_put(
                fetched.post.clone(),
                fetched.uri.clone(),
                fetched.author_id.clone(),
                fetched.post_id.clone(),
                ingestor,
                client,
            ))
            .await;
            match result {
                Ok(()) => {
                    let (author_id, post_id) = (&fetched.author_id, &fetched.post_id);
                    if remove_if_taken_down(&fetched.uri, author_id, post_id, ingestor).await? {
                        debug!(uri = %fetched.uri, "Out-of-scope post taken down while it was fetched");
                        continue;
                    }
                    debug!(uri = %fetched.uri, "Indexed out-of-scope post an in-scope record points at");
                    saved.insert(fetched.uri);
                }
                Err(e) => {
                    debug!(uri = %fetched.uri, error = %e, "Out-of-scope post not indexed yet");
                    left.push(fetched);
                }
            }
        }
        if left.is_empty() || saved.len() == saved_before {
            return Ok(saved);
        }
        unsaved = left;
    }
}
