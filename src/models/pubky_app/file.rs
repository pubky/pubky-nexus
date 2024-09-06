use serde::{Deserialize, Serialize};

use super::traits::Validatable;

/// Profile schema
#[derive(Deserialize, Serialize, Debug)]
pub struct PubkyAppFile {
    pub name: String,
    pub src: String,
    pub content_type: String,
    pub size: u64,
}

impl Validatable for PubkyAppFile {
    // TODO: content_type validation.
    async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
