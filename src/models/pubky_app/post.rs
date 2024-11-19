use super::traits::{TimestampId, Validatable};
use crate::types::DynError;
use axum::async_trait;
use neo4rs::BoltType;
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

// Validation
const MAX_SHORT_CONTENT_LENGTH: usize = 1000;
const MAX_LONG_CONTENT_LENGTH: usize = 50000;

/// Represents the type of pubky-app posted data
/// Used primarily to best display the content in UI
#[derive(Serialize, Deserialize, ToSchema, Default, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PostKind {
    #[default]
    Short,
    Long,
    Image,
    Video,
    Link,
    File,
}

/// This implementation maps each `PostKind` variant to its string
/// representation (e.g., `PostKind::Short` to `"short"`) and then
/// converts it into a `BoltType`, which is used in Neo4j queries
/// via the `neo4rs` crate
impl From<PostKind> for BoltType {
    fn from(kind: PostKind) -> Self {
        let kind_str = match kind {
            PostKind::Short => "short",
            PostKind::Long => "long",
            PostKind::Image => "image",
            PostKind::Video => "video",
            PostKind::Link => "link",
            PostKind::File => "file",
        };
        BoltType::from(kind_str)
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
    pub attachments: Option<Vec<String>>,
}

impl TimestampId for PubkyAppPost {}

#[async_trait]
impl Validatable for PubkyAppPost {
    async fn sanitize(self) -> Result<Self, DynError> {
        // Sanitize content
        let mut content = self.content.trim().to_string();

        // We are using content keyword `[DELETED]` for deleted posts from a homeserver that still have relationships
        // placed by other users (replies, tags, etc). This content is exactly matched by the client to apply effects to deleted content.
        // Placing posts with content `[DELETED]` is not allowed.
        if content == *"[DELETED]" {
            content = "empty".to_string()
        }

        // Define content length limits based on PostKind
        let max_content_length = match self.kind {
            PostKind::Short => MAX_SHORT_CONTENT_LENGTH,
            PostKind::Long => MAX_LONG_CONTENT_LENGTH,
            _ => MAX_SHORT_CONTENT_LENGTH, // Default limit for other kinds
        };

        let content = content.chars().take(max_content_length).collect::<String>();

        // Sanitize parent URI if present
        let parent = if let Some(uri_str) = &self.parent {
            match Url::parse(uri_str) {
                Ok(url) => Some(url.to_string()), // Valid URI, use normalized version
                Err(_) => None,                   // Invalid URI, discard or handle appropriately
            }
        } else {
            None
        };

        // Sanitize embed if present
        let embed = if let Some(embed) = &self.embed {
            match Url::parse(&embed.uri) {
                Ok(url) => Some(PostEmbed {
                    kind: embed.kind.clone(),
                    uri: url.to_string(), // Use normalized version
                }),
                Err(_) => None, // Invalid URI, discard or handle appropriately
            }
        } else {
            None
        };

        Ok(PubkyAppPost {
            content,
            kind: self.kind,
            parent,
            embed,
            attachments: self.attachments,
        })
    }

    //TODO: implement full validation rules. Min/Max lengths, post kinds, etc.
    async fn validate(&self, id: &str) -> Result<(), DynError> {
        self.validate_id(id).await?;

        // Validate content length
        match self.kind {
            PostKind::Short => {
                if self.content.chars().count() > MAX_SHORT_CONTENT_LENGTH {
                    return Err("Post content exceeds maximum length for Short kind".into());
                }
            }
            PostKind::Long => {
                if self.content.chars().count() > MAX_LONG_CONTENT_LENGTH {
                    return Err("Post content exceeds maximum length for Short kind".into());
                }
            }
            _ => (),
        };

        // TODO: additional validation?

        Ok(())
    }
}
