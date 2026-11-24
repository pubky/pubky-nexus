//! Nexus's side of the homeserver client: the PKDNS lookup in Nexus's terms, and the
//! conversions between Nexus's IDs and the client's keys.

use nexus_common::db::PubkyClientResult;
use nexus_common::models::user::{PkdnsHomeserverResolver, PkdnsRecord};
use pubky::errors::PkarrError;
use pubky::PublicKey;
use pubky_social_specs::PubkyId;

use crate::homeserver::{
    HomeserverClient, HomeserverResolver, HsError, HsErrorKind, HsOp, PubkyHomeserverClient,
};

/// The homeserver client the watcher runs on: every homeserver call, plus the PKDNS lookup
/// with Nexus's checks ([`PkdnsHomeserverResolver::resolve_own_record`]), which the user
/// ingestor, the periodic user-to-homeserver resolver, and the primary homeserver's
/// processor (for users without a `HOSTED_BY` edge) use.
pub trait NexusHomeserverClient: HomeserverClient + PkdnsHomeserverResolver {}

impl<T: HomeserverClient + PkdnsHomeserverResolver + ?Sized> NexusHomeserverClient for T {}

/// Rebuilds the client's PKDNS answer as Nexus's [`PkdnsRecord`]: a packet is
/// [`PkdnsRecord::Published`], no packet is [`PkdnsRecord::Missing`], and a DHT or relay
/// failure is the pkarr resolve error inside `PubkyClientError`, as `pkdns_record_of` returns.
#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyHomeserverClient {
    async fn resolve_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        match HomeserverResolver::resolve_record(self, user_pk).await {
            Ok(Some(packet)) => Ok(PkdnsRecord::Published(packet)),
            Ok(None) => Ok(PkdnsRecord::Missing),
            Err(e) => Err(PkarrError::Resolve(e).into()),
        }
    }
}

/// The key of homeserver `id`, for a call to it. A format-valid ID need not be a curve point;
/// such an ID fails `op` as a malformed request, labelled with the ID.
pub(crate) fn hs_key(id: &PubkyId, op: HsOp) -> Result<PublicKey, HsError> {
    id.to_public_key().map_err(|e| HsError {
        hs: id.to_string(),
        op,
        kind: HsErrorKind::malformed(e),
    })
}
