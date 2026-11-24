//! Nexus's side of the homeserver client: the client the watcher holds, and the conversion
//! from Nexus's IDs to the client's keys. The PKDNS lookup in Nexus's terms is implemented
//! for [`WatcherClient`](pubky_watcher::WatcherClient) in `nexus-common`.

use nexus_common::models::user::PkdnsHomeserverResolver;
use pubky::PublicKey;
use pubky_social_specs::PubkyId;

use pubky_watcher::{
    HomeserverEventSource, HsError, HsErrorKind, HsOp, KeyEventSource, ResourceReader,
};

/// The homeserver client the watcher runs on: every homeserver call, plus the PKDNS lookup
/// with Nexus's checks ([`PkdnsHomeserverResolver::resolve_own_record`]), which the user
/// ingestor, the periodic user-to-homeserver resolver, and the primary homeserver's
/// processor (for users without a `HOSTED_BY` edge) use.
pub trait NexusHomeserverClient:
    HomeserverEventSource + KeyEventSource + ResourceReader + PkdnsHomeserverResolver
{
}

impl<T> NexusHomeserverClient for T where
    T: HomeserverEventSource + KeyEventSource + ResourceReader + PkdnsHomeserverResolver + ?Sized
{
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
