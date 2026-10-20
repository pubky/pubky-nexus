use pubky_app_specs::{ParsedUri, PubkyId};

use crate::db::PubkyConnector;
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::traits::Collection;
use crate::models::user::{
    apply_resolved_homeserver, get_user_homeserver, set_user_homeserver, HomeserverRecheck,
    MappingUpdate, StaleMapping, UserDetails, UserHsCursor,
};
use crate::StackConfig;

/// Ingests previously-unknown users unless their HS is blacklisted.
#[derive(Debug, Default, Clone)]
pub struct UserIngestor {
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
        Self::new(config.net.external_hs_pk_blacklist.iter().cloned())
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
        // `user_id` may itself be an HS PK (e.g. a file `src` of the form
        // `pubky://<hs_pk>/...` that addresses the HS directly). `get_homeserver_of`
        // returns `None` for an HS PK, so without this self-check a blacklisted HS
        // used as a direct source would slip through and we'd reach out to it.
        if self.hs_blacklist.is_blacklisted(user_id.as_ref()) {
            return Err(ModelError::HsBlacklisted {
                hs_id: user_id.to_string(),
            });
        }

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

    /// Resolves and persists a previously-unknown user.
    ///
    /// Returns [`ModelError::HsBlacklisted`] if the user's resolved HS is blacklisted.
    #[tracing::instrument(name = "user.ingest", skip_all)]
    pub async fn maybe_ingest_user(&self, user_id: &PubkyId) -> ModelResult<()> {
        let user_id_str = user_id.to_string();
        if UserDetails::get_by_id(&user_id_str).await?.is_some() {
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

        // Seed the user's HS cursor floor (no-op if we're already tracking them).
        UserHsCursor::init(user_id, &hs_id).await?;

        Ok(())
    }

    /// Re-checks a known user's homeserver against the network now and follows
    /// the record: a record naming another homeserver moves the mapping there,
    /// a record naming none keeps it. The lookup asks every DHT node and relay
    /// for the newest packet instead of trusting the client's cache, which is
    /// what a check made because something changed has to do. This is the
    /// programmatic way to switch a user's homeserver; the periodic resolver
    /// only follows records when its stale mapping is disabled.
    ///
    /// Returns `None` for a user nexus does not know; ingest them instead.
    ///
    /// Returns [`ModelError::HsBlacklisted`] if the published HS is blacklisted.
    #[tracing::instrument(name = "user.recheck", skip_all)]
    pub async fn recheck_user(&self, user_id: &PubkyId) -> ModelResult<Option<HomeserverRecheck>> {
        let user_id_str = user_id.to_string();
        if UserDetails::get_by_id(&user_id_str).await?.is_none() {
            tracing::debug!("Skipping recheck: {user_id_str} unknown");
            return Ok(None);
        }

        let pubky = PubkyConnector::get().map_err(ModelError::from_generic)?;
        // Refreshes the client's cache with the newest packet on the network;
        // the lookup below reads that cache.
        pubky
            .client()
            .pkarr()
            .resolve_most_recent(&user_id.to_public_key())
            .await;

        let resolved = match self
            .ensure_hs_not_blacklisted(user_id)
            .await
            .inspect_err(|e| tracing::warn!("Aborting recheck of {user_id}: {e}"))?
        {
            Some(hs_id) => {
                Some(PubkyId::try_from(hs_id.as_str()).map_err(ModelError::from_generic)?)
            }
            None => None,
        };

        let update =
            apply_resolved_homeserver(&user_id_str, resolved.as_ref(), StaleMapping::Disabled)
                .await?;
        if let (MappingUpdate::Bound | MappingUpdate::Switched { .. }, Some(hs_id)) =
            (&update, &resolved)
        {
            tracing::info!("User {user_id} now bound to HS {hs_id} after a recheck");
            UserHsCursor::init(user_id, hs_id).await?;
        }

        let homeserver = get_user_homeserver(&user_id_str)
            .await?
            .map(|mapping| mapping.hs_id);
        Ok(Some(HomeserverRecheck::new(&update, homeserver)))
    }
}
