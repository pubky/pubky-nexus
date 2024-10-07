use super::traits::Validatable;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// Validation
const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 50;
const MAX_BIO_LENGTH: usize = 160;
const MAX_IMAGE_LENGTH: usize = 300;
const MAX_LINKS: usize = 5;
const MAX_LINK_TITLE_LENGTH: usize = 100;
const MAX_LINK_URL_LENGTH: usize = 200;
const MAX_STATUS_LENGTH: usize = 50;

/// Profile schema
/// URI: /pub/pubky.app/profile.json
#[derive(Deserialize, Serialize, Debug)]
pub struct PubkyAppUser {
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

#[async_trait]
impl Validatable for PubkyAppUser {
    async fn validate(&self, _id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Validate name length
        if self.name.len() < MIN_USERNAME_LENGTH || self.name.len() > MAX_USERNAME_LENGTH {
            return Err("Invalid name length".into());
        }

        // Validate bio length
        if let Some(bio) = &self.bio {
            if bio.len() > MAX_BIO_LENGTH {
                return Err("Bio exceeds maximum length".into());
            }
        }

        // Validate image length
        if let Some(image) = &self.image {
            if image.len() > MAX_IMAGE_LENGTH {
                return Err("Image URI exceeds maximum length".into());
            }
        }

        // Validate links
        if let Some(links) = &self.links {
            if links.len() > MAX_LINKS {
                return Err("Too many links".into());
            }
            for link in links {
                if link.title.len() > MAX_LINK_TITLE_LENGTH || link.url.len() > MAX_LINK_URL_LENGTH
                {
                    return Err("Link title or URL too long".into());
                }
            }
        }

        // Validate status length
        if let Some(status) = &self.status {
            if status.len() > MAX_STATUS_LENGTH {
                return Err("Status exceeds maximum length".into());
            }
        }

        Ok(())
    }
}
