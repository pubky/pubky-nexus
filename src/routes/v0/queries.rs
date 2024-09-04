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
