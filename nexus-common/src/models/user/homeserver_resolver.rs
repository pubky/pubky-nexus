use pubky::PublicKey;
use pubky_social_specs::PubkyId;

use crate::db::{PubkyClientResult, PubkyConnector};

/// Resolves a user's currently published homeserver from PKDNS/DHT.
///
/// Abstracted behind a trait so callers can route the lookup through their own client, or
/// drive it with a mock in tests instead of hitting the network.
#[async_trait::async_trait]
pub trait PkdnsHomeserverResolver: Send + Sync {
    /// Returns the HS published for `user_pk`, if any is currently published.
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>>;
}

/// Resolver backed by the shared [`PubkyConnector`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PubkyConnectorResolver;

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyConnectorResolver {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        let pubky = PubkyConnector::get()?;
        // `Ok(None)` means no HS is published. A failed lookup and a malformed
        // `_pubky` record stay an `Err` (`PkarrFailed`), so the resolver loop
        // reports them as `LookupFailed` and `InvalidRecord` instead of treating
        // the user as having no HS.
        Ok(pubky.get_homeserver_of(user_pk).await?.map(PubkyId::from))
    }
}
