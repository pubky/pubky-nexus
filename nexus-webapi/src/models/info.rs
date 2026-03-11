use std::path::PathBuf;

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use nexus_common::db::kv::get_last_rdb_save_time;

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub name: String,
    pub repository: String,
    pub version: String,
    pub commit_hash: String,
    pub last_index_snapshot: String,
    pub base_file_url: String,
}

impl ServerInfo {
    pub async fn new(base_file_path: PathBuf) -> Self {
        let last_index_snapshot = Self::get_index_snapshot().await;

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
            base_file_url: base_file_path.to_string_lossy().into_owned(),
        }
    }

    async fn get_index_snapshot() -> String {
        let last_index_snapshot_in_secs: i64 = get_last_rdb_save_time()
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or_default();
        // Convert the seconds in milliseconds
        let datetime: DateTime<Utc> = Utc
            .timestamp_millis_opt(last_index_snapshot_in_secs * 1000)
            .single()
            .expect("Invalid timestamp");
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    }
}
