mod bookmark;
mod counts;
mod details;
mod relationships;
mod stream;
mod view;

pub use bookmark::Bookmark;
pub use counts::PostCounts;
pub use details::PostDetails;
pub use relationships::PostRelationships;
pub use stream::{
    PostStream, StreamSource, POST_PER_USER_KEY_PARTS, POST_REPLIES_PER_POST_KEY_PARTS,
    POST_REPLIES_PER_USER_KEY_PARTS, POST_TIMELINE_KEY_PARTS, POST_TOTAL_ENGAGEMENT_KEY_PARTS,
};
pub use view::PostView;

#[derive(Debug, Clone, PartialEq)]
pub enum PostInteraction {
    Replies(String),
    Reposts(String),
}

impl PostInteraction {
    // We cannot use Serialize of Serde. It would serialise in {"Replies":"example_uri"}
    pub fn as_str(&self) -> &str {
        match self {
            PostInteraction::Replies(_) => "replies",
            PostInteraction::Reposts(_) => "reposts"
        }
    }
    pub fn get_uri(&self) -> &str {
        match self {
            PostInteraction::Replies(uri) | PostInteraction::Reposts(uri) => uri,
        }
    }
}