use nexus_common::db::PubkyClient;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::user::UserDetails;
use nexus_common::types::DynError;
use pubky::PublicKey;
use pubky_app_specs::{ParsedUri, PubkyId};

/// Service module for homeserver management operations
pub struct HomeserverManager;

impl HomeserverManager {
    /// If a referenced post is hosted on a new, unknown homeserver, this method triggers ingestion of that homeserver.
    ///
    /// ### Arguments
    ///
    /// - `referenced_post_uri`: The parent post (if current post is a reply to it), or a reposted post (if current post is a Repost)
    pub async fn maybe_ingest_for_post(referenced_post_uri: &str) -> Result<(), DynError> {
        let parsed_post_uri = ParsedUri::try_from(referenced_post_uri)?;
        let ref_post_author_id = parsed_post_uri.user_id.as_str();

        Self::maybe_ingest_for_user(ref_post_author_id).await
    }

    /// If a referenced user is using a new, unknown homeserver, this method triggers ingestion of that homeserver.
    ///
    /// ### Arguments
    ///
    /// - `referenced_user_id`: The URI of the referenced user
    pub async fn maybe_ingest_for_user(referenced_user_id: &str) -> Result<(), DynError> {
        let pubky_client = PubkyClient::get()?;

        if UserDetails::get_by_id(referenced_user_id).await?.is_some() {
            tracing::debug!("Skipping homeserver ingestion: author {referenced_user_id} already known");
            return Ok(());
        }

        let ref_post_author_pk = referenced_user_id.parse::<PublicKey>()?;
        let Some(ref_post_author_hs) = pubky_client.get_homeserver(&ref_post_author_pk).await else {
            tracing::warn!("Skipping homeserver ingestion: author {ref_post_author_pk} has no published homeserver");
            return Ok(());
        };

        let hs_pk = PubkyId::try_from(&ref_post_author_hs)?;
        if let Ok(Some(_)) = Homeserver::get_by_id(hs_pk.clone()).await {
            tracing::warn!("Skipping homeserver ingestion: author {ref_post_author_pk} not yet known, but their homeserver is known");
            return Ok(());
        }

        Homeserver::new(hs_pk.clone())
            .put_to_graph()
            .await
            .inspect(|_| tracing::info!("Ingested homeserver {hs_pk}"))
            .inspect_err(|e| tracing::error!("Failed to ingest homeserver {hs_pk}: {e}"))
    }
}
