use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    pub description: String,
    pub homepage: String,
    pub license: String,
    pub name: String,
    pub repository: String,
    pub version: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerInfo {
    pub fn new() -> Self {
        Self {
            description: env!("CARGO_PKG_DESCRIPTION").to_string(),
            homepage: env!("CARGO_PKG_HOMEPAGE").to_string(),
            license: env!("CARGO_PKG_LICENSE").to_string(),
            name: env!("CARGO_PKG_NAME").to_string(),
            repository: env!("CARGO_PKG_REPOSITORY").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}
