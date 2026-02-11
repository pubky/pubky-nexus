use std::collections::{HashMap, HashSet};

use crate::Result;
use nexus_common::models::{file::FileDetails, post::PostView, traits::Collection};
use serde::{Deserialize, Serialize};
use tracing::warn;
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct PostViewDetailed {
    #[serde(flatten)]
    pub view: PostView,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments_metadata: Vec<FileDetails>,
}

impl PostViewDetailed {
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
            let attachment_uris = view.details.attachments.as_deref().unwrap_or(&[]);
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
            return Ok(Self(
                views
                    .into_iter()
                    .map(|view| PostViewDetailed {
                        view,
                        attachments_metadata: vec![],
                    })
                    .collect(),
            ));
        }

        // Collect unique attachment URIs across all posts for a single batched fetch
        let all_uris: Vec<String> = views
            .iter()
            .flat_map(|v| v.details.attachments.as_deref().unwrap_or(&[]))
            .collect::<HashSet<_>>()
            .into_iter()
            .cloned()
            .collect();

        // Single batched fetch, then build a lookup by database key (owner_id, file_id)
        // This is more robust than URI string matching as it's immune to normalization differences
        let metadata_by_key: HashMap<(String, String), FileDetails> =
            fetch_attachment_metadata(&all_uris)
                .await?
                .into_iter()
                .map(|fd| ((fd.owner_id.clone(), fd.id.clone()), fd))
                .collect();

        // Distribute results back to each post by parsing URIs to database keys
        let detailed = views
            .into_iter()
            .map(|view| {
                let attachments_metadata = view
                    .details
                    .attachments
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|uri| {
                        let file_key = FileDetails::file_key_from_uri(uri);
                        if file_key.len() == 2 {
                            metadata_by_key
                                .get(&(file_key[0].clone(), file_key[1].clone()))
                                .cloned()
                        } else {
                            warn!("Invalid file URI format, cannot extract key: {}", uri);
                            None
                        }
                    })
                    .collect();
                PostViewDetailed {
                    view,
                    attachments_metadata,
                }
            })
            .collect();

        Ok(Self(detailed))
    }
}

async fn fetch_attachment_metadata(attachments: &[String]) -> Result<Vec<FileDetails>> {
    if attachments.is_empty() {
        return Ok(vec![]);
    }

    let file_keys: Vec<Vec<String>> = attachments
        .iter()
        .map(|uri| FileDetails::file_key_from_uri(uri))
        .collect();

    let keys_refs: Vec<Vec<&str>> = file_keys
        .iter()
        .map(|k| k.iter().map(|s| s.as_str()).collect())
        .collect();
    let keys: Vec<&[&str]> = keys_refs.iter().map(|v| v.as_slice()).collect();

    let results = FileDetails::get_by_ids(&keys).await?;

    Ok(results
        .into_iter()
        .zip(attachments.iter())
        .filter_map(|(details, uri)| {
            if details.is_none() {
                warn!("Attachment metadata not found for URI: {}", uri);
            }
            details
        })
        .collect())
}
