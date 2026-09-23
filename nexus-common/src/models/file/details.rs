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
use std::path::Path;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
    pub hero: Option<String>,
    pub feed: Option<String>,
    pub small: Option<String>,
}

impl FileUrls {
    /// Creates a new instance by constructing URLs for file variants
    ///
    /// # Arguments
    /// * `base_path` - A reference to a `PathBuf` representing the base directory where files are stored
    /// * `variants` - A slice of `FileVariant` values representing the available file variants
    pub fn new(base_path: &Path, variants: &[FileVariant]) -> Self {
        let build_url = |variant: &FileVariant| {
            base_path
                .join(variant.to_string())
                .to_string_lossy()
                .into_owned()
        };

        Self {
            main: build_url(&FileVariant::Main),
            hero: variants
                .contains(&FileVariant::Hero)
                .then(|| build_url(&FileVariant::Hero)),
            feed: variants
                .contains(&FileVariant::Feed)
                .then(|| build_url(&FileVariant::Feed)),
            small: variants
                .contains(&FileVariant::Small)
                .then(|| build_url(&FileVariant::Small)),
        }
    }

    /// Fills in the derived variants a stored record predates.
    ///
    /// This list is written once, at ingestion, so a file indexed before a variant existed keeps
    /// the list it was written with: every file indexed before `hero` returns `hero: null`, even
    /// though the static route derives that variant on demand. Every variant of a file lives in
    /// the same directory as `main`, so a missing URL is `main` with its last segment swapped.
    ///
    /// Called on the way out of a read, never on the way in: the stored record is left alone.
    pub fn fill_derived_variants(&mut self, content_type: &str) {
        for variant in get_valid_variants_for_content_type(content_type) {
            let slot = match variant {
                FileVariant::Main => continue,
                FileVariant::Hero => &mut self.hero,
                FileVariant::Feed => &mut self.feed,
                FileVariant::Small => &mut self.small,
            };
            if slot.is_none() {
                *slot = Some(replace_last_path_segment(&self.main, &variant.to_string()));
            }
        }
    }
}

/// Swaps the last path segment of a variant URL: `.../0035R8SA18DE0/main` becomes `.../hero`.
/// A URL with no separator is returned unchanged, so a malformed record cannot panic a read.
fn replace_last_path_segment(url: &str, segment: &str) -> String {
    match url.rfind('/') {
        Some(index) => format!("{}/{}", &url[..index], segment),
        None => url.to_string(),
    }
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
            size: pubkyapp_file.size as i64,
            urls: meta.urls,
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

    /// A record written before `hero` existed: `main` and `feed`, nothing else.
    fn legacy_image_urls() -> FileUrls {
        FileUrls {
            main: "/static/files/user/file/main".to_string(),
            hero: None,
            feed: Some("/static/files/user/file/feed".to_string()),
            small: None,
        }
    }

    #[test]
    fn test_fill_derived_variants_backfills_what_a_legacy_record_predates() {
        let mut urls = legacy_image_urls();
        urls.fill_derived_variants("image/png");

        assert_eq!(urls.hero.as_deref(), Some("/static/files/user/file/hero"));
        assert_eq!(urls.small.as_deref(), Some("/static/files/user/file/small"));
        // A variant the record already carries keeps the URL it was written with.
        assert_eq!(urls.feed.as_deref(), Some("/static/files/user/file/feed"));
        assert_eq!(urls.main, "/static/files/user/file/main");
    }

    #[test]
    fn test_fill_derived_variants_leaves_a_content_type_without_variants_alone() {
        let mut urls = FileUrls {
            main: "/static/files/user/file/main".to_string(),
            ..Default::default()
        };
        urls.fill_derived_variants("video/mp4");

        assert!(urls.hero.is_none() && urls.feed.is_none() && urls.small.is_none());
    }

    #[test]
    fn test_replace_last_path_segment_without_a_separator_is_a_no_op() {
        assert_eq!(replace_last_path_segment("main", "hero"), "main");
    }
}
