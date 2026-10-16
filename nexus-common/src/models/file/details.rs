use crate::db::graph::{GraphResult, Query};
use crate::db::kv::RedisResult;
use crate::db::{exec_single_row, queries, RedisOps};
use crate::media::{get_valid_variants_for_content_type, FileVariant};
use crate::models::error::ModelResult;
use crate::models::traits::Collection;
use async_trait::async_trait;
use chrono::Utc;
use pubky_app_specs::{ParsedUri, PubkyAppFile, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
    pub large: Option<String>,
    pub feed: Option<String>,
    pub small: Option<String>,
}

impl FileUrls {
    /// Every variant URL a file has. They depend only on the owner, the id and the content type,
    /// so they are rebuilt on each read rather than trusted from storage, where a list written
    /// before a variant existed would go stale.
    pub fn new(owner_id: &str, file_id: &str, content_type: &str) -> Self {
        let variants = get_valid_variants_for_content_type(content_type);
        let url = |variant: FileVariant| format!("{owner_id}/{file_id}/{variant}");
        let derived = |variant: FileVariant| variants.contains(&variant).then(|| url(variant));

        Self {
            main: url(FileVariant::Main),
            large: derived(FileVariant::Large),
            feed: derived(FileVariant::Feed),
            small: derived(FileVariant::Small),
        }
    }
}

mod json_string {
    use serde::{self, Serializer};

    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: serde::Serialize,
    {
        let json_string = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&json_string)
    }
}

/// Represents a file and its metadata, including links to the actual binary of the file.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
#[serde(from = "StoredFileDetails")]
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
    #[serde(serialize_with = "json_string::serialize")]
    pub urls: FileUrls,
    pub metadata: Option<HashMap<String, String>>,
}

/// A file as the graph and the index hold it. Any stored `urls` is ignored: every read rebuilds
/// them, so all readers agree and a new variant needs no backfill.
#[derive(Deserialize)]
struct StoredFileDetails {
    id: String,
    uri: String,
    owner_id: String,
    indexed_at: i64,
    created_at: i64,
    src: String,
    name: String,
    size: i64,
    content_type: String,
    metadata: Option<HashMap<String, String>>,
}

impl From<StoredFileDetails> for FileDetails {
    fn from(stored: StoredFileDetails) -> Self {
        Self {
            urls: FileUrls::new(&stored.owner_id, &stored.id, &stored.content_type),
            id: stored.id,
            uri: stored.uri,
            owner_id: stored.owner_id,
            indexed_at: stored.indexed_at,
            created_at: stored.created_at,
            src: stored.src,
            name: stored.name,
            size: stored.size,
            content_type: stored.content_type,
            metadata: stored.metadata,
        }
    }
}

impl RedisOps for FileDetails {}

#[async_trait]
impl Collection<&[&str]> for FileDetails {
    fn collection_details_graph_query(id_list: &[&[&str]]) -> Query {
        queries::get::get_files_by_ids(id_list)
    }

    fn put_graph_query(&self) -> GraphResult<Query> {
        queries::put::create_file(self)
    }

    async fn extend_on_index_miss(_: &[std::option::Option<Self>]) -> RedisResult<()> {
        Ok(())
    }
}

impl FileDetails {
    pub fn from_homeserver(
        pubkyapp_file: &PubkyAppFile,
        uri: String,
        user_id: String,
        file_id: String,
    ) -> Self {
        Self {
            urls: FileUrls::new(&user_id, &file_id, &pubkyapp_file.content_type),
            name: pubkyapp_file.name.clone(),
            src: pubkyapp_file.src.clone(),
            content_type: pubkyapp_file.content_type.clone(),
            uri,
            id: file_id,
            created_at: pubkyapp_file.created_at,
            indexed_at: Utc::now().timestamp_millis(),
            owner_id: user_id.to_string(),
            size: pubkyapp_file.size as i64,
            metadata: None,
        }
    }

    pub async fn delete(&self) -> ModelResult<()> {
        exec_single_row(queries::del::delete_file(&self.owner_id, &self.id))
            .await
            .inspect_err(|e| tracing::error!("Graph file deletion, {}: {:?}", self.id, e))?;
        Self::remove_from_index_multiple_json(&[&[&self.owner_id, &self.id]])
            .await
            .inspect_err(|e| tracing::error!("Index file deletion, {}: {:?}", self.id, e))?;
        Ok(())
    }

    pub fn file_key_from_uri(uri: &str) -> Option<(String, String)> {
        let parsed_uri = ParsedUri::try_from(uri).ok()?;
        if let Resource::File(file_id) = parsed_uri.resource {
            Some((parsed_uri.user_id.to_string(), file_id))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(content_type: &str, urls: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "file",
            "uri": "",
            "owner_id": "owner",
            "indexed_at": 0,
            "created_at": 0,
            "src": "",
            "name": "",
            "size": 0,
            "content_type": content_type,
            "urls": urls,
            "metadata": null,
        })
    }

    // A record indexed before `large` existed, with the legacy variant-less `main`.
    #[test]
    fn test_stored_urls_are_rebuilt_on_read() {
        let file: FileDetails =
            serde_json::from_value(stored("image/png", r#"{"main":"owner/file"}"#))
                .expect("stored file");

        assert_eq!(file.urls.main, "owner/file/main");
        assert_eq!(file.urls.large.as_deref(), Some("owner/file/large"));
        assert_eq!(file.urls.feed.as_deref(), Some("owner/file/feed"));
        assert_eq!(file.urls.small.as_deref(), Some("owner/file/small"));
    }

    #[test]
    fn test_a_video_has_only_main() {
        let file: FileDetails =
            serde_json::from_value(stored("video/mp4", "{}")).expect("stored file");

        assert_eq!(file.urls.main, "owner/file/main");
        assert!(file.urls.large.is_none() && file.urls.feed.is_none() && file.urls.small.is_none());
    }
}
