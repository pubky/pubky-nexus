pub mod details;
pub mod global;
pub mod post;
pub mod search;
pub mod stream;
pub mod traits;
pub mod user;

// TODO: Use all the structs in that away
pub use details::TagDetails;

use serde::Deserialize;
use std::fmt::Display;
use utoipa::ToSchema;

#[derive(Deserialize, Debug, ToSchema, Clone)]
pub enum TaggedType {
    Post,
    User,
}

impl Display for TaggedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaggedType::Post => write!(f, "Post"),
            TaggedType::User => write!(f, "User"),
        }
    }
}
