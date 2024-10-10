use crate::models::post::{PostStreamReach, PostStreamSorting};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct TagsQuery {
    pub limit_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
}

#[derive(Deserialize)]
pub struct PaginationQuery {
    pub skip: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct PostStreamQuery {
    pub viewer_id: Option<String>,
    pub author_id: Option<String>,
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub sorting: Option<PostStreamSorting>,
    pub reach: Option<PostStreamReach>,
    pub tag: Option<String>,
}
