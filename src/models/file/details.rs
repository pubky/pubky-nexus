use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::PubkyAppFile;
use crate::models::traits::Collection;
use crate::{queries, RedisOps};
use axum::async_trait;
use chrono::Utc;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
}

/// Represents a file and its metadata, including links to the actual binary of the file.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileDetails {
    pub id: String,
    pub uri: String,
    pub owner_id: String,
    pub indexed_at: i64,
    pub created_at: i64,
    pub src: String,
    pub name: String,
    pub size: u64,
    pub content_type: String,
    pub urls: FileUrls,
}

pub struct FileMeta {
    pub urls: FileUrls,
}

impl RedisOps for FileDetails {}

#[async_trait]
impl Collection<&[&str]> for FileDetails {
    fn graph_query(id_list: &[&[&str]]) -> Query {
        queries::read::get_files_by_ids(id_list)
    }

    fn to_graph_query(&self) -> Result<Query, Box<dyn std::error::Error + Send + Sync>> {
        queries::write::create_file(self)
    }

    async fn extend_on_index_miss(_: &[std::option::Option<Self>]) {
        return;
    }
}

impl FileDetails {
    pub fn new() -> Self {
        Self {
            id: String::new(),
            uri: String::new(),
            owner_id: String::new(),
            urls: FileUrls {
                main: String::new(),
            },
            src: String::new(),
            name: String::new(),
            size: 0,
            created_at: Utc::now().timestamp(),
            indexed_at: Utc::now().timestamp(),
            content_type: String::new(),
        }
    }

    pub fn from_homeserver(
        pubkyapp_file: &PubkyAppFile,
        uri: String,
        user_id: String,
        file_id: String,
        meta: FileMeta,
    ) -> Self {
        Self {
            name: pubkyapp_file.name.clone(),
            src: pubkyapp_file.src.clone(),
            content_type: pubkyapp_file.content_type.clone(),
            uri,
            id: file_id,
            created_at: pubkyapp_file.created_at,
            indexed_at: Utc::now().timestamp_millis(),
            owner_id: user_id.to_string(),
            size: pubkyapp_file.size,
            urls: meta.urls,
        }
    }

    pub async fn delete(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Delete on Redis
        Self::remove_from_index_multiple_json(&[&[&self.owner_id, &self.id]]).await?;

        // Delete graph node;
        exec_single_row(queries::write::delete_file(&self.owner_id, &self.id)).await?;

        Ok(())
    }

    pub fn file_key_from_uri(uri: &str) -> Vec<String> {
        let path = uri.replace("pubky:", "");
        let parts: Vec<&str> = path.split("/").collect();

        vec![String::from(parts[0]), String::from(parts[parts.len() - 1])]
    }
}