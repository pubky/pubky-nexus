use super::traits::{HashId, Validatable};
use axum::async_trait;
use serde::{Deserialize, Serialize};

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
