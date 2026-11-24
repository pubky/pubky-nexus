use pubky::pkarr::dns::rdata::RData;
use pubky::pkarr::SignedPacket;
use pubky::PublicKey;

/// Why a PKDNS lookup through a [`HomeserverResolver`] failed: no DHT node or relay gave a
/// usable answer. The underlying error, e.g. pkarr's, is its [`source`](std::error::Error::source).
#[derive(Debug, thiserror::Error)]
#[error("Failed to resolve the DHT record: {source}")]
pub struct ResolveError {
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl ResolveError {
    /// A lookup that failed because of `source`. An implementation of
    /// [`HomeserverResolver`] other than [`WatcherClient`](super::WatcherClient) builds its
    /// failures with it.
    pub fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
        }
    }
}

/// Looks up the PKDNS record a user publishes.
///
/// The record is returned as the DHT or a relay answered it, so that the caller can keep it,
/// check who signed it and when, and refuse an older answer that is replayed or lagging.
/// [`published_homeserver`] reads the homeserver it names.
#[async_trait::async_trait]
pub trait HomeserverResolver: Send + Sync {
    /// The signed packet `user` publishes, or `None` when no packet is found. A DHT or relay
    /// failure is an error, so `None` means no record, not an unknown.
    async fn resolve_record(&self, user: &PublicKey) -> Result<Option<SignedPacket>, ResolveError>;
}

/// The homeserver a PKDNS record names: the target of its `_pubky` SVCB/HTTPS record, if
/// there is one and it is a public key. Mirrors `Pubky::get_homeserver_of`, whose parsing
/// the SDK does not export.
/// Takes the packet [`HomeserverResolver::resolve_record`] returns.
pub fn published_homeserver(packet: &SignedPacket) -> Option<PublicKey> {
    let target = packet
        .resource_records("_pubky")
        .find_map(|rr| match &rr.rdata {
            RData::SVCB(svcb) => Some(svcb.target.to_string()),
            RData::HTTPS(https) => Some(https.0.target.to_string()),
            _ => None,
        })?;
    PublicKey::try_from_z32(&target).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pubky::pkarr::dns::{rdata::SVCB, Name};
    use pubky::Keypair;

    /// The `_pubky` target is read from HTTPS and SVCB records the way the SDK reads it; a
    /// packet without one, or with a target that is no key, names none.
    #[test]
    fn published_homeserver_reads_the_pubky_target() {
        let user = Keypair::random();
        let homeserver = Keypair::random().public_key();
        let name = || Name::try_from("_pubky").expect("valid name");
        fn svcb(target: &str) -> SVCB<'_> {
            SVCB::new(0, target.try_into().expect("valid target"))
        }

        let https = SignedPacket::builder()
            .https(name(), svcb(&homeserver.to_z32()), 3600)
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&https), Some(homeserver.clone()));

        let svcb_record = SignedPacket::builder()
            .svcb(name(), svcb(&homeserver.to_z32()), 3600)
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&svcb_record), Some(homeserver.clone()));

        // A key as the target, so only the name filter can make this `None`.
        let other_name = SignedPacket::builder()
            .https(
                Name::try_from("_other").expect("valid name"),
                svcb(&homeserver.to_z32()),
                3600,
            )
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&other_name), None);

        let domain_target = SignedPacket::builder()
            .https(name(), svcb("example.com"), 3600)
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&domain_target), None);
    }
}
