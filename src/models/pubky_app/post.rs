use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the type of pubky-app posted data
/// Used primarily to best display the content in UI
#[derive(Serialize, Deserialize, ToSchema, Default)]
pub enum PostKind {
    #[default]
    Short,
    Long,
    Image,
    Video,
    Link,
    File,
}

/// Used primarily to best display the content in UI
#[derive(Serialize, Deserialize, Default)]
pub struct PostEmbed {
    pub r#type: String, //e.g., "post", we have to define a type for this.
    pub uri: String,
}

/// Represents raw post in homeserver with content and kind
/// URI: /pub/pubky.app/posts/:post_id
/// Where post_id is CrockfordBase32 encoding of timestamp
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppPost {
    pub content: String,
    pub kind: PostKind,
    pub embed: PostEmbed,
}
