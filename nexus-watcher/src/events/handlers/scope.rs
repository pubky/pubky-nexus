//! In-scope records that point at a post outside the scope.
//!
//! A reply, repost, tag or bookmark by an in-scope user can target a post whose
//! author is a stub. That post is never read through its author's stream, so the
//! record would wait for it forever. Instead, that one post is fetched and indexed;
//! its author stays a stub and nothing else of theirs is read.

use nexus_common::models::post::PostDetails;
use nexus_common::models::user::{is_stub, UserIngestor};
use pubky_app_specs::{ParsedUri, PubkyAppObject, Resource};
use tracing::{debug, warn};

use super::post;
use crate::events::{fetch_object, EventProcessorError};

/// How many posts up a chain of out-of-scope parents are fetched for one record.
const MAX_FETCH_DEPTH: u8 = 8;

tokio::task_local! {
    /// Out-of-scope posts being fetched above this one, so a long or circular chain
    /// of parents stops.
    static FETCH_DEPTH: u8;
}

/// Indexes the post at `post_uri` if its author is a stub and it isn't indexed yet.
///
/// Returns whether the post was newly indexed, so the caller can retry its own
/// record at once. A failed fetch is logged and returns `false`: the caller's record
/// then waits for the post as it would without a scope.
pub(super) async fn index_out_of_scope_post(
    post_uri: &ParsedUri,
    ingestor: &UserIngestor,
) -> Result<bool, EventProcessorError> {
    let Resource::Post(post_id) = &post_uri.resource else {
        return Ok(false);
    };
    let author_id = &post_uri.user_id;

    if !is_stub(author_id).await? || PostDetails::get_by_id(author_id, post_id).await?.is_some() {
        return Ok(false);
    }

    let depth = FETCH_DEPTH.try_with(|depth| *depth).unwrap_or(0);
    if depth >= MAX_FETCH_DEPTH {
        warn!(%author_id, %post_id, "Not fetching out-of-scope post: chain of parents too long");
        return Ok(false);
    }

    let uri = post_uri
        .try_to_uri_str()
        .map_err(EventProcessorError::generic)?;
    let post = match fetch_object(&uri, &post_uri.resource).await {
        Ok(PubkyAppObject::Post(post)) => post,
        Ok(_) => return Ok(false),
        Err(e) => {
            warn!(%uri, error = %e, "Could not fetch out-of-scope post");
            return Ok(false);
        }
    };

    let indexed = FETCH_DEPTH
        .scope(
            depth + 1,
            Box::pin(post::sync_put(
                post,
                uri.clone(),
                author_id.clone(),
                post_id.clone(),
                ingestor,
            )),
        )
        .await;

    match indexed {
        Ok(()) => {
            debug!(%uri, "Indexed out-of-scope post an in-scope record points at");
            Ok(true)
        }
        Err(e) => {
            warn!(%uri, error = %e, "Could not index out-of-scope post");
            Ok(false)
        }
    }
}
