use std::future::Future;
use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use super::PkdnsHomeserverResolver;
use crate::db::PubkyClientResult;
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::traits::Collection;
use crate::models::user::{
    get_user_homeserver, set_user_homeserver, HomeserverRecheck, MappingUpdate, RecheckOutcome,
    StaleMapping, UserDetails, UserHsCursor,
};
use crate::StackConfig;

/// Ingests previously-unknown users unless their HS is blacklisted.
#[derive(Clone)]
pub struct UserIngestor {
    hs_blacklist: HsBlacklist,
    /// Looks up the HS a user publishes.
    resolver: Arc<dyn PkdnsHomeserverResolver>,
}

/// Names the blacklist only: the resolver is a trait object.
impl std::fmt::Debug for UserIngestor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserIngestor")
            .field("hs_blacklist", &self.hs_blacklist)
            .finish_non_exhaustive()
    }
}

impl UserIngestor {
    /// Builds an ingestor enforcing the given HS blacklist, looking users' homeservers up
    /// through `resolver`.
    pub fn new(
        external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>,
        resolver: Arc<dyn PkdnsHomeserverResolver>,
    ) -> Self {
        Self {
            hs_blacklist: HsBlacklist::new(external_hs_pk_blacklist),
            resolver,
        }
    }

    /// Builds an ingestor enforcing `config`'s HS blacklist, looking users' homeservers up
    /// through `resolver`.
    pub fn from_config(config: &StackConfig, resolver: Arc<dyn PkdnsHomeserverResolver>) -> Self {
        Self::new(
            config.net.external_hs_pk_blacklist.iter().cloned(),
            resolver,
        )
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
    ) -> ModelResult<Option<PubkyId>> {
        let user_pk = user_id.to_public_key();
        self.resolve_unless_blacklisted(user_id, self.resolver.resolve_homeserver(&user_pk))
            .await
    }

    /// `lookup`'s answer for `user_id`, refused if it is blacklisted.
    async fn resolve_unless_blacklisted(
        &self,
        user_id: &PubkyId,
        lookup: impl Future<Output = PubkyClientResult<Option<PubkyId>>>,
    ) -> ModelResult<Option<PubkyId>> {
        // `user_id` may itself be an HS PK (e.g. a file `src` of the form
        // `pubky://<hs_pk>/...` that addresses the HS directly). The resolver
        // returns `None` for an HS PK, so without this self-check a blacklisted HS
        // used as a direct source would slip through and we'd reach out to it.
        if self.hs_blacklist.is_blacklisted(user_id.as_ref()) {
            return Err(ModelError::HsBlacklisted {
                hs_id: user_id.to_string(),
            });
        }

        let resolved = lookup.await.map_err(ModelError::from_generic)?;
        let Some(hs_id) = resolved else {
            return Ok(None);
        };

        if self.hs_blacklist.is_blacklisted(hs_id.as_ref()) {
            return Err(ModelError::HsBlacklisted {
                hs_id: hs_id.to_string(),
            });
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
        set_user_homeserver(&user_id_str, hs_id.as_ref()).await?;

        // Seed the user's HS cursor floor (no-op if we're already tracking them).
        UserHsCursor::init(user_id, hs_id.as_ref()).await?;

        Ok(())
    }

    /// Re-checks a known user's homeserver against the network now and follows
    /// the record: a record naming another homeserver moves the mapping there,
    /// a record naming none keeps it. The lookup is the resolver's fresh one
    /// ([`PkdnsHomeserverResolver::resolve_homeserver_fresh`]): the network's
    /// newest packet, not a cache. This is the programmatic way to switch a
    /// user's homeserver; the periodic resolver only follows records when its
    /// stale mapping is disabled.
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

        let user_pk = user_id.to_public_key();
        let resolved = self
            .resolve_unless_blacklisted(user_id, self.resolver.resolve_homeserver_fresh(&user_pk))
            .await
            .inspect_err(|e| tracing::warn!("Aborting recheck of {user_id}: {e}"))?;

        let stored = get_user_homeserver(&user_id_str).await?;
        let update =
            MappingUpdate::decide(stored.as_ref(), resolved.as_ref(), StaleMapping::Disabled);
        update.apply(&user_id_str).await?;
        let homeserver = match update.bound_to() {
            Some(hs_id) => {
                UserHsCursor::init(user_id, hs_id.as_ref()).await?;
                Some(hs_id.to_string())
            }
            None => stored.map(|mapping| mapping.hs_id),
        };

        Ok(Some(HomeserverRecheck {
            homeserver,
            outcome: RecheckOutcome::from(&update),
        }))
    }
}

#[cfg(test)]
mod tests {
    use pubky::PublicKey;

    use super::*;
    use crate::db::PubkyClientResult;
    use crate::utils::test_utils::random_pubky_id;

    /// Resolves every user to one homeserver, without the network.
    struct FixedResolver(PubkyId);

    #[async_trait::async_trait]
    impl PkdnsHomeserverResolver for FixedResolver {
        async fn resolve_homeserver(&self, _: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
            Ok(Some(self.0.clone()))
        }
    }

    /// The blacklist check looks the user's homeserver up through the injected resolver.
    #[tokio::test]
    async fn blacklist_check_uses_the_injected_resolver() {
        let blacklisted = random_pubky_id();
        let allowed = random_pubky_id();
        let user = random_pubky_id();

        let refusing = UserIngestor::new(
            [blacklisted.clone()],
            Arc::new(FixedResolver(blacklisted.clone())),
        );
        let err = refusing.ensure_hs_not_blacklisted(&user).await.unwrap_err();
        assert!(
            matches!(err, ModelError::HsBlacklisted { hs_id } if hs_id == blacklisted.to_string())
        );

        let accepting = UserIngestor::new([blacklisted], Arc::new(FixedResolver(allowed.clone())));
        let hs_id = accepting.ensure_hs_not_blacklisted(&user).await.unwrap();
        assert_eq!(hs_id, Some(allowed));
    }
}
