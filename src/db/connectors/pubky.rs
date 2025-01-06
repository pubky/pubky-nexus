use crate::Config;
use mainline::Testnet;
use pubky::Client;
use std::sync::Arc;
use tokio::sync::OnceCell;

static PUBKY_CONNECTOR_SINGLETON: OnceCell<PubkyConnector> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub struct PubkyConnector {
    pubky_client: Arc<Client>,
}

#[derive(Debug)]
pub enum PubkyConnectorError {
    AlreadyInitialized,
    NotInitialized,
    IoError(std::io::Error),
}

impl From<std::io::Error> for PubkyConnectorError {
    fn from(e: std::io::Error) -> Self {
        PubkyConnectorError::IoError(e)
    }
}

impl std::fmt::Display for PubkyConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubkyConnectorError::AlreadyInitialized => {
                write!(f, "PubkyConnector has already been initialized")
            }
            PubkyConnectorError::NotInitialized => write!(f, "PubkyConnector not initialized"),
            PubkyConnectorError::IoError(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for PubkyConnectorError {}

impl PubkyConnector {
    /// Initializes the PubkyConnector singleton with the given configuration
    pub fn initialise(
        config: &Config,
        testnet: Option<&Testnet>,
    ) -> Result<(), PubkyConnectorError> {
        // There is not need to initialise, already in the global context
        if PUBKY_CONNECTOR_SINGLETON.get().is_some() {
            return Ok(());
        }
        let pubky_client = match testnet {
            Some(testnet) => Client::builder().testnet(testnet).build()?,
            None => match config.testnet {
                true => {
                    let testnet = Testnet {
                        bootstrap: vec![config.bootstrap.clone()],
                        nodes: vec![],
                    };
                    Client::builder().testnet(&testnet).build()?
                }
                false => Client::new()?,
            },
        };
        let manager = Self {
            pubky_client: Arc::new(pubky_client),
        };
        PUBKY_CONNECTOR_SINGLETON
            .set(manager)
            .map_err(|_| PubkyConnectorError::AlreadyInitialized)?;
        Ok(())
    }

    /// Retrieves the shared Client connection.
    pub fn get_pubky_client() -> Result<Arc<Client>, PubkyConnectorError> {
        if let Some(resolver) = PUBKY_CONNECTOR_SINGLETON.get() {
            Ok(resolver.pubky_client.clone())
        } else {
            Err(PubkyConnectorError::NotInitialized)
        }
    }
}
