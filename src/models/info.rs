use serde::Serialize;
use utoipa::ToSchema;

use crate::{db::kv::last_save::get_last_rdb_save_time, Config};

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub name: String,
    pub repository: String,
    pub version: String,
    pub commit_hash: String,
    pub last_index_snapshot: u64,
    pub base_file_url: String,
}

impl ServerInfo {
    pub async fn new() -> Self {
        let config = Config::from_env();
        let last_index_snapshot = get_last_rdb_save_time().await.unwrap_or_default();
        Self {
            description: env!("CARGO_PKG_DESCRIPTION").to_string(),
            homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
            license: env!("CARGO_PKG_LICENSE").to_string(),
            name: env!("CARGO_PKG_NAME").to_string(),
            repository: env!("CARGO_PKG_REPOSITORY").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            commit_hash: option_env!("GIT_COMMIT_HASH")
                .unwrap_or("unknown")
                .to_string(),
            last_index_snapshot,
            base_file_url: config.base_file_url,
        }
    }
}
