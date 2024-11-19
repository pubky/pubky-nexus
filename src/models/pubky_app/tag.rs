use super::traits::{HashId, Validatable};
use crate::types::DynError;
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
/// `/pub/pubky.app/tags/FPB0AM9S93Q3M1GFY1KV09GMQM`
///
/// Where tag_id is Crockford-base32(Blake3("{uri_tagged}:{label}")[:half])
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
    async fn sanitize(self) -> Result<Self, DynError> {
        // Convert label to lowercase and trim
        let label = self.label.trim().to_lowercase();

        // Enforce maximum label length safely
        let label = label.chars().take(MAX_TAG_LABEL_LENGTH).collect::<String>();

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

    async fn validate(&self, id: &str) -> Result<(), DynError> {
        self.validate_id(id).await?;

        // Validate label length based on characters
        if self.label.chars().count() > MAX_TAG_LABEL_LENGTH {
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
