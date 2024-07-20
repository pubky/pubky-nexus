use pk_social_common::{
    connectors::{
        neo4j::{get_neo4j_graph, Node},
        redis::{get_redis_conn, AsyncCommands},
    },
    queries,
};
use serde::{Deserialize, Serialize};
use tokio::join;
use utoipa::ToSchema;

use super::relationship::Relationship;

const PROFILE_PREFIX: &str = "profile!";

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Link {
    title: String,
    url: String,
}

impl Default for Link {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a profile link with a title and URL.
impl Link {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

/// Represents profile data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileData {
    name: String,
    bio: String,
    image: String,
    links: Vec<Link>,
    status: String,
}

impl Default for ProfileData {
    fn default() -> Self {
        Self::new()
    }
}

impl ProfileData {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            bio: String::new(),
            image: String::new(),
            links: vec![Link::new()],
            status: String::new(),
        }
    }

    /// Creates a `ProfileData` instance from a Neo4j `Node`.
    pub fn from_node(node: &Node) -> Self {
        Self {
            name: node.get("name").unwrap_or_default(),
            bio: node.get("bio").unwrap_or_default(),
            image: node.get("image").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
            // links: node.get("links").unwrap_or_default(), // do not yet exist on neo4j-example
            links: vec![Link::new()],
        }
    }
}

/// Represents a tag author with a URI, ID, and profile data.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Author {
    uri: String,
    id: String,
    profile: ProfileData,
}

impl Default for Author {
    fn default() -> Self {
        Self::new()
    }
}

impl Author {
    pub fn new() -> Self {
        Self {
            uri: String::new(),
            id: String::new(),
            profile: ProfileData::new(),
        }
    }
}

/// Represents a Tag source with an author, creation time, indexing time, and ID.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct From {
    author: Author,
    created_at: i64,
    indexed_at: i64,
    id: String,
}

impl Default for From {
    fn default() -> Self {
        Self::new()
    }
}

impl From {
    pub fn new() -> Self {
        Self {
            author: Author::new(),
            created_at: 0,
            indexed_at: 0,
            id: String::new(),
        }
    }
}

/// Represents a tag with it's tag label, count, and author sources.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Tag {
    tag: String,
    count: u32,
    from: Vec<From>,
}

impl Default for Tag {
    fn default() -> Self {
        Self::new()
    }
}

impl Tag {
    pub fn new() -> Self {
        Self {
            tag: String::new(),
            count: 0,
            from: vec![From::new()],
        }
    }
}

/// Represents a Pubky user profile with relational data including tags, counts, and relationship with a viewer.
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Profile {
    profile: ProfileData,
    tags_count: u32,
    posts_count: u32,
    followers_count: u32,
    following_count: u32,
    friends_count: u32,
    tagged_as: Vec<Tag>,
    viewer: Relationship,
}

impl Default for Profile {
    fn default() -> Self {
        Self::new()
    }
}

impl Profile {
    pub fn new() -> Self {
        Self {
            profile: ProfileData::new(),
            tags_count: 0,
            posts_count: 0,
            followers_count: 0,
            following_count: 0,
            friends_count: 0,
            tagged_as: vec![Tag::new()],
            viewer: Relationship::new(),
        }
    }

    /// Retrieves a profile by user ID, checking the cache first and then the graph database.
    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        // Concurrent relationship and profile retrieval
        // Get the relationship information
        let relationship = match viewer_id {
            Some(v_id) => Relationship::get(user_id, v_id).await?.unwrap_or_default(),
            None => Relationship::new(),
        };

        // Try to get from indexed cache
        if let Some(mut indexed_profile) = Self::get_from_index(user_id).await? {
            indexed_profile.viewer = relationship;
            return Ok(Some(indexed_profile));
        }

        // Fallback to query from graph
        match Self::get_from_graph(user_id).await? {
            Some(mut profile) => {
                profile.viewer = relationship;
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    /// Retrieves a profile from Neo4j, processes various queries, and caches the result in Redis.
    async fn get_from_index(user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_PREFIX}{user_id}");

        if let Ok(cached_profile) = redis_conn.get::<_, String>(&cache_key).await {
            let profile: Profile = serde_json::from_str(&cached_profile)?;

            // println!("Found profile index {cache_key}");
            return Ok(Some(profile));
        }

        Ok(None)
    }

    /// Retrieves a profile from Neo4j, processes various queries, and caches the result in Redis.
    async fn get_from_graph(user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let graph = get_neo4j_graph()?;

        // Define all queries
        let user_query = queries::get_user_by_id(user_id);
        let follow_counts_query = queries::get_follow_counts(user_id);
        let tagged_as_query = queries::get_tagged_as(user_id);

        // Execute all queries concurrently
        let graph = graph.lock().await;
        let (user_result, follow_counts_result, tagged_as_result) = join!(
            graph.execute(user_query),
            graph.execute(follow_counts_query),
            graph.execute(tagged_as_query),
        );

        // Handle results
        let mut user_result = user_result?;
        let mut follow_counts_result = follow_counts_result?;
        let mut tagged_as_result = tagged_as_result?;

        // Exit early if user not found
        let user_row = if let Some(row) = user_result.next().await? {
            row
        } else {
            return Ok(None);
        };

        let mut profile = Self::new();
        let node: Node = user_row.get("u").unwrap();
        profile.profile = ProfileData::from_node(&node);

        // Process follow counts result
        if let Some(row) = follow_counts_result.next().await? {
            profile.following_count = row.get("following_count").unwrap_or_default();
            profile.followers_count = row.get("followers_count").unwrap_or_default();
            profile.friends_count = row.get("friends_count").unwrap_or_default();
        }

        // Process tagged as result
        while let Some(row) = tagged_as_result.next().await? {
            let tag: String = row.get("tag").unwrap_or_default();
            let count: u32 = row.get("count").unwrap_or_default();

            let authors: Vec<Author> = row
                .get::<Vec<Node>>("authors")
                .unwrap_or_default()
                .into_iter()
                .map(|author_node| Author {
                    uri: author_node.get("uri").unwrap_or_default(),
                    id: author_node.get("id").unwrap_or_default(),
                    profile: ProfileData::from_node(&author_node),
                })
                .collect();

            let tag_info = Tag {
                tag,
                count,
                from: authors
                    .into_iter()
                    .map(|author| From {
                        author,
                        created_at: 0,     // TODO: populate this field as needed
                        indexed_at: 0,     // TODO: populate this field as needed
                        id: String::new(), // TODO: populate this field as needed
                    })
                    .collect(),
            };

            profile.tagged_as.push(tag_info);
        }

        // Graph queries are expensive, so we save it to the index as cache.
        Self::set_index(user_id, &profile).await?;

        Ok(Some(profile))
    }

    /// Sets the profile in the Redis cache.
    async fn set_index(user_id: &str, profile: &Profile) -> Result<(), Box<dyn std::error::Error>> {
        let mut redis_conn = get_redis_conn().await?;
        let cache_key = format!("{PROFILE_PREFIX}{user_id}");

        let profile_json = serde_json::to_string(&profile)?;

        redis_conn.set_ex(&cache_key, profile_json, 3600).await?;
        // println!("Saved profile index {cache_key}");

        Ok(())
    }
}
