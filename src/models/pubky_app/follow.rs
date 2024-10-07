use super::traits::Validatable;
use axum::async_trait;
use serde::{Deserialize, Serialize};

/// Represents raw homeserver follow object with timestamp
/// URI: /pub/pubky.app/follows/:user_id
///
/// Example URI:
///
/// `/pub/pubky.app/follows/pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy``
///
#[derive(Serialize, Deserialize, Default)]
pub struct PubkyAppFollow {
    pub created_at: i64,
}

#[async_trait]
impl Validatable for PubkyAppFollow {
    async fn validate(&self, _id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // TODO: additional follow validation? E.g, validate `created_at` ?
        Ok(())
    }
}
