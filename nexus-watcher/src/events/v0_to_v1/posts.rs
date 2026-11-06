//! The posts step. Every post the user authored gets the `social/v1` address, tombstones and
//! posts the migrator skipped included, because Nexus can't tell which ones have a copy and
//! nothing fetches these addresses. In the same write, its attachments are rewritten one by
//! one, in order, through [`super::attachments::rewrite`], so each names the file the migrator
//! wrote. A titled article keeps its image first attachment, rewritten, though the migrator
//! moved it into the copy's `cover_image`: `content` has no `cover_image` and the attachment
//! is its only link to the image. `content` and `lock` are not touched.
//!
//! The caches that hold the address are then rewritten, never deleted, because several readers
//! don't refill a missing key: `Post:Details` from the graph, JSON only, with no sorted-set
//! writes, and `Post:Relationships` of the replies and reposts of those posts, other users'
//! included, through its reindex.
//!
//! Paging is keyset on the post id, never `SKIP`, so a rerun repeats the Redis writes of the
//! posts whose graph write already landed.

use super::attachments;
use super::files::FileMap;
use crate::events::EventProcessorError;
use nexus_common::db::graph::keyset_scan;
use nexus_common::db::{exec_single_row, fetch_all_rows_from_graph, queries, GraphError};
use nexus_common::models::post::{PostDetails, PostRelationships};
use pubky_social_specs::{post_uri_builder, PubkyId};
use tracing::debug;

/// Posts per batch: one graph write and one cache write each.
pub const BATCH_SIZE: usize = 100;

/// A post as the page reads it: its id and its stored attachments, `None` when it has none.
type StoredPost = (String, Option<Vec<String>>);

/// Migrates the posts `user_id` authored, their attachments resolved through `files`, in
/// batches of `batch_size`, each batch its own transaction. Running it again gives the same
/// result.
pub async fn migrate(
    user_id: &PubkyId,
    files: &FileMap,
    batch_size: usize,
) -> Result<(), EventProcessorError> {
    debug!(%user_id, "Migrating posts");
    let author_id: &str = user_id.as_ref();
    // The page queries return at most `MAX_QUERY_LIMIT` rows, and the scan stops on a page
    // shorter than its batch size: a bigger batch would read a full page as the last one
    let batch_size = batch_size.min(queries::get::MAX_QUERY_LIMIT);
    keyset_scan(batch_size, "migration.posts", |after_id| async move {
        let page = queries::migration::get_author_posts_page(author_id, &after_id, batch_size);
        let posts = fetch_all_rows_from_graph(page)
            .await?
            .iter()
            .map(|row| {
                Ok((
                    row.get::<String>("id").map_err(GraphError::from)?,
                    row.get::<Option<Vec<String>>>("attachments")
                        .map_err(GraphError::from)?,
                ))
            })
            .collect::<Result<Vec<StoredPost>, GraphError>>()?;
        let Some(last_id) = posts.last().map(|(post_id, _)| post_id.clone()) else {
            return Ok((0, None));
        };
        migrate_batch(user_id, files, &posts, batch_size).await?;
        Ok((posts.len(), Some(last_id)))
    })
    .await
}

/// One batch: the addresses and attachments in the graph, then the caches that hold them.
async fn migrate_batch(
    user_id: &PubkyId,
    files: &FileMap,
    posts: &[StoredPost],
    batch_size: usize,
) -> Result<(), EventProcessorError> {
    let author_id: &str = user_id.as_ref();
    let addresses: Vec<(String, String, Option<Vec<String>>)> = posts
        .iter()
        .map(|(post_id, stored)| {
            let uri = post_uri_builder(author_id.to_string(), post_id.clone());
            let attachments = stored.as_ref().map(|uris| {
                uris.iter()
                    .map(|uri| attachments::rewrite(user_id, files, uri))
                    .collect()
            });
            (post_id.clone(), uri, attachments)
        })
        .collect();
    exec_single_row(queries::migration::set_post_addresses(
        author_id, &addresses,
    ))
    .await?;

    let post_ids: Vec<String> = posts.iter().map(|(post_id, _)| post_id.clone()).collect();
    let details = PostDetails::get_many_from_graph(author_id, &post_ids).await?;
    PostDetails::put_many_to_index(author_id, details).await?;

    reindex_children_relationships(author_id, &post_ids, batch_size).await
}

/// Rebuilds `Post:Relationships` of every reply and repost of `post_ids`, whoever wrote them,
/// in pages of `batch_size` keyed on the child's id.
async fn reindex_children_relationships(
    author_id: &str,
    post_ids: &[String],
    batch_size: usize,
) -> Result<(), EventProcessorError> {
    keyset_scan(
        batch_size,
        "migration.posts.children",
        |after_id| async move {
            let page = queries::migration::get_post_children_page(
                author_id, post_ids, &after_id, batch_size,
            );
            let rows = fetch_all_rows_from_graph(page).await?;
            let mut last_id = None;
            for row in &rows {
                let child_author_id: String = row.get("author_id").map_err(GraphError::from)?;
                let child_post_id: String = row.get("post_id").map_err(GraphError::from)?;
                PostRelationships::reindex(&child_author_id, &child_post_id).await?;
                last_id = Some(child_post_id);
            }
            Ok((rows.len(), last_id))
        },
    )
    .await
}
