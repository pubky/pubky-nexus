use pkarr::mainline::Testnet;
use pubky::PubkyClient;

use crate::Config;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

static PUBKY_CONNECTOR_SINGLETON: OnceCell<PubkyConnector> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub struct PubkyConnector {
    pubky_client: Arc<PubkyClient>,
}

#[derive(Debug)]
pub enum PubkyConnectorError {
    AlreadyInitialized,
    NotInitialized,
}

impl std::fmt::Display for PubkyConnectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PubkyConnectorError::AlreadyInitialized => {
                write!(f, "PubkyConnector has already been initialized")
            }
            PubkyConnectorError::NotInitialized => write!(
                f,
                "PubkyConnector must be called before accessing PubkyClient connector"
            ),
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
        // There is not need to initialise
        if PUBKY_CONNECTOR_SINGLETON.get().is_some() {
            return Ok(());
        }
        let pubky_client = match testnet {
            Some(testnet) => PubkyClient::builder()
                .testnet(testnet)
                .dht_request_timeout(Duration::from_millis(2000))
                .build(),
            None => match config.testnet {
                true => {
                    let testnet = Testnet {
                        bootstrap: vec![config.bootstrap.clone()],
                        nodes: vec![],
                    };
                    PubkyClient::test(&testnet)
                }
                false => PubkyClient::default(),
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

    /// Retrieves the shared PubkyClient connection.
    pub fn get_pubky_client() -> Result<Arc<PubkyClient>, PubkyConnectorError> {
        if let Some(resolver) = PUBKY_CONNECTOR_SINGLETON.get() {
            Ok(resolver.pubky_client.clone())
        } else {
            Err(PubkyConnectorError::NotInitialized)
        }
    }
}
