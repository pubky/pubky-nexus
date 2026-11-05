//! The resource changes feed lines of the tags a flip moved. The feed keys a tag reference by
//! the path of its tag file, and the tags step moves every `Resource` tag edge the user wrote
//! from its `pubky.app` tag file to its `social/v1` one. A consumer still holds the reference
//! under the `pubky.app` path, and the v1 `DEL` the new app sends retracts the `social/v1`
//! path, so each move is published as a put at the new path followed by a del at the old one.
//!
//! The move is recorded on the edge, by the tags step's own write: `v0_uri`, the address the
//! feed published it under, and `moved_at`, set until the move is published. Publishing is
//! outside the flip, which never fails on the feed: [`publish_moves`] runs right after the
//! flip's epoch write, and a failure there is only logged; [`sweep`] runs periodically in the
//! watcher and publishes every move still recorded, the ones whose publish failed or never
//! ran. A consumer sees a move up to one sweep later when the first publish fails.
//!
//! Moves are published a page at a time, under the locks of the page's tag files (see
//! [`tag::lock_tag_files`]), the ones their handlers take, and read again under them: a `DEL`
//! or a takedown of a tag either ran before, and its edge is gone, or runs after its move is
//! published and retracts it. The page's puts and dels are appended in one script, then each
//! `moved_at` is cleared, only if it is still the move that was read. A crash between the
//! append and the clear leaves the moves recorded, and the next sweep appends them again:
//! duplicate puts and dels, which the feed's consumers ignore. A delete of a tag whose move is
//! still recorded retracts the old path itself, next to its own del (see [`tag::del_at`]).

use crate::events::handlers::tag;
use crate::events::EventProcessorError;
use nexus_common::db::{fetch_all_rows_from_graph, fetch_key_from_graph, queries, GraphError};
use nexus_common::models::event::resource_changes::{ResourceChange, ResourceChangeOp};
use pubky_social_specs::PubkyId;
use std::collections::BTreeSet;
use tokio::sync::MutexGuard;
use tracing::{debug, warn};

/// Edges per page of a sweep or of a user's publish: one graph read, then, under the page's
/// locks, one graph read, one append and one graph write.
pub const BATCH_SIZE: usize = 100;

/// The locks of a page's tag files, held while it is published.
type Locks = Vec<MutexGuard<'static, ()>>;

/// An edge whose move is still to publish, as a page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recorded {
    edge: String,
    tagger_id: String,
    tag_id: String,
    v0_id: Option<String>,
    moved_at: i64,
}

impl Recorded {
    /// The tag files its handlers lock it by, `(tagger_id, id)`: its id and its `v0_id`.
    fn files(&self) -> impl Iterator<Item = (String, String)> + '_ {
        [Some(&self.tag_id), self.v0_id.as_ref()]
            .into_iter()
            .flatten()
            .map(|id| (self.tagger_id.clone(), id.clone()))
    }
}

/// A recorded move as it is read again under its locks.
struct Moved {
    recorded: Recorded,
    tag_uri: String,
    v0_uri: String,
    label: String,
    indexed_at: i64,
    resource_id: String,
    resource_uri: String,
}

impl Moved {
    /// The put at the edge's address and the del at the address it moved from.
    fn lines(&self) -> Result<(ResourceChange, ResourceChange), EventProcessorError> {
        let line = |op, tag_uri: &str, at| {
            ResourceChange::tag(
                op,
                self.resource_id.clone(),
                self.resource_uri.clone(),
                self.recorded.tagger_id.clone(),
                self.label.clone(),
                tag_uri,
                at,
            )
        };
        Ok((
            line(ResourceChangeOp::Put, &self.tag_uri, self.indexed_at)?,
            line(ResourceChangeOp::Del, &self.v0_uri, self.recorded.moved_at)?,
        ))
    }
}

/// Publishes the moves `user_id`'s edges still record: the flip's own publish, right after
/// its epoch write. Returns how many it published.
pub async fn publish_moves(user_id: &PubkyId) -> Result<usize, EventProcessorError> {
    publish(Some(user_id.as_ref()), BATCH_SIZE).await
}

/// Publishes every move the graph still records, of every user, in pages of `batch_size`:
/// the watcher's periodic sweep. Returns how many it published.
pub async fn sweep(batch_size: usize) -> Result<usize, EventProcessorError> {
    publish(None, batch_size).await
}

