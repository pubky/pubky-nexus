//! In-scope records that point at a post outside the scope.
//!
//! A reply, repost, tag or bookmark by an in-scope user can target a post whose
//! author is a stub. That post is never read through its author's stream, so the
//! record would wait for it forever. Instead, that post is fetched from its author's
//! homeserver and indexed, along with any of its own out-of-scope parents it needs;
//! the authors stay stubs and nothing else of theirs is read.

use std::collections::HashSet;

use deadpool_redis::redis;
use nexus_common::db::get_redis_conn;
use nexus_common::models::post::PostDetails;
use nexus_common::models::takedown::is_taken_down;
use nexus_common::models::user::{is_stub, UserIngestor};
use pubky_app_specs::{ParsedUri, PubkyAppObject, PubkyAppPost, Resource};
use tracing::{debug, warn};

use super::post;
use crate::events::{fetch_object, EventProcessorError};

/// Posts fetched for one record in one attempt. A chain longer than this continues
/// on the record's next retry, from where this attempt stopped.
const MAX_FETCHES: u32 = 20;
/// Where a chain too long for one attempt continues: target uri -> first post not fetched.
const RESUME_PREFIX: &str = "Scope:FetchResume";

tokio::task_local! {
    /// Set while a chain is being saved, so the post handler's own call for a missing
    /// target doesn't start a second walk inside the first.
    static WALKING: ();
}

/// Indexes the post at `post_uri`, and the out-of-scope posts it depends on, if its
/// author is a stub and it isn't indexed yet.
///
/// The chain is first collected by walking up through parents and reposted posts,
/// then saved oldest first, so every post's targets exist when it is saved. If the
/// budget runs out, the first post it didn't reach is remembered, and the next
/// attempt for the same target starts there, so a long chain lands over retries.
///
/// Returns whether `post_uri` was newly indexed, so the caller can retry its record at
/// once. A failed fetch is logged and returns `false`: the record then waits for the
/// post as it would without a scope.
pub(super) async fn index_out_of_scope_post(
    post_uri: &ParsedUri,
    ingestor: &UserIngestor,
) -> Result<bool, EventProcessorError> {
    if WALKING.try_with(|_| ()).is_ok() {
        return Ok(false);
    }
    let target = post_uri
        .try_to_uri_str()
        .map_err(EventProcessorError::generic)?;

    WALKING
        .scope((), async {
            let mut budget = MAX_FETCHES;
            let mut stopped_at = None;

            // Finish an earlier attempt's chain first, from where it stopped.
            if let Some(resume) = read_resume(&target).await? {
                if let Ok(resume_uri) = ParsedUri::try_from(resume.as_str()) {
                    let chain =
                        collect_chain(&resume_uri, ingestor, &mut budget, &mut stopped_at).await?;
                    save_chain(chain, ingestor).await;
                }
            }

            let chain = collect_chain(post_uri, ingestor, &mut budget, &mut stopped_at).await?;
            let indexed = save_chain(chain, ingestor).await.contains(&target);
            write_resume(&target, stopped_at.as_deref()).await?;
            Ok(indexed)
        })
        .await
}

