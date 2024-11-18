use pkarr::PublicKey;
use serde::{Deserialize, Serialize};
use std::fmt;
use utoipa::ToSchema;
use crate::types::DynError;

/// Represents user data with name, bio, image, links, and status.
#[derive(Serialize, Deserialize, ToSchema, Default, Clone, Debug)]
pub struct PubkyId(pub String);

impl PubkyId {
    pub fn try_from(str: &str) -> Result<Self, DynError> {
        // Validate string is a valid Pkarr public key
        PublicKey::try_from(str)?;
        Ok(PubkyId(str.to_string()))
    }
}

impl fmt::Display for PubkyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Deref for PubkyId {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for PubkyId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
