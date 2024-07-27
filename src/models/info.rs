use serde::Serialize;
use utoipa::ToSchema;

use crate::db::kv::last_save::get_last_rdb_save_time;

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub name: String,
    pub repository: String,
    pub version: String,
    pub last_index_snapshot: u64,
}

impl ServerInfo {
    pub async fn new() -> Self {
        let last_index_snapshot = get_last_rdb_save_time().await.unwrap_or_default();
        Self {
            description: env!("CARGO_PKG_DESCRIPTION").to_string(),
            homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
            license: env!("CARGO_PKG_LICENSE").to_string(),
            name: env!("CARGO_PKG_NAME").to_string(),
            repository: env!("CARGO_PKG_REPOSITORY").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            last_index_snapshot,
        }
    }
}
