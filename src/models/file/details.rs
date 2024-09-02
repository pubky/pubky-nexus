use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::graph::exec::exec_single_row;
use crate::events::Event;
use crate::models::homeserver::HomeserverFile;
use crate::{queries, RedisOps};
use chrono::Utc;
use graph_node_macro::GraphNode;
use neo4rs::Node;
use pubky::PubkyClient;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct FileUrls {
    pub main: String,
}

#[derive(Serialize, Deserialize, ToSchema, Default)]
pub struct FileKey {
    pub owner_id: String,
    pub file_id: String,
}

/// Represents a file and its metadata, including links to the actual binary of the file.
#[derive(Serialize, Deserialize, ToSchema, Default, GraphNode)]
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

struct FileMeta {
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

    pub async fn from_homeserver(
        event: &Event,
        homeserver_file: HomeserverFile,
        client: &PubkyClient,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let id = event.uri.path.split("/").last().unwrap();

        let file_meta = FileDetails::ingest(event, &homeserver_file, client).await?;

        Ok(FileDetails {
            name: homeserver_file.name,
            src: homeserver_file.src,
            content_type: homeserver_file.content_type,
            uri: event.uri.path.to_string(),
            id: id.to_string(),
            created_at: Utc::now().timestamp_millis(),
            indexed_at: Utc::now().timestamp_millis(),
            owner_id: event.user_id.to_string(),
            size: homeserver_file.size,
            urls: FileUrls {
                main: file_meta.urls.main,
            },
        })
    }

    // TODO: Move it into its own process, server, etc
    async fn ingest(
        event: &Event,
        homeserver_file: &HomeserverFile,
        client: &PubkyClient,
    ) -> Result<FileMeta, Box<dyn std::error::Error + Send + Sync>> {
        let id = event.uri.path.split("/").last().unwrap();
        let static_path = format!("{}/{}", event.user_id, id);

        let response = client.get(homeserver_file.src.as_str()).await?.unwrap();

        let mut static_file = File::create(format!("static/files/{}", &static_path)).await?;

        static_file.write_all(&response).await?;

        Ok(FileMeta {
            urls: FileUrls { main: static_path },
        })
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

    /// Retrieves the file fields from Neo4j.
    async fn get_from_graph(
        key: &FileKey,
    ) -> Result<Option<FileDetails>, Box<dyn std::error::Error + Send + Sync>> {
        let mut result;
        {
            let graph = get_neo4j_graph()?;
            let query = queries::read::get_file_by_id(&key.owner_id, &key.file_id);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        match result.next().await? {
            Some(row) => {
                let node: Node = row.get("f")?;
                let file = Self::from_node(&node);
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
            let query = queries::read::get_files_by_ids(pairs);

            let graph = graph.lock().await;
            result = graph.execute(query).await?;
        }

        let mut nodes = vec![];

        while let Some(row) = result.next().await? {
            let node: Node = row.get("f")?;
            let file = Self::from_node(&node);
            file.put_index_json(&[&file.owner_id, &file.id]).await?;
            nodes.push(file);
        }
        Ok(nodes)
    }
}
