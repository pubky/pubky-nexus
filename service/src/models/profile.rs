use pk_social_common::{
    connectors::neo4j::{Node, GLOBAL_NEO4J_CONNECTOR},
    queries,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct Link {
    title: String,
    url: String,
}

#[derive(Serialize, ToSchema)]
pub struct ProfileData {
    name: String,
    bio: String,
    image: String,
    links: Vec<Link>,
    status: String,
}

#[derive(Serialize, ToSchema)]
pub struct Author {
    uri: String,
    id: String,
    profile: ProfileData,
}

#[derive(Serialize, ToSchema)]
pub struct From {
    author: Author,
    created_at: i64,
    indexed_at: i64,
    id: String,
}

#[derive(Serialize, ToSchema)]
pub struct Tag {
    tag: String,
    count: u32,
    from: Vec<From>,
}
#[derive(Serialize, ToSchema)]
pub struct Viewer {
    following: bool,
    followed_by: bool,
}

#[derive(Serialize, ToSchema)]
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
        let graph = GLOBAL_NEO4J_CONNECTOR
            .get()
            .expect("Neo4jConnector not initialized")
            .graph
            .get()
            .expect("Not connected to Neo4j");

        let query = queries::get_user_by_id(user_id);

        let mut result = graph.execute(query).await?;

        if let Some(row) = result.next().await? {
            let node: Node = row.get("u").unwrap();
            Ok(Some(Self::from_neo4j_user_node(&node).await))
        } else {
            Ok(None)
        }
    }
}
