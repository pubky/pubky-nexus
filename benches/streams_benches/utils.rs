use nexus_common::types::Pagination;

pub const LIMIT_20: Pagination = Pagination {
    skip: None,
    limit: Some(20),
    start: None,
    end: None,
};