/// Fetches `start` and every out-of-scope post it depends on that isn't indexed yet,
/// in the order they were fetched: a post always comes before its targets.
async fn collect_chain(
    start: &ParsedUri,
    ingestor: &UserIngestor,
    budget: &mut u32,
    stopped_at: &mut Option<String>,
) -> Result<Vec<(String, ParsedUri, PubkyAppPost)>, EventProcessorError> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut pending = vec![start.clone()];

    while let Some(post_uri) = pending.pop() {
        let Resource::Post(post_id) = &post_uri.resource else {
            continue;
        };
        let uri = post_uri
            .try_to_uri_str()
            .map_err(EventProcessorError::generic)?;
        if !seen.insert(uri.clone())
            || is_taken_down(&uri).await?
            || !is_stub(&post_uri.user_id).await?
            || PostDetails::get_by_id(&post_uri.user_id, post_id)
                .await?
                .is_some()
        {
            continue;
        }
        if *budget == 0 {
            stopped_at.get_or_insert(uri);
            continue;
        }
        *budget -= 1;

        let post = match fetch_object(&uri, &post_uri.resource).await {
            Ok(PubkyAppObject::Post(post)) => post,
            Ok(_) => continue,
            Err(e) => {
                warn!(%uri, error = %e, "Could not fetch out-of-scope post");
                continue;
            }
        };
        for target in targets_of(&post) {
            if ingestor.maybe_ingest_author_of_post(&target).await.is_ok() {
                pending.push(target);
            }
        }
        chain.push((uri, post_uri, post));
    }
    Ok(chain)
}

/// Saves a chain oldest first, in passes until a pass saves nothing, so a post whose
/// targets were fetched in an awkward order is saved on a later pass. Returns the
/// uris saved.
async fn save_chain(
    chain: Vec<(String, ParsedUri, PubkyAppPost)>,
    ingestor: &UserIngestor,
) -> HashSet<String> {
    let mut unsaved: Vec<_> = chain.into_iter().rev().collect();
    let mut saved = HashSet::new();

    loop {
        let saved_before = saved.len();
        let mut left = Vec::new();
        for (uri, post_uri, post) in unsaved {
            let Resource::Post(post_id) = &post_uri.resource else {
                continue;
            };
            // Boxed: the post handler calls back here for a missing target (and returns at
            // once, see `WALKING`), which makes the two futures recursive.
            let result = Box::pin(post::sync_put(
                post.clone(),
                uri.clone(),
                post_uri.user_id.clone(),
                post_id.clone(),
                ingestor,
            ))
            .await;
            match result {
                Ok(()) => {
                    debug!(%uri, "Indexed out-of-scope post an in-scope record points at");
                    saved.insert(uri);
                }
                Err(e) => {
                    debug!(%uri, error = %e, "Out-of-scope post not indexed yet");
                    left.push((uri, post_uri, post));
                }
            }
        }
        if left.is_empty() || saved.len() == saved_before {
            return saved;
        }
        unsaved = left;
    }
}

/// The posts a post depends on: the one it replies to and the one it reposts.
fn targets_of(post: &PubkyAppPost) -> Vec<ParsedUri> {
    let parent = post.parent.as_deref();
    let embed = post.embed.as_ref().map(|embed| embed.uri.as_str());
    [parent, embed]
        .into_iter()
        .flatten()
        .filter_map(|uri| ParsedUri::try_from(uri).ok())
        .collect()
}

async fn read_resume(target: &str) -> Result<Option<String>, EventProcessorError> {
    let mut redis_conn = get_redis_conn().await?;
    let resume: Option<String> = redis::cmd("GET")
        .arg(format!("{RESUME_PREFIX}:{target}"))
        .query_async(&mut redis_conn)
        .await
        .map_err(EventProcessorError::generic)?;
    Ok(resume)
}

/// Remembers where the chain stopped, or forgets it once the chain was walked whole.
async fn write_resume(target: &str, stopped_at: Option<&str>) -> Result<(), EventProcessorError> {
    let key = format!("{RESUME_PREFIX}:{target}");
    let mut redis_conn = get_redis_conn().await?;
    let cmd = match stopped_at {
        Some(stopped_at) => {
            warn!(%target, %stopped_at, "Chain of out-of-scope posts too long for one attempt; continuing on retry");
            let mut cmd = redis::cmd("SET");
            cmd.arg(&key).arg(stopped_at);
            cmd
        }
        None => {
            let mut cmd = redis::cmd("DEL");
            cmd.arg(&key);
            cmd
        }
    };
    let () = cmd
        .query_async(&mut redis_conn)
        .await
        .map_err(EventProcessorError::generic)?;
    Ok(())
}
