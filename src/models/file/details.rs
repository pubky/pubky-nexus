use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::{queries, Config, RedisOps};
use chrono::Utc;
use neo4rs::Node;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    main: String,
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct FileKey {
    owner_id: String,
    file_id: String,
}

/// Represents a file and its metadata, including links to the actual binary of the file.
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct FileDetails {
    pub id: String,
    pub uri: String,
    pub owner_id: String,
    pub indexed_at: i64,
    pub created_at: i64,
    pub src: String,
    pub size: u64,
    pub content_type: String,
    pub urls: FileUrls,
}

impl RedisOps for FileDetails {}

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
            size: 0,
            created_at: Utc::now().timestamp(),
            indexed_at: Utc::now().timestamp(),
            content_type: String::new(),
        }
    }

    pub fn file_key_from_uri(uri: &str) -> FileKey {
        let path = uri.replace("pubky:", "");
        let parts: Vec<&str> = path.split("/").collect();

        FileKey {
            owner_id: String::from(parts[0]),
            file_id: String::from(parts[parts.len() - 1]),
        }
    }

    /// Retrieves file details by file uri.
    pub async fn get_file(
        key: &FileKey,
    ) -> Result<Option<FileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        match Self::try_from_index_json(&[&key.owner_id, &key.file_id]).await? {
            Some(details) => Ok(Some(details)),
            None => Self::get_from_graph(key).await,
        }
    }

    /// Retrieves a list of file details by a list of file uris.
    pub async fn get_files(
        keys: Vec<&FileKey>,
    ) -> Result<Vec<FileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let pairs = keys
            .clone()
            .into_iter()
            .map(|key| [key.owner_id.as_str(), key.file_id.as_str()])
            .collect::<Vec<[&str; 2]>>();

        let pair_slices: Vec<&[&str]> = pairs.iter().map(|arr| &arr[..]).collect();

        let mut files: Vec<FileDetails> = Self::try_from_index_multiple_json(&pair_slices)
            .await?
            .into_iter()
            .filter_map(|x| x)
            .collect::<Vec<FileDetails>>();

        let cache_miss_uris: Vec<[&str; 2]> = keys
            .into_iter()
            .filter_map(|x| {
                match files
                    .iter()
                    .find(|file| file.owner_id == x.owner_id && file.id == x.file_id)
                {
                    Some(_) => None,
                    None => Some([x.owner_id.as_str(), x.file_id.as_str()]),
                }
            })
            .collect();

        let cache_miss_files: Vec<FileDetails> =
            Self::get_many_from_graph(cache_miss_uris.iter().map(|x| &x[..]).collect()).await?;

        files.extend(cache_miss_files);
        Ok(files)
    }

    async fn from_node(node: &Node) -> Self {
        Self {
            uri: node.get("uri").unwrap_or_default(),
            id: node.get("id").unwrap_or_default(),
            indexed_at: node.get("indexed_at").unwrap_or_default(),
            created_at: node.get("created_at").unwrap_or_default(),
            size: node.get("size").unwrap_or_default(),
            content_type: node.get("content_type").unwrap_or_default(),
            src: node.get("src").unwrap_or_default(),
            urls: Self::to_public_urls(&node.get("urls").unwrap_or_default()),
            owner_id: node.get("owner_id").unwrap_or_default(),
        }
    }

    fn to_public_urls(relative_urls: &FileUrls) -> FileUrls {
        let config = Config::from_env();
        FileUrls {
            main: config.base_file_url + &relative_urls.main,
        }
    }

    /// Retrieves the file fields from Neo4j.
    async fn get_from_graph(
        key: &FileKey,
    ) -> Result<Option<FileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get_file_by_id(&key.owner_id, &key.file_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("f")?;
                let file = Self::from_node(&node).await;
                file.put_index_json(&[&file.owner_id, &file.id]).await?;
                Ok(Some(file))
            }
            None => Ok(None),
        }
    }

    /// Retrieves the file fields from Neo4j.
    async fn get_many_from_graph(
        pairs: Vec<&[&str]>,
    ) -> Result<Vec<FileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::get_files_by_ids(pairs);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut nodes = vec![];

        while let Some(row) = result.next().await? {
            let node: Node = row.get("f")?;
            let file = Self::from_node(&node).await;
            file.put_index_json(&[&file.owner_id, &file.id]).await?;
            nodes.push(file);
        }
        Ok(nodes)
    }
}
