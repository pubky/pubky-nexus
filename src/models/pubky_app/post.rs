use super::traits::{TimestampId, Validatable};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

// Validation
const MAX_SHORT_CONTENT_LENGTH: usize = 1000;

/// Represents the type of pubky-app posted data
/// Used primarily to best display the content in UI
#[derive(Serialize, Deserialize, ToSchema, Default, Debug, Clone)]
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
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PostEmbed {
    pub kind: PostKind,
    pub uri: String, // If a repost a `Short` and uri of the reposted post.
}

/// Represents raw post in homeserver with content and kind
/// URI: /pub/pubky.app/posts/:post_id
/// Where post_id is CrockfordBase32 encoding of timestamp
///
/// Example URI:
///
/// `/pub/pubky.app/posts/00321FCW75ZFY`
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PubkyAppPost {
    pub content: String,
    pub kind: PostKind,
    pub parent: Option<String>, // If a reply, the URI of the parent post.
    pub embed: Option<PostEmbed>,
}

impl TimestampId for PubkyAppPost {}

#[async_trait]
impl Validatable for PubkyAppPost {
    //TODO: implement full validation rules. Min/Max lengths, post kinds, etc.
    async fn validate(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.validate_id(id).await?;

        if let PostKind::Short = &self.kind {
            if self.content.len() > MAX_SHORT_CONTENT_LENGTH {
                return Err("Post content exceeds maximum length for Short kind".into());
            }
        }
        // TODO: additional validation?
        Ok(())
    }
}
