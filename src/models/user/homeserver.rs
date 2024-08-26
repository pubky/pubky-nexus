use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Raw schemas stored on homeserver.

/// Profile schema
#[derive(Deserialize, Serialize, Debug)]
pub struct HomeserverUser {
    pub name: String,
    pub bio: Option<String>,
    pub image: Option<String>,
    pub links: Option<Vec<UserLink>>,
    pub status: Option<String>,
}

/// Represents a user's single link with a title and URL.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
pub struct UserLink {
    pub title: String,
    pub url: String,
}
