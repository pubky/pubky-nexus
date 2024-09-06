use serde::{Deserialize, Serialize};

use super::traits::Validatable;

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

impl Validatable for PubkyAppFollow {
    // TODO: validate follow
    async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
