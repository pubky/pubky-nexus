use std::collections::HashMap;
use std::fmt::Display;

use crate::db::graph::exec::exec_single_row;
use crate::models::pubky_app::PubkyAppFile;
use crate::models::traits::Collection;
use crate::{queries, RedisOps};
use axum::async_trait;
use chrono::Utc;
use neo4rs::Query;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, PartialEq)]
pub enum FileVersions {
    MAIN,
    FEED,
    SMALL,
}

impl FileVersions {
    pub fn from_str(input: &str) -> Option<FileVersions> {
        match input {
            "main" => Some(FileVersions::MAIN),
            "feed" => Some(FileVersions::FEED),
            "small" => Some(FileVersions::SMALL),
            _ => None,
        }
    }
}

impl Display for FileVersions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_string = match self {
            FileVersions::MAIN => "main",
            FileVersions::FEED => "feed",
            FileVersions::SMALL => "small",
        };
        write!(f, "{}", version_string)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
}

mod json_string {
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    // Deserialize function: convert the JSON string into a struct
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let json_str: &'de str = <&str>::deserialize(deserializer)?;
        serde_json::from_str(json_str).map_err(serde::de::Error::custom)
    }

    // Serialize function: convert the struct back into a JSON string
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        let json_str = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_str)
    }
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
    pub size: i64,
    pub content_type: String,
    #[serde(with = "json_string")]
    pub urls: FileUrls,
    pub metadata: Option<HashMap<String, String>>,
}

pub struct FileMeta {
    pub urls: FileUrls,
}

impl RedisOps for FileDetails {}

#[async_trait]
impl Collection<&[&str]> for FileDetails {
    fn collection_details_graph_query(id_list: &[&[&str]]) -> Query {
        queries::get::get_files_by_ids(id_list)
    }

    fn put_graph_query(&self) -> Result<Query, Box<dyn std::error::Error + Send + Sync>> {
        queries::put::create_file(self)
    }

    async fn extend_on_index_miss(
        _: &[std::option::Option<Self>],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
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
            metadata: None,
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
            metadata: None,
        }
    }

    pub async fn delete(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Delete on Redis
        Self::remove_from_index_multiple_json(&[&[&self.owner_id, &self.id]]).await?;

        // Delete graph node;
        exec_single_row(queries::del::delete_file(&self.owner_id, &self.id)).await?;

        Ok(())
    }

    pub fn file_key_from_uri(uri: &str) -> Vec<String> {
        let path = uri.replace("pubky://", "");
        let parts: Vec<&str> = path.split("/").collect();

        vec![String::from(parts[0]), String::from(parts[parts.len() - 1])]
    }
}
