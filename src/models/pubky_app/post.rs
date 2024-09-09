use super::traits::{GenerateRandomId, Validatable};
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Represents the type of pubky-app posted data
/// Used primarily to best display the content in UI
#[derive(Serialize, Deserialize, ToSchema, Default, Debug)]
pub enum PostKind {
    #[default]
    Short,
    Long,
    Image,
    Video,
    Link,
    File,
}

impl fmt::Display for PostKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PostKind::Short => write!(f, "Short"),
            PostKind::Long => write!(f, "Long"),
            PostKind::Image => write!(f, "Image"),
            PostKind::Video => write!(f, "Video"),
            PostKind::Link => write!(f, "Link"),
            PostKind::File => write!(f, "File"),
        }
    }
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
///
/// Example URI:
///
/// `/pub/pubky.app/posts/00321FCW75ZFY`
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppPost {
    pub content: String,
    pub kind: PostKind,
    pub embed: Option<PostEmbed>,
}

impl GenerateRandomId for PubkyAppPost {}

impl Validatable for PubkyAppPost {
    //TODO: implement full validation rules. Min/Max lengths, post kinds, etc.
    async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
