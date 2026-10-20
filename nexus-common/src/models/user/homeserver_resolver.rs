use pubky::{Pubky, PublicKey};
use pubky_app_specs::PubkyId;

use crate::db::{PubkyClientResult, PubkyConnector};

/// Resolves a user's currently published homeserver from PKDNS/DHT.
///
/// Abstracted behind a trait so callers can route the lookup through their own client, or
/// drive it with a mock in tests instead of hitting the network.
#[async_trait::async_trait]
pub trait PkdnsHomeserverResolver: Send + Sync {
    /// Returns the HS published for `user_pk`, if any is currently published.
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>>;

    /// Like [`Self::resolve_homeserver`], but asks every DHT node and relay for the newest
    /// record instead of trusting a cache, which is what a check made because something
    /// changed has to do. A resolver without a cache has nothing fresher to answer with, so
    /// by default it answers as [`Self::resolve_homeserver`] does.
    async fn resolve_homeserver_fresh(
        &self,
        user_pk: &PublicKey,
    ) -> PubkyClientResult<Option<PubkyId>> {
        self.resolve_homeserver(user_pk).await
    }
}

/// The HS `user_pk` publishes, after `pubky`'s pkarr cache is refreshed with the newest
/// packet on the network; the lookup reads that cache.
pub async fn fresh_homeserver_of(pubky: &Pubky, user_pk: &PublicKey) -> Option<PubkyId> {
    pubky.client().pkarr().resolve_most_recent(user_pk).await;
    pubky.get_homeserver_of(user_pk).await.map(PubkyId::from)
}

/// Resolver backed by the shared [`PubkyConnector`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PubkyConnectorResolver;

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyConnectorResolver {
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        let pubky = PubkyConnector::get()?;
        Ok(pubky.get_homeserver_of(user_pk).await.map(PubkyId::from))
    }

    async fn resolve_homeserver_fresh(
        &self,
        user_pk: &PublicKey,
    ) -> PubkyClientResult<Option<PubkyId>> {
        let pubky = PubkyConnector::get()?;
        Ok(fresh_homeserver_of(&pubky, user_pk).await)
    }
}
