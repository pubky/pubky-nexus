use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use crate::db::{exec_single_row, fetch_key_from_graph, queries, PubkyConnector};
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::scope::{scope_from_config, EverythingScope, Scope};
use crate::models::traits::Collection;
use crate::models::user::{set_user_homeserver, UserCounts, UserDetails, UserHsCursor};
use crate::StackConfig;

/// `scope_state` of a user known to this Nexus but outside its scope.
pub const SCOPE_STATE_STUB: &str = "stub";

/// Ingests previously-unknown users unless their HS is blacklisted: users in the
/// scope are bound to their HS so they get polled, the others are kept as stubs.
#[derive(Debug, Clone)]
pub struct UserIngestor {
    hs_blacklist: HsBlacklist,
    scope: Arc<dyn Scope>,
}

impl Default for UserIngestor {
    fn default() -> Self {
        Self {
            hs_blacklist: HsBlacklist::default(),
            scope: Arc::new(EverythingScope),
        }
    }
}

impl UserIngestor {
    /// Builds an ingestor enforcing the given HS blacklist, indexing everyone.
    pub fn new(external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>) -> Self {
        Self {
            hs_blacklist: HsBlacklist::new(external_hs_pk_blacklist),
            ..Self::default()
        }
    }

    /// Builds an ingestor enforcing the given HS blacklist and scope.
    pub fn with_scope(
        external_hs_pk_blacklist: impl IntoIterator<Item = PubkyId>,
        scope: Arc<dyn Scope>,
    ) -> Self {
        Self {
            hs_blacklist: HsBlacklist::new(external_hs_pk_blacklist),
            scope,
        }
    }

    pub fn from_config(config: &StackConfig) -> Self {
        Self::with_scope(
            config.net.external_hs_pk_blacklist.iter().cloned(),
            scope_from_config(&config.scope),
        )
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
    /// A user in the scope is bound to its HS and gets a cursor, so it is polled.
    /// A user outside it is kept as a stub: the node exists so in-scope records can
    /// point at it, but it is never resolved again or polled.
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

        // Do not add to index, as this would affect the timeline of events for this user.
        // Only create stub graph node for HS-resolver to store user-HS mapping.
        user_details
            .put_to_graph()
            .await
            .inspect(|_| tracing::info!(in_scope, "Ingested user {user_id} from HS {hs_id}"))
            .inspect_err(|e| tracing::error!("Failed to ingest user {user_id}: {e}"))?;

        if !in_scope {
            let query = queries::put::set_user_scope_state(&user_id_str, Some(SCOPE_STATE_STUB));
            exec_single_row(query).await?;
            // A stub's own profile is never read, so it never gets the counts its profile
            // would create. In-scope records still move them (a follow, a fetched post),
            // and the counter updates need them to exist.
            UserCounts::default().put_to_index(&user_id_str).await?;
            return Ok(());
        }

        Self::bind_to_homeserver(&user_id_str, &hs_id).await
    }

    /// Tells the scope about a follow before it is indexed. If the follow brought the
    /// followee into the scope, a followee already kept as a stub is promoted so it
    /// starts being polled; an unknown followee is ingested in scope as usual.
    pub async fn on_follow(&self, follower_id: &PubkyId, followee_id: &PubkyId) -> ModelResult<()> {
        if self
            .scope
            .on_follow(follower_id.as_ref(), followee_id.as_ref())
            .await?
        {
            self.promote_if_stub(followee_id).await?;
        }
        Ok(())
    }

    /// Makes sure a user that must be in the scope (a seed) is known and polled.
    ///
    /// If its HS can't be resolved right now, the bare node is still created: the
    /// HS resolver binds it on a later run, and polling starts from there.
    pub async fn ensure_in_scope(&self, user_id: &PubkyId) -> ModelResult<()> {
        self.maybe_ingest_user(user_id).await?;
        if UserDetails::get_by_id(user_id).await?.is_none() {
            UserDetails::from_pubky(user_id.clone())
                .put_to_graph()
                .await?;
        }
        self.promote_if_stub(user_id).await
    }

    /// Promotes a stub to an in-scope user: binds it to its HS and gives it a cursor
    /// at 0, so it is read from the start of its stream. A no-op for any other user.
    async fn promote_if_stub(&self, user_id: &PubkyId) -> ModelResult<()> {
        if !is_stub(user_id.as_ref()).await? {
            return Ok(());
        }

        let Some(hs_id) = self.ensure_hs_not_blacklisted(user_id).await? else {
            tracing::warn!("Not promoting {user_id}: no published HS");
            return Ok(());
        };

        // Bind first: if clearing the state fails, the user is still a stub and the
        // next follow retries the whole promotion.
        Self::bind_to_homeserver(user_id.as_ref(), &hs_id).await?;
        exec_single_row(queries::put::set_user_scope_state(user_id.as_ref(), None)).await?;
        tracing::info!("Promoted {user_id} from stub: now in scope");
        Ok(())
    }

    /// Binds the user to its HS (HOSTED_BY + resolved_at) and seeds its cursor, which
    /// is what makes the watcher poll it.
    async fn bind_to_homeserver(user_id: &str, hs_id: &str) -> ModelResult<()> {
        set_user_homeserver(user_id, hs_id).await?;

        // Seed the user's HS cursor floor (no-op if we're already tracking them).
        UserHsCursor::init(user_id, hs_id).await?;

        Ok(())
    }
}

/// Whether the user is known but kept outside the scope.
pub async fn is_stub(user_id: &str) -> ModelResult<bool> {
    let query = queries::get::get_user_scope_state(user_id);
    let scope_state: Option<Option<String>> = fetch_key_from_graph(query, "scope_state").await?;
    Ok(scope_state.flatten().as_deref() == Some(SCOPE_STATE_STUB))
}
