pub mod details;
pub mod external;
pub mod global;
pub mod post;
pub mod search;
pub mod stream;
pub mod traits;
pub mod user;
pub mod view;

// TODO: Use all the structs in that away
pub use details::TagDetails;

use serde::Deserialize;
use std::fmt::Display;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum TaggedType {
    Post,
    User,
    ExternalLink,
}

impl Display for TaggedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedType::Post => write!(f, "Post"),
            TaggedType::User => write!(f, "User"),
            TaggedType::ExternalLink => write!(f, "ExternalLink"),
        }
    }
}

pub type Taggers = Vec<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagDeletionTarget {
    User { tagged_id: String },
    Post { post_id: String, author_id: String },
    ExternalLink { link_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagDeletion {
    pub target: TagDeletionTarget,
    pub label: String,
}
