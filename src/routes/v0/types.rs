use serde::Deserialize;

#[derive(Deserialize)]
pub struct TagsQuery {
    pub limit_tags: Option<usize>,
    pub limit_taggers: Option<usize>,
    pub viewer_id: Option<String>,
    pub depth: Option<u8>
}
