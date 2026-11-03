use std::sync::Arc;

use pubky_app_specs::{ParsedUri, PubkyId};

use super::{PkdnsHomeserverResolver, StoredRecord};
use crate::db::{fetch_key_from_graph, fetch_row_from_graph, queries};
use crate::models::error::{ModelError, ModelResult};
use crate::models::homeserver::HsBlacklist;
use crate::models::scope::{scope_from_config, EverythingScope, Scope};
use crate::models::traits::Collection;
use crate::models::user::outsider::{admit_outsider, create_outsider};
use crate::models::user::{set_user_homeserver_from_record, UserDetails, UserHsCursor};
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
        Ok(self
            .resolve_unless_blacklisted(user_id)
            .await?
            .map(|(hs_id, _)| hs_id))
    }

    /// The user's published HS and the record naming it, unless the user's own
    /// key or that HS is blacklisted. `None` when the user publishes no HS.
    async fn resolve_unless_blacklisted(
        &self,
        user_id: &PubkyId,
    ) -> ModelResult<Option<(String, StoredRecord)>> {
        // `user_id` may itself be an HS PK (e.g. a file `src` of the form
        // `pubky://<hs_pk>/...` that addresses the HS directly). The resolver
        // returns no HS for an HS PK, so without this self-check a blacklisted HS
        // used as a direct source would slip through and we'd reach out to it.
        if self.hs_blacklist.is_blacklisted(user_id.as_ref()) {
            return Err(ModelError::HsBlacklisted {
                hs_id: user_id.to_string(),
            });
        }

        let record = self
            .resolver
            .resolve_own_record(&user_id.to_public_key())
            .await
            .map_err(ModelError::from_generic)?;
        let (Some(stored), Some(hs_pk)) = (record.stored(), record.homeserver()) else {
            return Ok(None);
        };

        let hs_id = hs_pk.to_string();
        if self.hs_blacklist.is_blacklisted(&hs_id) {
            return Err(ModelError::HsBlacklisted { hs_id });
        }

        Ok(Some((hs_id, stored)))
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

        let resolved = self
            .resolve_unless_blacklisted(user_id)
            .await
            .inspect_err(|e| tracing::warn!("Aborting ingestion of {user_id}: {e}"))?;

        let Some((hs_id, record)) = resolved else {
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

        // Bind the user to their HS (HOSTED_BY + resolved_at) from the record just resolved
        set_user_homeserver_from_record(&user_id_str, &hs_id, &record).await?;

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

    /// Makes sure a user that must be in the scope (the moderator) is known and polled.
    ///
    /// If its HS can't be resolved right now, the bare node is still created: the HS
    /// resolver binds it on a later run, and polling starts from there. A node kept as an
    /// outsider (the API ingests with the plain scope, which doesn't pin the moderator) is
    /// brought back into the scope.
    pub async fn ensure_in_scope(&self, user_id: &PubkyId) -> ModelResult<()> {
        self.maybe_ingest_user(user_id).await?;
        if UserDetails::get_by_id(user_id).await?.is_none() {
            UserDetails::from_pubky(user_id.clone())
                .put_to_graph()
                .await?;
        }

        let query = queries::get::get_user_scope_state(user_id);
        let scope_state: Option<Option<String>> =
            fetch_key_from_graph(query, "scope_state").await?;
        if scope_state.flatten().is_none() {
            return Ok(());
        }
        // Bound first and let in last, so a failure before leaves it an outsider (never
        // polled meanwhile) and the next start retries the whole of it.
        if let Some((hs_id, record)) = self.resolve_unless_blacklisted(user_id).await? {
            set_user_homeserver_from_record(user_id, &hs_id, &record).await?;
            UserHsCursor::init(user_id, &hs_id).await?;
        }
        admit_outsider(user_id).await?;
        tracing::info!("Brought {user_id} back into the scope: it must always be read");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pubky::{Keypair, PublicKey};

    use super::*;
    use crate::db::{
        exec_single_row, fetch_row_from_graph, graph::Query, queries, PubkyClientResult,
    };
    use crate::models::user::PkdnsRecord;
    use crate::utils::test_utils::{random_pubky_id, signed_packet_naming};
    use crate::{types::DynError, StackConfig, StackManager};

    /// Answers every lookup with `keypair`'s own record naming one homeserver.
    struct SigningResolver {
        keypair: Keypair,
        homeserver: PubkyId,
    }

    #[async_trait::async_trait]
    impl PkdnsHomeserverResolver for SigningResolver {
        async fn resolve_record(&self, _: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
            Ok(PkdnsRecord::Published(signed_packet_naming(
                &self.keypair,
                Some(&self.homeserver),
                7,
            )))
        }
    }

    /// A newly seen user is bound from the record just resolved, which stays on
    /// the edge as the user's own signed packet.
    #[tokio_shared_rt::test(shared)]
    async fn ingested_user_is_bound_from_the_resolved_record() -> Result<(), DynError> {
        StackManager::setup(&StackConfig::default()).await?;
        let user = Keypair::random();
        let user_id = PubkyId::from(user.public_key());
        let homeserver = random_pubky_id();
        let ingestor = UserIngestor::new(
            [],
            Arc::new(SigningResolver {
                keypair: user,
                homeserver: homeserver.clone(),
            }),
        );

        ingestor.maybe_ingest_user(&user_id).await?;

        let query = Query::new(
            "ingested_user_record",
            "MATCH (u:User {id: $user_id})-[r:HOSTED_BY]->(hs:Homeserver)
             RETURN hs.id AS hs_id, r.record AS packet, r.record_ts AS timestamp",
        )
        .param("user_id", user_id.to_string());
        let row = fetch_row_from_graph(query)
            .await?
            .expect("the user is bound");
        assert_eq!(row.get::<String>("hs_id")?, homeserver.to_string());
        let stored = StoredRecord {
            packet: row.get("packet")?,
            timestamp: row.get("timestamp")?,
        };
        assert_eq!(stored.timestamp, 7);
        let packet = stored.verify(&user_id.to_public_key())?;
        assert_eq!(
            PkdnsRecord::Published(packet).homeserver(),
            Some(homeserver)
        );

        exec_single_row(queries::del::delete_user(user_id.as_ref())).await?;
        Ok(())
    }

    /// The blacklist check looks the user's homeserver up through the injected resolver.
    #[tokio::test]
    async fn blacklist_check_uses_the_injected_resolver() {
        let blacklisted = random_pubky_id();
        let allowed = random_pubky_id();
        let user = Keypair::random();
        let user_id = PubkyId::from(user.public_key());

        let refusing = UserIngestor::new(
            [blacklisted.clone()],
            Arc::new(SigningResolver {
                keypair: user.clone(),
                homeserver: blacklisted.clone(),
            }),
        );
        let err = refusing
            .ensure_hs_not_blacklisted(&user_id)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ModelError::HsBlacklisted { hs_id } if hs_id == blacklisted.to_string())
        );

        let accepting = UserIngestor::new(
            [blacklisted],
            Arc::new(SigningResolver {
                keypair: user,
                homeserver: allowed.clone(),
            }),
        );
        let hs_id = accepting.ensure_hs_not_blacklisted(&user_id).await.unwrap();
        assert_eq!(hs_id, Some(allowed.to_string()));
    }

    /// A lookup answered with another key's record fails, so the user is not
    /// taken to be hosted where that record points.
    #[tokio::test]
    async fn record_signed_by_another_key_is_refused() {
        let user_id = random_pubky_id();
        let ingestor = UserIngestor::new(
            [],
            Arc::new(SigningResolver {
                keypair: Keypair::random(),
                homeserver: random_pubky_id(),
            }),
        );

        let err = ingestor
            .ensure_hs_not_blacklisted(&user_id)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("is signed by"), "{err}");
    }
}