/// Pages through the recorded moves of `tagger_id`, or of every tagger, by `(moved_at, edge)`,
/// so a run visits each once, and publishes each page. A graph or Redis error ends the run:
/// what is left stays recorded for the next. A move that cannot be published is logged and
/// left recorded, and the run goes on.
async fn publish(tagger_id: Option<&str>, batch_size: usize) -> Result<usize, EventProcessorError> {
    let mut after = (i64::MIN, String::new());
    let mut published = 0;
    loop {
        let query =
            queries::migration::get_moved_resource_tags(tagger_id, (after.0, &after.1), batch_size);
        let page = fetch_all_rows_from_graph(query)
            .await?
            .iter()
            .map(|row| {
                let required = |key: &str| row.get::<String>(key).map_err(GraphError::from);
                Ok(Recorded {
                    edge: required("edge")?,
                    tagger_id: required("tagger_id")?,
                    tag_id: required("tag_id")?,
                    v0_id: row
                        .get::<Option<String>>("v0_id")
                        .map_err(GraphError::from)?,
                    moved_at: row.get::<i64>("moved_at").map_err(GraphError::from)?,
                })
            })
            .collect::<Result<Vec<_>, GraphError>>()?;
        published += publish_page(&page).await?;
        match page.last() {
            Some(last) if page.len() >= batch_size => after = (last.moved_at, last.edge.clone()),
            _ => break,
        }
    }
    if published > 0 {
        debug!(tagger_id, published, "Published moved resource tags");
    }
    Ok(published)
}

/// Reads the moves of `page` that are still recorded, under the locks of their tag files.
/// Returns them with the locks, which the read is repeated under, and the locks taken again,
/// if it names a file they do not cover.
async fn read_locked(
    page: &[Recorded],
) -> Result<(Vec<Result<Moved, GraphError>>, Locks), EventProcessorError> {
    let edges: Vec<(String, String, String)> = page
        .iter()
        .map(|r| (r.edge.clone(), r.tagger_id.clone(), r.tag_id.clone()))
        .collect();
    let mut locked: BTreeSet<(String, String)> = page.iter().flat_map(Recorded::files).collect();
    let mut locks = tag::lock_tag_files(locked.iter().map(|(t, i)| (t, i))).await;
    loop {
        let moves: Vec<Result<Moved, GraphError>> =
            fetch_all_rows_from_graph(queries::migration::get_moved_resource_tags_at(&edges))
                .await?
                .iter()
                .map(|row| {
                    let required = |key: &str| row.get::<String>(key).map_err(GraphError::from);
                    let number = |key: &str| row.get::<i64>(key).map_err(GraphError::from);
                    Ok(Moved {
                        recorded: Recorded {
                            edge: required("edge")?,
                            tagger_id: required("tagger_id")?,
                            tag_id: required("tag_id")?,
                            v0_id: row
                                .get::<Option<String>>("v0_id")
                                .map_err(GraphError::from)?,
                            moved_at: number("moved_at")?,
                        },
                        tag_uri: required("tag_uri")?,
                        v0_uri: required("v0_uri")?,
                        label: required("label")?,
                        indexed_at: number("indexed_at")?,
                        resource_id: required("resource_id")?,
                        resource_uri: required("resource_uri")?,
                    })
                })
                .collect();
        let files: BTreeSet<(String, String)> = moves
            .iter()
            .flatten()
            .flat_map(|moved| moved.recorded.files())
            .collect();
        if files.is_subset(&locked) {
            return Ok((moves, locks));
        }
        // Released before the locks of every file, which are taken in their own order
        drop(locks);
        locked.extend(files);
        locks = tag::lock_tag_files(locked.iter().map(|(t, i)| (t, i))).await;
    }
}

/// Publishes the moves of `page` still recorded, under the locks of their tag files: the put
/// at each edge's address and the del at the address it moved from, in one append, then the
/// records cleared. Returns how many it published.
async fn publish_page(page: &[Recorded]) -> Result<usize, EventProcessorError> {
    if page.is_empty() {
        return Ok(0);
    }
    let (moves, _locks) = read_locked(page).await?;
    let mut lines = Vec::with_capacity(moves.len());
    let mut published = (
        Vec::with_capacity(moves.len()),
        Vec::with_capacity(moves.len()),
    );
    for moved in moves {
        match moved.map_err(EventProcessorError::from).and_then(|moved| {
            let lines = moved.lines()?;
            Ok((moved.recorded, lines))
        }) {
            Ok((recorded, move_lines)) => {
                published.0.push((recorded.edge, recorded.tagger_id));
                published.1.push(recorded.moved_at);
                lines.push(move_lines);
            }
            // Left recorded: each run tries it again, and the rest of the page goes on
            Err(e) => warn!(error = %e, "A recorded resource tag move cannot be published"),
        }
    }
    if lines.is_empty() {
        return Ok(0);
    }
    ResourceChange::store_moves(&lines).await?;
    let cleared = fetch_key_from_graph::<i64>(
        queries::migration::clear_moved_resource_tags(&published.0, &published.1),
        "cleared",
    )
    .await?
    .unwrap_or(0);
    if cleared < lines.len() as i64 {
        // Nothing else changes a recorded move while its locks are held
        warn!(
            published = lines.len(),
            cleared, "Published resource tag moves stayed recorded"
        );
    }
    Ok(lines.len())
}
