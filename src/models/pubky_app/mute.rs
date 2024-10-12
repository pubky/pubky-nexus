use super::traits::Validatable;
use axum::async_trait;
use serde::{Deserialize, Serialize};

/// Represents raw homeserver Mute object with timestamp
/// URI: /pub/pubky.app/mute/:user_id
///
/// Example URI:
///
/// `/pub/pubky.app/mute/pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy``
///
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppMute {
    pub created_at: i64,
}

#[async_trait]
impl Validatable for PubkyAppMute {
    async fn validate(&self, _id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: additional Mute validation? E.g, validate `created_at` ?
        Ok(())
    }
}
