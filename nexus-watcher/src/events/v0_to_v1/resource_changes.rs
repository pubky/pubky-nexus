//! The resource changes step. The resource changes feed keys a tag reference by the path of its
//! tag file, and the tags step moves every `Resource` tag edge the user wrote from its
//! `pubky.app` tag file to its `social/v1` one. A consumer still holds the reference under the
//! `pubky.app` path, and the v1 `DEL` the new app sends retracts the `social/v1` path, so each
//! move is published as a put at the new path followed by a del at the old one.
//!
//! The old address is gone from the edge once the tags step writes a page, so [`prepare`] holds
//! each del, keyed by the new path, before that write. After the tags step, [`publish`] reads
//! back every edge a held del names and appends its put and the del together, releasing the
//! del. A flip that fails anywhere before the epoch write runs again whole, and this step with
//! it, so a del held by an earlier attempt is published by a later one. A put published twice
//! is a duplicate the feed's consumers already ignore.

use crate::events::EventProcessorError;
use chrono::Utc;
use nexus_common::db::{fetch_all_rows_from_graph, queries, GraphError};
use nexus_common::models::event::resource_changes::{ResourceChange, ResourceChangeOp};
use pubky_social_specs::PubkyId;
use tracing::debug;

/// A `Resource` tag edge the tags step moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub resource_id: String,
    pub resource_uri: String,
    pub label: String,
    /// The edge's address before the write, which its feed put was published under.
    pub from: String,
    /// The edge's address after the write.
    pub to: String,
}

/// Holds the del of each move's old address, before the tags step writes the page.
pub async fn prepare(user_id: &PubkyId, moves: &[Move]) -> Result<(), EventProcessorError> {
    let author = user_id.to_string();
    let at = Utc::now().timestamp_millis();
    let held = moves
        .iter()
        .map(|m| {
            let del = ResourceChange::tag(
                ResourceChangeOp::Del,
                m.resource_id.clone(),
                m.resource_uri.clone(),
                author.clone(),
                m.label.clone(),
                &m.from,
                at,
            )?;
            Ok((m.to.clone(), del))
        })
        .collect::<Result<Vec<_>, EventProcessorError>>()?;
    ResourceChange::prepare_moves(&author, &held).await?;
    Ok(())
}

/// Publishes every held move of `user_id` whose edge is at its new address.
pub async fn publish(user_id: &PubkyId) -> Result<(), EventProcessorError> {
    let author: &str = user_id.as_ref();
    let mut cursor = 0;
    loop {
        let (next, paths) = ResourceChange::pending_moves(author, cursor).await?;
        if !paths.is_empty() {
            let tag_uris: Vec<String> = paths
                .iter()
                .map(|path| format!("pubky://{author}{path}"))
                .collect();
            let query = queries::migration::get_tagger_resource_tags_at(author, &tag_uris);
            let rows = fetch_all_rows_from_graph(query).await?;
            let puts = rows
                .iter()
                .map(|row| {
                    let get = |key: &str| row.get::<String>(key).map_err(GraphError::from);
                    let tag_uri = get("tag_uri")?;
                    ResourceChange::tag(
                        ResourceChangeOp::Put,
                        get("resource_id")?,
                        get("resource_uri")?,
                        author.to_string(),
                        get("label")?,
                        &tag_uri,
                        row.get::<i64>("indexed_at").map_err(GraphError::from)?,
                    )
                    .map_err(EventProcessorError::from)
                })
                .collect::<Result<Vec<_>, EventProcessorError>>()?;
            if puts.len() < paths.len() {
                // Not moved by this flip's tags step: left held, published by none
                debug!(
                    %user_id,
                    held = paths.len(),
                    moved = puts.len(),
                    "Held resource tag moves with no edge at their new address"
                );
            }
            ResourceChange::finish_moves(author, &puts).await?;
        }
        if next == 0 {
            return Ok(());
        }
        cursor = next;
    }
}
