use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::models::user::UserIngestor;
use nexus_common::{types::DynError, ApiConfig};
use pubky::pkarr::{self, Keypair};

#[derive(Debug, Clone)]
pub struct ApiContext {
    pub(crate) api_config: ApiConfig,
    pub(crate) keypair: pkarr::Keypair,
    pub(crate) pkarr_client: pkarr::Client,
    pub(crate) ingestor: Arc<UserIngestor>,
}

pub struct ApiContextBuilder {
    api_config: ApiConfig,
    secret_dir: PathBuf,
    pkarr_builder: Option<pkarr::ClientBuilder>,
}

impl ApiContextBuilder {
    /// `secret_dir` holds the API's `secret` key file, which is created if missing
    pub fn new(api_config: ApiConfig, secret_dir: PathBuf) -> Self {
        Self {
            api_config,
            secret_dir,
            pkarr_builder: None,
        }
    }

    pub fn pkarr_builder(mut self, pkarr_builder: pkarr::ClientBuilder) -> Self {
        self.pkarr_builder = Some(pkarr_builder);

        self
    }

    pub fn try_build(&self) -> Result<ApiContext, DynError> {
        // Ensure the dir exists, so a missing secret file can be created in it
        std::fs::create_dir_all(self.secret_dir.clone())?;

        let ingestor = UserIngestor::from_config(&self.api_config.stack);

        let pkarr_builder = self.pkarr_builder.clone().unwrap_or_default();
        let pkarr_client = pkarr_builder.build()?;

        let keypair = self.read_or_create_keypair()?;

        Ok(ApiContext {
            api_config: self.api_config.clone(),
            keypair,
            pkarr_client,
            ingestor: Arc::new(ingestor),
        })
    }

    /// Reads the secret file. Creates a new secret file if it doesn't exist.
    fn read_or_create_keypair(&self) -> Result<Keypair, DynError> {
        let secret_file_path = self.secret_dir.join("secret");

        if !secret_file_path.exists() {
            Keypair::random().write_secret_key_file(&secret_file_path)?;
        }

        let keypair =
            Keypair::from_secret_key_file(&secret_file_path).map_err(|e| e.to_string())?;

        Ok(keypair)
    }
}
