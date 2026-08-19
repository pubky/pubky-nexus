/// Pagination parameters for the DB layer.
#[derive(Default, Debug, Clone, Copy)]
pub struct Pagination {
    pub skip: Option<usize>,
    pub limit: Option<usize>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}
