use super::traits::Validatable;
use axum::async_trait;
use log::error;
use serde::{Deserialize, Serialize};
use url::Url;
use utoipa::ToSchema;

// Validation
const MIN_USERNAME_LENGTH: usize = 3;
const MAX_USERNAME_LENGTH: usize = 50;
const MAX_BIO_LENGTH: usize = 160;
const MAX_IMAGE_LENGTH: usize = 300;
const MAX_LINKS: usize = 5;
const MAX_LINK_TITLE_LENGTH: usize = 100;
const MAX_LINK_URL_LENGTH: usize = 300;
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
    async fn sanitize(self) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let sanitized_name = self.name.trim().to_string();

        // Crop name to a maximum length of 50 characters
        let mut name = if sanitized_name.len() > MAX_USERNAME_LENGTH {
            sanitized_name[..MAX_USERNAME_LENGTH].to_string()
        } else {
            sanitized_name
        };

        // We use username keyword `[DELETED]` for a user whose `profile.json` has been deleted
        // Therefore this is not a valid username.
        if name == "[DELETED]".to_string() {
            name = "anonymous".to_string() //default username
        };

        // Sanitize bio
        let bio = self.bio.map(|b| {
            let trimmed = b.trim().to_string();
            if trimmed.len() > MAX_BIO_LENGTH {
                trimmed[..MAX_BIO_LENGTH].to_string()
            } else {
                trimmed
            }
        });

        // Sanitize image URL with URL parsing
        let image = match &self.image {
            Some(image_url) => {
                let sanitized_image_url = image_url.trim().to_string();

                match Url::parse(&sanitized_image_url) {
                    Ok(_) => Some(sanitized_image_url), // Valid image URL
                    Err(_) => None,                     // Invalid image URL, set to None
                }
            }
            None => None,
        };

        // Sanitize status
        let status = self.status.map(|s| {
            let trimmed = s.trim().to_string();
            if trimmed.len() > MAX_STATUS_LENGTH {
                trimmed[..MAX_STATUS_LENGTH].to_string()
            } else {
                trimmed
            }
        });

        // Sanitize links
        let links = self.links.map(|links_vec| {
            links_vec
                .into_iter()
                .take(MAX_LINKS)
                .filter_map(|link| {
                    let title = link.title.trim().to_string();
                    let sanitized_url = link.url.trim().to_string();

                    // Parse and validate the URL
                    match Url::parse(&sanitized_url) {
                        Ok(_) => {
                            // Ensure the title is within the allowed limit
                            let title = if title.len() > MAX_LINK_TITLE_LENGTH {
                                title[..MAX_LINK_TITLE_LENGTH].to_string()
                            } else {
                                title
                            };

                            // Ensure the URL is within the allowed limit
                            let url = if sanitized_url.len() > MAX_LINK_URL_LENGTH {
                                sanitized_url[..MAX_LINK_URL_LENGTH].to_string()
                            } else {
                                sanitized_url
                            };

                            // Only keep valid URLs
                            Some(UserLink { title, url })
                        }
                        Err(_) => {
                            error!("Invalid profile url {}", sanitized_url);
                            None // Discard invalid links
                        }
                    }
                })
                .collect()
        });

        Ok(PubkyAppUser {
            name,
            bio,
            image,
            links,
            status,
        })
    }

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
