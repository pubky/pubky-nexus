use pkarr::mainline::Testnet;
use pubky::PubkyClient;

use crate::Config;
use std::sync::Arc;
use tokio::sync::OnceCell;  

static EVENT_RESOLVER_SINGLETON: OnceCell<EventResolver> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub struct EventResolver {
    pubky_client: Arc<PubkyClient>
}

#[derive(Debug)]
pub enum EventResolverError {
    AlreadyInitialized,
    NotInitialized
}

impl std::fmt::Display for EventResolverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventResolverError::AlreadyInitialized => write!(f, "Event Resolver has already been initialized"),
            EventResolverError::NotInitialized => write!(f, "Event Resolver must be called before accessing PubkyClient connector"),
        }
    }
}

impl std::error::Error for EventResolverError {}

impl EventResolver {
    
    /// Initializes the EventResolver singleton with the given configuration
    pub fn initialise(config: &Config) -> Result<(), EventResolverError> {
        if EVENT_RESOLVER_SINGLETON.get().is_some() {
            return Err(EventResolverError::AlreadyInitialized);
        }
        let pubky_client = match config.testnet {
            true => {
                let testnet = Testnet {
                    bootstrap: vec![config.bootstrap.clone()],
                    nodes: vec![],
                };
                PubkyClient::test(&testnet)
            },
            false => PubkyClient::default()
        };
        let manager = Self {
            pubky_client: Arc::new(pubky_client),
        };
        EVENT_RESOLVER_SINGLETON.set(manager).map_err(|_| EventResolverError::AlreadyInitialized)?;
        Ok(())
    }

    /// Retrieves the shared PubkyClient connection.
    pub fn get_pubky_client() -> Result<Arc<PubkyClient>, EventResolverError> {
        if let Some(resolver) = EVENT_RESOLVER_SINGLETON.get() {
            Ok(resolver.pubky_client.clone())
        } else {
            Err(EventResolverError::NotInitialized)
        }
    }
}