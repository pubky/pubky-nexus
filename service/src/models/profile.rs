use pk_social_common::{
    connectors::{
        neo4j::{Node, NEO4J_CONNECTOR},
        redis::{AsyncCommands, REDIS_CONNECTOR},
    },
    queries,
};
use serde::{Deserialize, Serialize};
use tokio::join;
use utoipa::ToSchema;

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

impl Link {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
        }
    }
}

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

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Viewer {
    following: bool,
    followed_by: bool,
}

impl Default for Viewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewer {
    pub fn new() -> Self {
        Self {
            following: false,
            followed_by: false,
        }
    }
}

const PROFILE_PREFIX: &str = "profile:";
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Profile {
    profile: ProfileData,
    tags_count: u32,
    posts_count: u32,
    followers_count: u32,
    following_count: u32,
    friends_count: u32,
    tagged_as: Vec<Tag>,
    viewer: Viewer,
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
            viewer: Viewer::new(),
        }
    }

    pub async fn get_by_id(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        if let Some(indexed_profile) = Self::get_indexed(user_id).await? {
            return Ok(Some(indexed_profile));
        }

        Self::get_from_graph(user_id, viewer_id).await
    }

    async fn get_indexed(user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let redis_client = REDIS_CONNECTOR
            .get()
            .expect("RedisConnector not initialized")
            .client();
        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cache_key = format!("{PROFILE_PREFIX}{user_id}");

        if let Ok(cached_profile) = redis_conn.get::<_, String>(&cache_key).await {
            let profile: Profile = serde_json::from_str(&cached_profile)?;

            // println!("Found {cache_key} on index");
            return Ok(Some(profile));
        }

        Ok(None)
    }

    async fn get_from_graph(
        user_id: &str,
        viewer_id: Option<&str>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let graph = NEO4J_CONNECTOR
            .get()
            .expect("Neo4jConnector not initialized")
            .graph
            .get()
            .expect("Not connected to Neo4j");

        // Define all queries
        let user_query = queries::get_user_by_id(user_id);
        let follow_counts_query = queries::get_follow_counts(user_id);
        let tagged_as_query = queries::get_tagged_as(user_id);
        let viewer_relationship_query =
            queries::viewer_relationship(user_id, viewer_id.unwrap_or("non-existing-pk"));

        // Execute all queries concurrently
        let (user_result, follow_counts_result, tagged_as_result, viewer_relationship_result) = join!(
            graph.execute(user_query),
            graph.execute(follow_counts_query),
            graph.execute(tagged_as_query),
            graph.execute(viewer_relationship_query),
        );

        // Handle results
        let mut user_result = user_result?;
        let mut follow_counts_result = follow_counts_result?;
        let mut tagged_as_result = tagged_as_result?;
        let mut viewer_relationship_result = viewer_relationship_result?;

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

        // Process viewer relationship result if viewer_id is provided
        if viewer_id.is_some() {
            if let Some(row) = viewer_relationship_result.next().await? {
                profile.viewer.following = row.get("following").unwrap_or_default();
                profile.viewer.followed_by = row.get("followed_by").unwrap_or_default();
            }
        }

        // Graph queries are expensive, so we save it to the index as cache.
        Self::put_index(user_id, &profile).await?;

        Ok(Some(profile))
    }

    async fn put_index(user_id: &str, profile: &Profile) -> Result<(), Box<dyn std::error::Error>> {
        let redis_client = REDIS_CONNECTOR
            .get()
            .expect("RedisConnector not initialized")
            .client();

        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cache_key = format!("{PROFILE_PREFIX}{user_id}");

        let profile_json = serde_json::to_string(&profile)?;

        redis_conn.set_ex(&cache_key, profile_json, 3600).await?;
        // println!("Saved {cache_key} to index");

        Ok(())
    }
}
