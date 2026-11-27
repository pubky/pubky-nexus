use crate::db::{fetch_row_from_graph, queries, GraphResult, OperationOutcome};
use crate::models::error::ModelResult;
use crate::models::tag::traits::collection::taggers_page;
use crate::models::tag::traits::taggers::TaggersTuple;
use crate::types::Pagination;

/// The longest target URI a resource tag may name, in code points: the social/v1 spec's cap on
/// a reference URI, applied to `pubky.app` tags as well. Each put and del of a resource tag
/// copies its target into a resource changes feed line.
pub const RESOURCE_TAG_TARGET_MAX_LENGTH: usize =
    pubky_social_specs::VALIDATION_LIMITS.reference_uri_max_length;

/// Tags on Resource nodes. They live in the graph only: unlike user and post
/// tags, they keep no Redis index to serve stale or rebuild, besides the global
/// tag search. A resource's tag list is read with its details, through
/// `ResourceView::get_by_id`.
pub struct TagResource;

impl TagResource {
    /// Taggers of `label` on a resource, read from the graph on every call: sorted
    /// by user id and paged by `taggers_page` as for user and post taggers, with
    /// the viewer's tag address on the label. A missing resource has no taggers.
    pub async fn get_taggers(
        resource_id: &str,
        label: &str,
        pagination: Pagination,
        viewer_id: Option<&str>,
    ) -> ModelResult<TaggersTuple> {
        let (skip, limit) = taggers_page(&pagination);
        let query =
            queries::get::resource_label_taggers(resource_id, label, viewer_id, skip, limit);
        match fetch_row_from_graph(query).await? {
            Some(row) => Ok((row.get("taggers")?, row.get("tag_uri")?)),
            None => Ok((Vec::new(), None)),
        }
    }

    /// Creates or merges a TAGGED relationship between a user and a Resource node.
    /// Unlike Post/User tags which use MATCH (target must exist), this uses MERGE
    /// for the Resource (first tag creates it). Returns the outcome and, unless the tagger is
    /// missing, the edge's `indexed_at`: the time of the put that created it, which a replay or an
    /// equivalent file keeps.
    #[allow(clippy::too_many_arguments)]
    pub async fn put_to_graph_resource(
        tagger_id: &str,
        resource_id: &str,
        uri: &str,
        scheme: &str,
        tag_id: &str,
        tag_uri: &str,
        label: &str,
        indexed_at: i64,
    ) -> GraphResult<(OperationOutcome, Option<i64>)> {
        let query = queries::put::create_resource_tag(
            tagger_id,
            resource_id,
            uri,
            scheme,
            tag_id,
            tag_uri,
            label,
            indexed_at,
        );
        let Some(row) = fetch_row_from_graph(query).await? else {
            return Ok((OperationOutcome::MissingDependency, None));
        };
        let outcome = match row.get("flag")? {
            true => OperationOutcome::Updated,
            false => OperationOutcome::CreatedOrDeleted,
        };
        Ok((outcome, Some(row.get("indexed_at")?)))
    }
}
