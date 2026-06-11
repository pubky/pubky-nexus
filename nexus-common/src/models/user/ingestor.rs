use pubky_app_specs::{ParsedUri, PubkyId};

use crate::db::PubkyConnector;
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::traits::Collection;
use crate::models::user::{set_user_homeserver, UserDetails, UserHsCursor};
use crate::StackConfig;

/// Ingests previously-unknown users referenced by events, refusing any user
/// whose HS is blacklisted
#[derive(Debug, Default, Clone)]
pub struct UserIngestor {
    /// HS PKs which should not be indexed
    hs_blacklist: HsBlacklist,
}

impl UserIngestor {
    /// Builds an ingestor enforcing the given HS blacklist.
    pub fn new(external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>) -> Self {
        Self {
            hs_blacklist: HsBlacklist::new(external_hs_pk_blacklist),
        }
    }

    pub fn from_config(config: &StackConfig) -> Self {
        Self::new(config.external_hs_pk_blacklist.iter().cloned())
    }

    /// Ingests the author of a referenced post, if unknown.
    pub async fn maybe_ingest_author_of_post(
        &self,
        referenced_post_uri: &ParsedUri,
    ) -> ModelResult<()> {
        self.maybe_ingest_user(&referenced_post_uri.user_id).await
    }

    /// Resolves the HS hosting `user_id` and refuses it if blacklisted.
    ///
    /// # Returns
    /// - `Ok(Some(hs_id))` if the user's HS resolved and is not blacklisted
    /// - `Ok(None)` if the user has no published HS or is an HS PK itself
    /// - [`ModelError::HsBlacklisted`] if the resolved HS is blacklisted
    pub async fn ensure_hs_not_blacklisted(
        &self,
        user_id: &PubkyId,
    ) -> ModelResult<Option<String>> {
        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;

        let Some(hs_pk) = pubky.get_homeserver_of(&user_id.to_public_key()).await else {
            return Ok(None);
        };

        let hs_id = hs_pk.into_inner().to_z32();
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            return Err(ModelError::HsBlacklisted { hs_id });
        }

        Ok(Some(hs_id))
    }

    /// If a referenced user is unknown, not ingested in the graph yet, resolves their HS
    /// and persists the user node in the graph.
    ///
    /// If the resolved HS is blacklisted, throws  [`ModelError::HsBlacklisted`].
    #[tracing::instrument(name = "user.ingest", skip_all)]
    pub async fn maybe_ingest_user(&self, user_id: &PubkyId) -> ModelResult<()> {
        let user_id_str = user_id.to_string();
        if UserDetails::get_by_id(&user_id_str).await?.is_some() {
            // If user is already known, don't check the HS blacklist as it has no effect on users
            tracing::debug!("Skipping ingestion: {user_id_str} already known");
            return Ok(());
        }

        let maybe_hs_id = self
            .ensure_hs_not_blacklisted(user_id)
            .await
            .inspect_err(|e| tracing::warn!("Aborting ingestion of {user_id}: {e}"))?;

        let Some(hs_id) = maybe_hs_id else {
            tracing::warn!("Skipping ingestion: {user_id} has no published HS or is an HS PK");
            return Ok(());
        };

        let user_details = UserDetails::from_pubky(user_id.clone());

        // Do not add to index, as this would affect the timeline of events for this user.
        // Only create stub graph node for HS-resolver to store user-HS mapping.
        user_details
            .put_to_graph()
            .await
            .inspect(|_| tracing::info!("Ingested user {user_id} from HS {hs_id}"))
            .inspect_err(|e| tracing::error!("Failed to ingest user {user_id}: {e}"))?;

        // Bind the user to their HS (HOSTED_BY + resolved_at), since we just resolved the HS
        set_user_homeserver(&user_id_str, &hs_id).await?;

        // Store the start point of the user's HS cursor
        UserHsCursor::write(user_id, &hs_id, 0).await?;

        Ok(())
    }
}
