use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use super::PkdnsHomeserverResolver;
use crate::db::{fetch_row_from_graph, queries};
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::scope::{scope_from_config, EverythingScope, Scope};
use crate::models::traits::Collection;
use crate::models::user::outsider::create_outsider;
use crate::models::user::{set_user_homeserver, UserDetails, UserHsCursor};
use crate::{ScopeConfig, StackConfig};

/// Ingests previously-unknown users unless their HS is blacklisted.
#[derive(Clone)]
pub struct UserIngestor {
    hs_blacklist: HsBlacklist,
    /// Looks up the HS a user publishes.
    resolver: Arc<dyn PkdnsHomeserverResolver>,
    scope: Arc<dyn Scope>,
}

/// Names the blacklist and the scope only: the resolver is a trait object.
impl std::fmt::Debug for UserIngestor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserIngestor")
            .field("hs_blacklist", &self.hs_blacklist)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl UserIngestor {
    /// Builds an ingestor enforcing the given HS blacklist, looking users' homeservers up
    /// through `resolver`, and indexing everyone.
    pub fn new(
        external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>,
        resolver: Arc<dyn PkdnsHomeserverResolver>,
    ) -> Self {
        Self {
            hs_blacklist: HsBlacklist::new(external_hs_pk_blacklist),
            resolver,
            scope: Arc::new(EverythingScope),
        }
    }

    /// Indexes the users of `scope` only.
    pub fn with_scope(mut self, scope: Arc<dyn Scope>) -> Self {
        self.scope = scope;
        self
    }

    /// Builds an ingestor enforcing `config`'s HS blacklist and scope, looking users'
    /// homeservers up through `resolver`.
    pub fn from_config(config: &StackConfig, resolver: Arc<dyn PkdnsHomeserverResolver>) -> Self {
        Self::new(
            config.net.external_hs_pk_blacklist.iter().cloned(),
            resolver,
        )
        .with_scope(scope_from_config(&config.scope))
    }

    /// The scope deciding which users are indexed.
    pub fn scope(&self) -> &Arc<dyn Scope> {
        &self.scope
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
        // `pubky://<hs_pk>/...` that addresses the HS directly). The resolver
        // returns `None` for an HS PK, so without this self-check a blacklisted HS
        // used as a direct source would slip through and we'd reach out to it.
        if self.hs_blacklist.is_blacklisted(user_id.as_ref()) {
            return Err(ModelError::HsBlacklisted {
                hs_id: user_id.to_string(),
            });
        }

        let resolved = self
            .resolver
            .resolve_homeserver(&user_id.to_public_key())
            .await
            .map_err(ModelError::from_generic)?;
        let Some(hs_pk) = resolved else {
            return Ok(None);
        };

        let hs_id = hs_pk.to_string();
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            return Err(ModelError::HsBlacklisted { hs_id });
        }

        Ok(Some(hs_id))
    }

    /// Resolves and persists a previously-unknown user.
    ///
    /// A user in the scope is bound to its HS and gets a cursor, so it is polled. A user
    /// outside it is kept as an outsider; see [`crate::models::user::outsider`].
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

        let in_scope = self.scope.is_in_scope(&user_id_str, Some(&hs_id)).await?;

        let user_details = UserDetails::from_pubky(user_id.clone());

        if !in_scope {
            tracing::info!("Kept {user_id} from HS {hs_id} as an outsider: outside the scope");
            return create_outsider(&user_details).await;
        }

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

    /// Whether the events of `user_id` are indexed.
    ///
    /// The graph decides for a user it can place (an outsider, or a user bound to a homeserver).
    /// One it can't (unknown, or not bound) is placed by the homeserver it publishes now, so
    /// an old event in a listed feed by a user who has moved off that homeserver stays out.
    /// An unknown user is ingested on the way, as a member or an outsider, so its next events are
    /// decided from the graph alone.
    pub async fn is_author_in_scope(&self, user_id: &PubkyId) -> ModelResult<bool> {
        if *self.scope.config() == ScopeConfig::Everything {
            return Ok(true);
        }
        if !self.scope.is_in_scope(user_id, None).await? {
            return Ok(false);
        }
        if UserDetails::get_by_id(user_id).await?.is_none() {
            self.maybe_ingest_user(user_id).await?;
            return self.scope.is_in_scope(user_id, None).await;
        }
        let bound = fetch_row_from_graph(queries::get::get_user_homeserver(user_id)).await?;
        match bound {
            Some(_) => Ok(true),
            None => match self.ensure_hs_not_blacklisted(user_id).await? {
                Some(hs_id) => self.scope.is_in_scope(user_id, Some(&hs_id)).await,
                // Nothing published: the listed feed it came from is all there is to go by.
                None => Ok(true),
            },
        }
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
        assert_eq!(hs_id, Some(allowed.to_string()));
    }
}
