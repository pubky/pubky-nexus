use crate::models::post::{PostStreamSorting, ViewerStreamSource};

pub struct PostStreamValues {
    pub viewer_id: Option<String>,
    pub author_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub post_id: Option<String>,
}

impl PostStreamValues {
    pub fn new(
        viewer_id: Option<String>,
        author_id: Option<String>,
        tags: Option<Vec<String>>,
        post_id: Option<String>,
    ) -> Self {
        Self {
            viewer_id,
            author_id,
            tags,
            post_id,
        }
    }
}

pub struct PostStreamFilters {
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub sorting: PostStreamSorting,
    pub source: ViewerStreamSource,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

impl PostStreamFilters {
    pub fn new(
        sorting: PostStreamSorting,
        source: ViewerStreamSource,
        skip: Option<usize>,
        limit: Option<usize>,
        start: Option<f64>,
        end: Option<f64>,
    ) -> Self {
        Self {
            skip,
            limit,
            sorting,
            source,
            start,
            end,
        }
    }
}
