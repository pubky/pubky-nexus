use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use crate::db::PubkyConnector;
use crate::db::{exec_single_row, fetch_key_from_graph, queries};
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::scope::{scope_from_config, watcher_scope, EverythingScope, Scope};
use crate::models::traits::Collection;
use crate::models::user::stub::{mark_stub, unstub};
use crate::models::user::{set_user_homeserver, UserDetails, UserHsCursor};
use crate::{StackConfig, WatcherConfig};

/// `scope_state` of a member that left the scope: no longer polled, content kept.
pub const SCOPE_STATE_LEFT: &str = "left";

/// Ingests previously-unknown users unless their HS is blacklisted.
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

    /// The watcher's ingestor: like [`Self::from_config`], with the moderator always in
    /// scope.
    pub fn from_watcher_config(config: &WatcherConfig) -> Self {
        Self::with_scope(
            config.stack.net.external_hs_pk_blacklist.iter().cloned(),
            watcher_scope(config),
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
    /// A user in the scope is bound to its HS and gets a cursor, so it is polled. A user
    /// outside it is kept as a stub; see [`crate::models::user::stub`].
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
            return mark_stub(&user_id_str).await;
        }

        Self::bind_to_homeserver(&user_id_str, &hs_id).await
    }

    /// Makes sure a user that must be in the scope (the moderator, a seed) is known and
    /// polled.
    ///
    /// If its HS can't be resolved right now, the bare node is still created: the HS
    /// resolver binds it on a later run, and polling starts from there.
    pub async fn ensure_in_scope(&self, user_id: &PubkyId) -> ModelResult<()> {
        self.maybe_ingest_user(user_id).await?;
        if UserDetails::get_by_id(user_id).await?.is_none() {
            UserDetails::from_pubky(user_id.clone())
                .put_to_graph()
                .await?;
        }
        self.promote(user_id).await?;
        Ok(())
    }

    /// Brings a user kept outside the scope (a stub, or a member that left) back in:
    /// binds it to its HS with a cursor at 0, so it is read from the start of its
    /// stream, and adds it to the rankings.
    ///
    /// Returns whether the user is in scope now: `true` if it was promoted or already in,
    /// `false` if it is unknown or has no published HS, so it couldn't be bound.
    pub async fn promote(&self, user_id: &PubkyId) -> ModelResult<bool> {
        let query = queries::get::get_user_scope_state(user_id);
        let scope_state: Option<Option<String>> =
            fetch_key_from_graph(query, "scope_state").await?;
        match scope_state {
            None => return Ok(false),
            Some(None) => return Ok(true),
            Some(Some(_)) => {}
        }

        let Some(hs_id) = self.ensure_hs_not_blacklisted(user_id).await? else {
            tracing::warn!("Not promoting {user_id}: no published HS");
            return Ok(false);
        };

        // Bind first: if clearing the state fails, the user is still out of scope and
        // the next promotion retries the whole of it.
        Self::bind_to_homeserver(user_id, &hs_id).await?;
        exec_single_row(queries::put::set_user_scope_state(user_id, None)).await?;
        unstub(user_id).await?;
        tracing::info!("Brought {user_id} into the scope");
        Ok(true)
    }

    /// Takes a member that left the scope out of it: marks it `left`, unbinds it from its
    /// HS and forgets its cursor, so it is no longer polled. Its content stays; if it
    /// comes back, it is promoted and read from the start of its stream again.
    pub async fn demote(&self, user_id: &str) -> ModelResult<()> {
        exec_single_row(queries::put::unbind_user_from_scope(
            user_id,
            SCOPE_STATE_LEFT,
        ))
        .await?;
        UserHsCursor::remove_all(user_id).await?;
        tracing::info!("{user_id} left the scope");
        Ok(())
    }

    /// Binds the user to its HS (HOSTED_BY + resolved_at) and seeds its cursor, which is
    /// what makes the watcher poll it.
    async fn bind_to_homeserver(user_id: &str, hs_id: &str) -> ModelResult<()> {
        set_user_homeserver(user_id, hs_id).await?;

        // Seed the user's HS cursor floor (no-op if we're already tracking them).
        UserHsCursor::init(user_id, hs_id).await?;

        Ok(())
    }
}
