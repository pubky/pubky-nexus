use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
pub struct UserId(pub String);

impl UserId {
    pub fn try_from(str: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Validate string is a valid Pkarr public key
        PublicKey::try_from(str)?;
        Ok(UserId(str.to_string()))
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Deref for UserId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for UserId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
