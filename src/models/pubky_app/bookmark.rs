use super::traits::{GenerateId, Validatable};
use serde::{Deserialize, Serialize};

/// Represents raw homeserver bookmark with id
/// URI: /pub/pubky.app/bookmarks/:bookmark_id
///
/// Example URI:
///
/// `/pub/pubky.app/bookmarks/kx8uzgiq5f75bqofp51nq8r11r`
///
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppBookmark {
    pub uri: String,
    pub created_at: i64,
}

impl GenerateId for PubkyAppBookmark {
    /// Bookmark ID is created based on the hash of the URI bookmarked
    fn get_id_data(&self) -> String {
        self.uri.clone()
    }
}

impl Validatable for PubkyAppBookmark {
    fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: validate ID of incoming bookmark is correct
        Ok(())
    }
}

#[test]
fn test_create_bookmark_id() {
    let bookmark = PubkyAppBookmark {
        uri: "user_id/pub/pubky.app/posts/post_id".to_string(),
        created_at: 1627849723,
    };

    let bookmark_id = bookmark.create_id();
    println!("Generated Bookmark ID: {}", bookmark_id);
}
