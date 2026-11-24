use std::path::PathBuf;
use std::sync::Arc;

use nexus_common::db::{build_pubky, PubkyClientError, PubkyClientResult};
use nexus_common::models::user::{PkdnsHomeserverResolver, PkdnsRecord, UserIngestor};
use nexus_common::{types::DynError, ApiConfig};
use pubky::pkarr::{self, Keypair};
use pubky::PublicKey;
use pubky_watcher::WatcherClient;

#[derive(Debug, Clone)]
pub struct ApiContext {
    pub(crate) api_config: ApiConfig,
    pub(crate) keypair: pkarr::Keypair,
    pub(crate) pkarr_client: pkarr::Client,
    pub(crate) ingestor: Arc<UserIngestor>,
    /// The Pubky client the ingest-user route looks users' homeservers up with. `None` when
    /// it could not be built: serving reads does not depend on it, so only that route fails.
    pub(crate) pubky_client: Option<Arc<WatcherClient>>,
    /// Why [`Self::pubky_client`] could not be built, logged once the API starts.
    pub(crate) pubky_client_error: Option<String>,
}

pub struct ApiContextBuilder {
    api_config: ApiConfig,
    secret_dir: PathBuf,
    pkarr_builder: Option<pkarr::ClientBuilder>,
    pubky_client: Option<Arc<WatcherClient>>,
}

impl ApiContextBuilder {
    /// `secret_dir` holds the API's `secret` key file, which is created if missing
    pub fn new(api_config: ApiConfig, secret_dir: PathBuf) -> Self {
        Self {
            api_config,
            secret_dir,
            pkarr_builder: None,
            pubky_client: None,
        }
    }

    /// The Pubky client to share with the rest of the process. Without one, the context builds
    /// its own from the `[stack.net]` settings.
    pub fn pubky_client(mut self, pubky_client: Arc<WatcherClient>) -> Self {
        self.pubky_client = Some(pubky_client);

        self
    }

    pub fn pkarr_builder(mut self, pkarr_builder: pkarr::ClientBuilder) -> Self {
        self.pkarr_builder = Some(pkarr_builder);

        self
    }

    pub fn try_build(&self) -> Result<ApiContext, DynError> {
        // Ensure the dir exists, so a missing secret file can be created in it
        std::fs::create_dir_all(self.secret_dir.clone())?;

        let (pubky_client, pubky_client_error) = match &self.pubky_client {
            Some(pubky_client) => (Some(pubky_client.clone()), None),
            None => match build_pubky(&self.api_config.stack.net) {
                Ok(pubky) => (Some(Arc::new(WatcherClient::from_shared(pubky))), None),
                Err(e) => (None, Some(e.to_string())),
            },
        };
        let ingestor = UserIngestor::from_config(
            &self.api_config.stack,
            Arc::new(IngestResolver(pubky_client.clone())),
        );

        let pkarr_builder = self.pkarr_builder.clone().unwrap_or_default();
        let pkarr_client = pkarr_builder.build()?;

        let keypair = self.read_or_create_keypair()?;

        Ok(ApiContext {
            api_config: self.api_config.clone(),
            keypair,
            pkarr_client,
            ingestor: Arc::new(ingestor),
            pubky_client,
            pubky_client_error,
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

/// Looks users up on PKDNS through the API's Pubky client. Without one, every lookup fails as
/// it did before the client was built, with [`PubkyClientError::NotInitialized`].
struct IngestResolver(Option<Arc<WatcherClient>>);

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for IngestResolver {
    async fn resolve_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        match &self.0 {
            Some(pubky_client) => pubky_client.resolve_record(user_pk).await,
            None => Err(PubkyClientError::NotInitialized),
        }
    }
}
