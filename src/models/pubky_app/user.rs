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
        // Sanitize name
        let sanitized_name = self.name.trim();
        // Crop name to a maximum length of MAX_USERNAME_LENGTH characters
        let mut name = sanitized_name
            .chars()
            .take(MAX_USERNAME_LENGTH)
            .collect::<String>();

        // We use username keyword `[DELETED]` for a user whose `profile.json` has been deleted
        // Therefore this is not a valid username.
        if name == *"[DELETED]" {
            name = "anonymous".to_string(); // default username
        }

        // Sanitize bio
        let bio = self
            .bio
            .map(|b| b.trim().chars().take(MAX_BIO_LENGTH).collect::<String>());

        // Sanitize image URL with URL parsing
        let image = match &self.image {
            Some(image_url) => {
                let sanitized_image_url = image_url.trim();

                match Url::parse(sanitized_image_url) {
                    Ok(_) => {
                        // Ensure the URL is within the allowed limit
                        let url = sanitized_image_url
                            .chars()
                            .take(MAX_IMAGE_LENGTH)
                            .collect::<String>();
                        Some(url) // Valid image URL
                    }
                    Err(_) => None, // Invalid image URL, set to None
                }
            }
            None => None,
        };

        // Sanitize status
        let status = self
            .status
            .map(|s| s.trim().chars().take(MAX_STATUS_LENGTH).collect::<String>());

        // Sanitize links
        let links = self.links.map(|links_vec| {
            links_vec
                .into_iter()
                .take(MAX_LINKS)
                .filter_map(|link| {
                    let title = link.title.trim();
                    let sanitized_url = link.url.trim();

                    // Parse and validate the URL
                    match Url::parse(sanitized_url) {
                        Ok(_) => {
                            // Ensure the title is within the allowed limit
                            let title = title
                                .chars()
                                .take(MAX_LINK_TITLE_LENGTH)
                                .collect::<String>();

                            // Ensure the URL is within the allowed limit
                            let url = sanitized_url
                                .chars()
                                .take(MAX_LINK_URL_LENGTH)
                                .collect::<String>();

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
        let name_length = self.name.chars().count();
        if !(MIN_USERNAME_LENGTH..=MAX_USERNAME_LENGTH).contains(&name_length) {
            return Err("Invalid name length".into());
        }

        // Validate bio length
        if let Some(bio) = &self.bio {
            if bio.chars().count() > MAX_BIO_LENGTH {
                return Err("Bio exceeds maximum length".into());
            }
        }

        // Validate image length
        if let Some(image) = &self.image {
            if image.chars().count() > MAX_IMAGE_LENGTH {
                return Err("Image URI exceeds maximum length".into());
            }
        }

        // Validate links
        if let Some(links) = &self.links {
            if links.len() > MAX_LINKS {
                return Err("Too many links".into());
            }
            for link in links {
                if link.title.chars().count() > MAX_LINK_TITLE_LENGTH
                    || link.url.chars().count() > MAX_LINK_URL_LENGTH
                {
                    return Err("Link title or URL too long".into());
                }
            }
        }

        // Validate status length
        if let Some(status) = &self.status {
            if status.chars().count() > MAX_STATUS_LENGTH {
                return Err("Status exceeds maximum length".into());
            }
        }

        Ok(())
    }
}
