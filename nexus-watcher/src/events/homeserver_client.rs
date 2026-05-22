use nexus_common::db::{PubkyClientError, PubkyConnector};
use pubky::{Method, PubkyHttpClient, PubkyResource};
use pubky_app_specs::PubkyId;
use reqwest::Response;

/// HTTP client bound to a specific homeserver.
///
/// `pubky.public_storage().get(uri)` resolves the *user's* pkarr record on
/// every fetch to discover where the user lives. In the watcher we already
/// know which homeserver produced the event, so going through user-pkarr is
/// redundant and racy — the user could re-point their record between event
/// emission and our fetch. `HomeserverClient` targets the homeserver
/// directly by public key (only `homeserver_id -> url` resolution) and uses
/// the `pubky-host` header to route the tenant on the receiving end.
pub struct HomeserverClient {
    homeserver_id: PubkyId,
    client: PubkyHttpClient,
}

impl HomeserverClient {
    pub fn new(homeserver_id: PubkyId) -> Result<Self, PubkyClientError> {
        let pubky = PubkyConnector::get()?;
        Ok(Self {
            homeserver_id,
            client: pubky.client().clone(),
        })
    }

    /// GET a `pubky://<user>/<path>` URI from the bound homeserver.
    pub async fn get(&self, pubky_uri: &str) -> pubky::Result<Response> {
        let resource: PubkyResource = pubky_uri.parse()?;
        let url = format!("https://{}{}", self.homeserver_id, resource.path.as_str(),);
        let response = self
            .client
            .request(Method::GET, &url)
            .header("pubky-host", resource.owner.z32())
            .send()
            .await?;
        Ok(response)
    }
}
