use crate::Result;
use nexus_common::models::{file::FileDetails, post::PostView, traits::Collection};
use serde::{Deserialize, Serialize};
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
            Self::fetch_attachment_metadata(attachment_uris).await?
        } else {
            vec![]
        };

        Ok(Some(Self {
            view,
            attachments_metadata,
        }))
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

        Ok(FileDetails::get_by_ids(&keys)
            .await?
            .into_iter()
            .flatten()
            .collect())
    }
}
