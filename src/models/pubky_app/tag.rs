use super::traits::{HashId, Validatable};
use axum::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

// Validation
const MAX_TAG_LABEL_LENGTH: usize = 20;

/// Represents raw homeserver tag with id
/// URI: /pub/pubky.app/tags/:tag_id
///
/// Example URI:
///
/// `/pub/pubky.app/tags/xsmykwj3jdzdwbox6bu5yjowzw`
///
/// Where tag_id is z-base32(Sha256("{uri_tagged}:{")))[:8]
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppTag {
    pub uri: String,
    pub label: String,
    pub created_at: i64,
}

#[async_trait]
impl HashId for PubkyAppTag {
    /// Tag ID is created based on the hash of the URI tagged and the label used
    fn get_id_data(&self) -> String {
        format!("{}:{}", self.uri, self.label)
    }
}

#[async_trait]
impl Validatable for PubkyAppTag {
    async fn sanitize(self) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Convert label to lowercase and trim
        let label = self.label.trim().to_lowercase();

        // Enforce maximum label length
        let label = if label.len() > MAX_TAG_LABEL_LENGTH {
            label[..MAX_TAG_LABEL_LENGTH].to_string()
        } else {
            label
        };

        // Sanitize URI
        let uri = match Url::parse(&self.uri) {
            Ok(url) => url.to_string(),
            Err(_) => return Err("Invalid URI in tag".into()),
        };

        Ok(PubkyAppTag {
            uri,
            label,
            created_at: self.created_at,
        })
    }

    async fn validate(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.validate_id(id).await?;

        // Validate label length
        if self.label.len() > MAX_TAG_LABEL_LENGTH {
            return Err("Tag label exceeds maximum length".into());
        }

        // TODO: more validation?

        Ok(())
    }
}

#[test]
fn test_create_id() {
    let tag = PubkyAppTag {
        uri: "user_id/pub/pubky.app/posts/post_id".to_string(),
        created_at: 1627849723,
        label: "cool".to_string(),
    };

    let tag_id = tag.create_id();
    println!("Generated Tag ID: {}", tag_id);
}
