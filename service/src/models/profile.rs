use pk_social_common::{
    connectors::{
        neo4j::{Node, NEO4J_CONNECTOR},
        redis::{AsyncCommands, REDIS_CONNECTOR},
    },
    queries,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Link {
    title: String,
    url: String,
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

        if let Some(profile) = Self::get_from_graph(user_id, viewer_id).await? {
            return Ok(Some(profile));
        }

        Ok(None)
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

        let query = queries::get_user_by_id(user_id);
        let mut result = graph.execute(query).await?;

        // Return early if no row is found
        let row = if let Some(row) = result.next().await? {
            row
        } else {
            return Ok(None);
        };

        let mut profile = Self::new();
        let node: Node = row.get("u").unwrap();
        profile.profile = ProfileData::from_node(&node);

        // Execute the get_follow_counts query
        let follow_counts_query = queries::get_follow_counts(user_id);
        let mut follow_counts_result = graph.execute(follow_counts_query).await?;

        if let Some(follow_counts_row) = follow_counts_result.next().await? {
            profile.following_count = follow_counts_row.get("following_count").unwrap_or_default();
            profile.followers_count = follow_counts_row.get("followers_count").unwrap_or_default();
            profile.friends_count = follow_counts_row.get("friends_count").unwrap_or_default();
        }

        // Execute the get_tagged_as query
        let tagged_as_query = queries::get_tagged_as(user_id);
        let mut tagged_as_result = graph.execute(tagged_as_query).await?;

        while let Some(tagged_as_row) = tagged_as_result.next().await? {
            let tag: String = tagged_as_row.get("tag").unwrap_or_default();
            let count: u32 = tagged_as_row.get("count").unwrap_or_default();

            let authors: Vec<Author> = tagged_as_row
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
                        created_at: 0,     // populate this field as needed
                        indexed_at: 0,     // populate this field as needed
                        id: String::new(), // populate this field as needed
                    })
                    .collect(),
            };

            profile.tagged_as.push(tag_info);
        }

        // Execute the is_following and is_followed_by queries if viewer_id is provided
        if let Some(viewer_id) = viewer_id {
            let is_following_query = queries::is_following(user_id, viewer_id);
            let mut is_following_result = graph.execute(is_following_query).await?;

            if let Some(is_following_row) = is_following_result.next().await? {
                profile.viewer.following = is_following_row.get("following").unwrap_or(false);
            }

            let is_followed_by_query = queries::is_followed_by(user_id, viewer_id);
            let mut is_followed_by_result = graph.execute(is_followed_by_query).await?;

            if let Some(is_followed_by_row) = is_followed_by_result.next().await? {
                profile.viewer.followed_by = is_followed_by_row.get("followed_by").unwrap_or(false);
            }
        }
        // Graph queries are expensive, we save it to the index as cache.
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
