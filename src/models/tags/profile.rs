use tokio::time::{sleep, Duration};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use crate::db::connectors::neo4j::get_neo4j_graph;
use crate::db::graph::queries;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthorInfo {
    #[serde(default)]
    uri: String,
    #[serde(default)]
    image: String
}

impl Default for AuthorInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a profile link with a title and URL.
impl AuthorInfo {
    pub fn new() -> Self {
        Self {
            uri: String::new(),
            image: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct TaggedFrom {
    tag_id: String, // TODO: Crobfordbase32 type
    indexed_at: String,
    author: AuthorInfo,
    profile_id: String,
}

impl Default for TaggedFrom {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a profile link with a title and URL.
impl TaggedFrom {
    pub fn new() -> Self {
        Self {
            tag_id: String::new(),
            indexed_at: String::new(),
            author: AuthorInfo::default(),
            profile_id: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileTag {
    from: Vec<TaggedFrom>,
    label: String
}

impl ProfileTag {
    pub async fn get_by_id(user_id: &str) -> Result<Option<Vec<ProfileTag>>, Box<dyn std::error::Error + Send + Sync>> {

        let query = queries::user_tags(user_id);
        let graph = get_neo4j_graph()?;

        let graph = graph.lock().await;
        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let tagged_from: Vec<ProfileTag> = row.get("user_tags").unwrap();
            return Ok(Some(tagged_from))
        }
        return Ok(None)
    }
}