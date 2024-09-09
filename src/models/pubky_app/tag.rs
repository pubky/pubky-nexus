use super::traits::{GenerateHashId, Validatable};
use serde::{Deserialize, Serialize};

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

impl GenerateHashId for PubkyAppTag {
    /// Tag ID is created based on the hash of the URI tagged and the label used
    fn get_id_data(&self) -> String {
        format!("{}:{}", self.uri, self.label)
    }
}

impl Validatable for PubkyAppTag {
    async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: validate ID and content of incoming tag is correct
        Ok(())
    }
}

#[test]
fn testcreate_id() {
    let tag = PubkyAppTag {
        uri: "user_id/pub/pubky.app/posts/post_id".to_string(),
        created_at: 1627849723,
        label: "cool".to_string(),
    };

    let tag_id = tag.create_id();
    println!("Generated Tag ID: {}", tag_id);
}
