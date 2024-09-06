use crate::db::graph::exec::exec_single_row;
use crate::models::traits::Collection;
use crate::{queries, RedisOps};
use axum::async_trait;
use chrono::Utc;
use graph_node_macro::GraphNode;
use neo4rs::{Node, Query};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
}

/// Represents a file and its metadata, including links to the actual binary of the file.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default, GraphNode)]
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

    async fn extend_on_cache_miss(_: &[std::option::Option<Self>]) {
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

    pub async fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Save on Redis
        self.put_index_json(&[&self.owner_id, &self.id]).await?;

        // Save graph node;
        exec_single_row(queries::write::create_file(self)).await?;

        Ok(())
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
