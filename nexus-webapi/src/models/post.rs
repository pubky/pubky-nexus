use nexus_common::models::{file::FileDetails, post::PostView};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct Post {
    #[serde(flatten)]
    pub view: PostView,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments_metadata: Vec<FileDetails>,
}
