pub mod stream;
pub mod tag;
pub mod view;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ResourceDetails {
    pub id: String,
    pub uri: String,
    pub scheme: String,
    pub indexed_at: i64,
}
