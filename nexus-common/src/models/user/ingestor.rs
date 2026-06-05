use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use crate::db::PubkyConnector;
use crate::models::error::{ModelError, ModelResult};
use crate::models::traits::Collection;
use crate::models::user::{UserDetails, UserHsCursor};
use crate::WatcherConfig;

/// Ingests previously-unknown users referenced by events, refusing any user
/// whose HS is blacklisted.
///
/// Shared by the watcher and webapi, both built from the same [`WatcherConfig`]
/// blacklist. The default carries an empty blacklist (ingest everything).
#[derive(Debug, Default, Clone)]
pub struct UserIngestor {
    /// HS PKs which should not be indexed
    external_hs_pk_blacklist: Arc<Vec<PubkyId>>,
}

impl UserIngestor {
    /// Builds an ingestor enforcing the given HS blacklist.
    pub fn new(external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>) -> Self {
        Self {
            external_hs_pk_blacklist: Arc::new(external_hs_pk_blacklist.into_iter().collect()),
        }
    }

    /// Builds an ingestor from [`WatcherConfig::external_hs_pk_blacklist`].
    pub fn from_config(config: &WatcherConfig) -> Self {
        Self::new(config.external_hs_pk_blacklist.iter().cloned())
    }

    /// Whether `hs_id` (z-base-32) is blacklisted.
    fn is_hs_blacklisted(&self, hs_id: &str) -> bool {
        self.external_hs_pk_blacklist
            .iter()
            .any(|pk| pk.as_ref() == hs_id)
    }

    /// Ingests the author of a referenced post, if unknown.
    pub async fn maybe_ingest_author_of_post(
        &self,
        referenced_post_uri: &ParsedUri,
    ) -> ModelResult<()> {
        self.maybe_ingest_user(&referenced_post_uri.user_id).await
    }

    /// If a referenced user is unknown, not ingested in the graph yet, resolves their HS
    /// and persists the user node in the graph.
    ///
    /// If the resolved HS is blacklisted, throws  [`ModelError::HomeserverBlacklisted`].
    #[tracing::instrument(name = "user.ingest", skip_all)]
    pub async fn maybe_ingest_user(&self, user_id: &PubkyId) -> ModelResult<()> {
        let user_id_str = user_id.to_string();
        if UserDetails::get_by_id(&user_id_str).await?.is_some() {
            tracing::debug!("Skipping ingestion: {user_id_str} already known");
            return Ok(());
        }

        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;

        let user_pk = user_id.to_public_key();

        let Some(hs_pk) = pubky.get_homeserver_of(&user_pk).await else {
            tracing::warn!("Skipping ingestion: {user_id} has no published HS or is an HS PK");
            return Ok(());
        };

        let hs_id = hs_pk.into_inner().to_z32();

        // Refuse users hosted on a blacklisted HS.
        if self.is_hs_blacklisted(&hs_id) {
            tracing::warn!("Aborting ingestion: {user_id} hosted on blacklisted HS {hs_id}");
            return Err(ModelError::HomeserverBlacklisted { hs_id });
        }

        let user_details = UserDetails::from_pubky(PubkyId::from(user_pk));

        // Do not add to index, as this would affect the timeline of events for this user.
        // Only create stub graph node for HS-resolver to store user-HS mapping.
        user_details
            .put_to_graph()
            .await
            .inspect(|_| tracing::info!("Ingested user {user_id} from HS {hs_id}"))
            .inspect_err(|e| tracing::error!("Failed to ingest user {user_id}: {e}"))?;

        // Store the start point of the user's HS cursor
        UserHsCursor::write(user_id, &hs_id, 0).await?;

        Ok(())
    }
}
