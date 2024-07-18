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

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ProfileData {
    name: String,
    bio: String,
    image: String,
    links: Vec<Link>,
    status: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Author {
    uri: String,
    id: String,
    profile: ProfileData,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct From {
    author: Author,
    created_at: i64,
    indexed_at: i64,
    id: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct Tag {
    tag: String,
    count: u32,
    from: Vec<From>,
}
#[derive(Serialize, Deserialize, ToSchema)]
pub struct Viewer {
    following: bool,
    followed_by: bool,
}

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

impl ProfileData {
    pub fn from_neo4j_user_node(node: &Node) -> Self {
        Self {
            bio: node.get("bio").unwrap_or_default(),
            image: node.get("image").unwrap_or_default(),
            name: node.get("name").unwrap_or_default(),
            status: node.get("status").unwrap_or_default(),
            // TODO: placeholder for links (these don't yet exist in the Neo4J-Example)
            links: vec![
                Link {
                    title: "website".to_string(),
                    url: "https://example.com".to_string(),
                },
                Link {
                    title: "email".to_string(),
                    url: "mailto:example@example.com".to_string(),
                },
            ],
        }
    }
}

impl Profile {
    pub async fn from_neo4j_user_node(node: &Node) -> Self {
        // This is just a placeholder implementation. Replace with actual data extraction logic.
        Self {
            profile: ProfileData::from_neo4j_user_node(node),
            // TODO: placeholder for counts, tags and viewer relationships
            tags_count: 0,
            posts_count: 0,
            followers_count: 0,
            following_count: 0,
            friends_count: 0,
            tagged_as: vec![Tag {
                tag: "example_tag".to_string(),
                count: 1,
                from: vec![From {
                    author: Author {
                        uri: "example_uri".to_string(),
                        id: "example_id".to_string(),
                        profile: ProfileData {
                            name: "example_name".to_string(),
                            bio: "example_bio".to_string(),
                            image: "example_image".to_string(),
                            links: vec![
                                Link {
                                    title: "website".to_string(),
                                    url: "https://example.com".to_string(),
                                },
                                Link {
                                    title: "email".to_string(),
                                    url: "mailto:example@example.com".to_string(),
                                },
                            ],
                            status: "example_status".to_string(),
                        },
                    },
                    created_at: 1721316034032,
                    indexed_at: 1721316034051,
                    id: "2ZHNFM12N5E00".to_string(),
                }],
            }],
            viewer: Viewer {
                following: false,
                followed_by: false,
            },
        }
    }

    pub async fn get_by_id(user_id: &str) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        // REDIS AND CACHE
        let redis_client = REDIS_CONNECTOR
            .get()
            .expect("RedisConnector not initialized")
            .client();

        let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
        let cache_key = format!("profile:{}", user_id);

        // Check if the profile is cached in Redis
        if let Ok(cached_profile) = redis_conn.get::<_, String>(&cache_key).await {
            println!("CACHE FOR KEY {:?}, SERVING PROFILE FROM REDIS", cache_key);
            let profile: Profile = serde_json::from_str(&cached_profile)?;
            return Ok(Some(profile));
        }

        // NEO4J AND GRAPH
        let graph = NEO4J_CONNECTOR
            .get()
            .expect("Neo4jConnector not initialized")
            .graph
            .get()
            .expect("Not connected to Neo4j");

        let query = queries::get_user_by_id(user_id);

        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            println!("NO CACHE FOUND, SERVING PROFILE FROM NEO4J");
            let node: Node = row.get("u").unwrap();
            let profile = Self::from_neo4j_user_node(&node).await;

            // Cache the profile in Redis
            let profile_json = serde_json::to_string(&profile)?;
            redis_conn.set_ex(&cache_key, profile_json, 3600).await?;

            Ok(Some(profile))
        } else {
            Ok(None)
        }
    }
}
