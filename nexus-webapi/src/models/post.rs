use std::collections::{HashMap, HashSet};

use crate::Result;
use nexus_common::models::{file::FileDetails, post::PostView, traits::Collection};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};
use utoipa::openapi::Required;
use utoipa::{IntoParams, PartialSchema, ToSchema};

/// The `include_attachment_metadata` flag of `/v0`'s post view and post streams: whether the
/// `File` nodes of a post's attachments are fetched into `attachments_metadata`. Read as the
/// `bool` it wraps, so a value that is not `true` or `false` is the 400 it has always been,
/// and documented as the one query parameter it is. `/v1` reads [`NoAttachmentMetadata`] in
/// its place: a `social/v1` file has no `File` node, so there is no metadata to promise.
///
/// [`NoAttachmentMetadata`]: crate::models::v1::NoAttachmentMetadata
#[derive(Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
#[serde(transparent)]
pub struct IncludeAttachmentMetadata(pub bool);

impl From<IncludeAttachmentMetadata> for bool {
    fn from(flag: IncludeAttachmentMetadata) -> Self {
        flag.0
    }
}

/// The parameter as `/v0` has always listed it, after the route's other query parameters.
impl IntoParams for IncludeAttachmentMetadata {
    fn into_params(_: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
        vec![ParameterBuilder::new()
            .name("include_attachment_metadata")
            .parameter_in(ParameterIn::Query)
            .description(Some("Include file metadata for post attachments"))
            .required(Required::False)
            .schema(Some(bool::schema()))
            .build()]
    }
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct PostViewDetailed {
    #[serde(flatten)]
    pub view: PostView,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments_metadata: Vec<FileDetails>,
}

impl PostViewDetailed {
    pub fn new(view: PostView, attachments_metadata: Vec<FileDetails>) -> Self {
        Self {
            view,
            attachments_metadata,
        }
    }

    pub async fn get_by_id(
        author_id: &str,
        post_id: &str,
        viewer_id: Option<&str>,
        limit_tags: Option<usize>,
        limit_taggers: Option<usize>,
        include_attachment_metadata: bool,
    ) -> Result<Option<Self>> {
        let Some(view) =
            PostView::get_by_id(author_id, post_id, viewer_id, limit_tags, limit_taggers).await?
        else {
            return Ok(None);
        };

        let attachments_metadata = if include_attachment_metadata {
            let attachment_uris = &view.details.attachments;
            fetch_attachment_metadata(attachment_uris).await?
        } else {
            vec![]
        };

        Ok(Some(Self {
            view,
            attachments_metadata,
        }))
    }
}

#[derive(Deserialize, Serialize, ToSchema, Default)]
pub struct PostStreamDetailed(pub Vec<PostViewDetailed>);

impl PostStreamDetailed {
    pub async fn from_post_views(
        views: Vec<PostView>,
        include_attachment_metadata: bool,
    ) -> Result<Self> {
        if !include_attachment_metadata {
            let views_detailed = views
                .into_iter()
                .map(|view| PostViewDetailed::new(view, vec![]))
                .collect();
            return Ok(Self(views_detailed));
        }

        // Collect unique attachment URIs across all posts for a single batched fetch
        let all_uris: Vec<String> = views
            .iter()
            .flat_map(|v| &v.details.attachments)
            .collect::<HashSet<_>>()
            .into_iter()
            .cloned()
            .collect();

        // Single batched fetch, then build a lookup by DB key (owner_id, file_id)
        let metadata_by_key: HashMap<(String, String), FileDetails> =
            fetch_attachment_metadata(&all_uris)
                .await?
                .into_iter()
                .map(|fd| ((fd.owner_id.clone(), fd.id.clone()), fd))
                .collect();

        // Distribute results back to each post
        let detailed = views
            .into_iter()
            .map(|view| {
                let attachments_metadata = view
                    .details
                    .attachments
                    .iter()
                    .filter_map(|uri| {
                        let key = FileDetails::file_key_from_uri(uri)?;
                        metadata_by_key.get(&key).cloned()
                    })
                    .collect();
                PostViewDetailed::new(view, attachments_metadata)
            })
            .collect();

        Ok(Self(detailed))
    }
}

/// Fetches file metadata for a list of attachment URIs in a single batched DB call.
/// Malformed URIs and missing entries are logged and skipped gracefully.
async fn fetch_attachment_metadata(attachments: &[String]) -> Result<Vec<FileDetails>> {
    if attachments.is_empty() {
        return Ok(vec![]);
    }

    // Parse each URI into (owner_id, file_id) DB keys, filtering out malformed ones
    let valid_keys: Vec<((String, String), &String)> = attachments
        .iter()
        .filter_map(|uri| {
            let key = FileDetails::file_key_from_uri(uri);
            if key.is_none() {
                // A `social/v1` or web attachment is a normal address with no `File` node, not
                // a fault worth a warning on every read of the post.
                debug!("Skipping invalid file URI: {}", uri);
            }
            Some((key?, uri))
        })
        .collect();

    // Reshape owned keys into the borrowed slices that get_by_ids expects
    let key_refs: Vec<[&str; 2]> = valid_keys
        .iter()
        .map(|((owner, id), _)| [owner.as_str(), id.as_str()])
        .collect();
    let slice_keys: Vec<&[&str]> = key_refs.iter().map(|arr| arr.as_slice()).collect();

    let results = FileDetails::get_by_ids(&slice_keys).await?;

    // Results arrive in the same order as the input keys; map them back to the original URIs
    Ok(results
        .into_iter()
        .zip(valid_keys.iter())
        .filter_map(|(details, (_, uri))| {
            if details.is_none() {
                warn!("Attachment metadata not found for URI: {}", uri);
            }
            details
        })
        .collect())
}
