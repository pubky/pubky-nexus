use axum::body::Bytes;
use serde::{Deserialize, Serialize};

/// Profile schema
#[derive(Deserialize, Serialize, Debug)]
pub struct HomeserverFile {
    pub name: String,
    pub src: String,
    pub content_type: String,
    pub size: u64,
}

impl HomeserverFile {
    pub async fn try_from(blob: &Bytes) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file: Self = serde_json::from_slice(blob)?;
        file.validate().await?;
        Ok(file)
    }

    // TODO: content_type validation.
    pub async fn validate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}
