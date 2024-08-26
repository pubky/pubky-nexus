use axum::body::Bytes;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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

impl HomeserverUser {
    pub async fn try_from(blob: &Bytes) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let user: Self = serde_json::from_slice(blob)?;
        user.validate().await?;
        Ok(user)
    }

    //TODO: implement full validation rules. Min/Max length of links, of bio, of username, etc.
    pub async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
