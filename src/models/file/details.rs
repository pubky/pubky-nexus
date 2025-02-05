use crate::db::graph::exec::exec_single_row;
use crate::models::traits::Collection;
use crate::types::DynError;
use crate::{queries, RedisOps};
use async_trait::async_trait;
use chrono::Utc;
use log::error;
use neo4rs::Query;
use pubky_app_specs::{ParsedUri, PubkyAppFile, Resource};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
}

mod json_string {
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: serde::Serialize,
    {
        let json_string = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_string)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: serde::de::DeserializeOwned,
    {
        let json_string = String::deserialize(deserializer)?;
        serde_json::from_str(&json_string).map_err(serde::de::Error::custom)
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

    fn put_graph_query(&self) -> Result<Query, DynError> {
        queries::put::create_file(self)
    }

    async fn extend_on_index_miss(_: &[std::option::Option<Self>]) -> Result<(), DynError> {
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

    pub async fn delete(&self) -> Result<(), DynError> {
        // Delete graph node;
        match exec_single_row(queries::del::delete_file(&self.owner_id, &self.id)).await {
            Ok(_) => {
                // Delete on Redis
                Self::remove_from_index_multiple_json(&[&[&self.owner_id, &self.id]]).await?;
            }
            Err(e) => {
                error!("File deletion: {:?}", e);
                return Err("File: We could not delete the file".into());
            }
        };
        Ok(())
    }

    pub fn file_key_from_uri(uri: &str) -> Vec<String> {
        let parsed_uri = ParsedUri::try_from(uri).unwrap_or_default();
        if let Resource::File(file_id) = parsed_uri.resource {
            vec![parsed_uri.user_id.to_string(), file_id]
        } else {
            vec![parsed_uri.user_id.to_string(), String::default()]
        }
    }
}
