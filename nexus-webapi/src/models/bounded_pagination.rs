use super::{BoundedLimit, BoundedSkip};
use nexus_common::types::Pagination;
use serde::Deserialize;

/// Bounded pagination pair for route query structs.
///
/// Flatten into a query struct with `#[serde(flatten)]`.  Both fields are
/// optional; absent params resolve to their compile-time defaults (`0` for
/// skip, `LIMIT_DEFAULT` for limit).  Out-of-range values are rejected at
/// deserialization with a 400 response.
///
/// Use `to_pagination(start, end)` to produce a nexus-common `Pagination`
/// ready for the DB layer.  Pass `None, None` when no cursor fields are needed.
#[derive(Deserialize, Debug, Default)]
pub struct BoundedPagination<
    const SKIP_MAX: usize,
    const LIMIT_DEFAULT: usize,
    const LIMIT_MAX: usize,
> {
    pub skip: Option<BoundedSkip<SKIP_MAX>>,
    pub limit: Option<BoundedLimit<LIMIT_DEFAULT, LIMIT_MAX>>,
}

impl<const SKIP_MAX: usize, const LIMIT_DEFAULT: usize, const LIMIT_MAX: usize>
    BoundedPagination<SKIP_MAX, LIMIT_DEFAULT, LIMIT_MAX>
{
    pub fn skip_value(&self) -> usize {
        self.skip.as_ref().map_or(0, BoundedSkip::value)
    }

    pub fn limit_value(&self) -> usize {
        self.limit
            .as_ref()
            .map_or(LIMIT_DEFAULT, BoundedLimit::value)
    }

    /// Build a nexus-common `Pagination` from the bounded values.
    /// Pass `start`/`end` cursor timestamps when the route supports them;
    /// pass `None, None` for pure offset pagination.
    pub fn to_pagination(&self, start: Option<f64>, end: Option<f64>) -> Pagination {
        Pagination {
            skip: Some(self.skip_value()),
            limit: Some(self.limit_value()),
            start,
            end,
        }
    }
}
