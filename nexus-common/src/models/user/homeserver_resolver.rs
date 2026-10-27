use pubky::pkarr::dns::rdata::RData;
use pubky::pkarr::SignedPacket;
use pubky::{Pubky, PublicKey};
use pubky_app_specs::PubkyId;

use crate::db::{PubkyClientResult, PubkyConnector};

/// A user's PKDNS record, as the DHT or a relay answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PkdnsRecord {
    /// No packet was found. On pubky 0.9.3 DHT and relay failures surface this
    /// way too, so it carries no information about the user's homeserver.
    Missing,
    /// The user's signed packet, verified by the client that fetched it.
    Published(SignedPacket),
}

impl PkdnsRecord {
    /// The homeserver the record names: the target of its `_pubky` SVCB/HTTPS
    /// record, if there is one and it is a public key.
    pub fn homeserver(&self) -> Option<PubkyId> {
        match self {
            Self::Published(packet) => published_homeserver(packet),
            Self::Missing => None,
        }
    }

    /// The packet's timestamp in microseconds, authored by the key holder.
    /// Orders records of one key against each other and nothing else.
    pub fn timestamp(&self) -> Option<u64> {
        match self {
            Self::Published(packet) => Some(packet.timestamp().as_u64()),
            Self::Missing => None,
        }
    }
}

/// Resolves a user's currently published homeserver from PKDNS/DHT.
///
/// Abstracted behind a trait so callers can route the lookup through their own client, or
/// drive it with a mock in tests instead of hitting the network.
#[async_trait::async_trait]
pub trait PkdnsHomeserverResolver: Send + Sync {
    /// Looks up the PKDNS record of `user_pk`.
    async fn resolve_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord>;

    /// Returns the HS published for `user_pk`, if any is currently published.
    async fn resolve_homeserver(&self, user_pk: &PublicKey) -> PubkyClientResult<Option<PubkyId>> {
        Ok(self.resolve_record(user_pk).await?.homeserver())
    }
}

/// Looks up the PKDNS record of `user_pk` through `pubky`: the lookup behind
/// `Pubky::get_homeserver_of`, which folds a missing packet and a packet naming
/// no homeserver into one `None`.
pub async fn pkdns_record_of(pubky: &Pubky, user_pk: &PublicKey) -> PkdnsRecord {
    match pubky.client().pkarr().resolve(user_pk).await {
        Some(packet) => PkdnsRecord::Published(packet),
        None => PkdnsRecord::Missing,
    }
}

/// The homeserver a PKDNS record names: the target of its `_pubky` SVCB/HTTPS
/// record, if that is a public key. Mirrors `Pubky::get_homeserver_of`, whose
/// parsing the SDK does not export.
fn published_homeserver(packet: &SignedPacket) -> Option<PubkyId> {
    let target = packet
        .resource_records("_pubky")
        .find_map(|rr| match &rr.rdata {
            RData::SVCB(svcb) => Some(svcb.target.to_string()),
            RData::HTTPS(https) => Some(https.0.target.to_string()),
            _ => None,
        })?;
    PublicKey::try_from_z32(&target).ok().map(PubkyId::from)
}

/// Resolver backed by the shared [`PubkyConnector`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PubkyConnectorResolver;

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for PubkyConnectorResolver {
    async fn resolve_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        let pubky = PubkyConnector::get()?;
        Ok(pkdns_record_of(&pubky, user_pk).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils::{random_pubky_id, signed_packet_naming};
    use pubky::pkarr::dns::{rdata::SVCB, Name};
    use pubky::Keypair;

    /// The `_pubky` target is read from HTTPS and SVCB records the way the SDK
    /// reads it; a packet without one, or with a target that is no key, names none.
    #[test]
    fn published_homeserver_reads_the_pubky_target() {
        let user = Keypair::random();
        let homeserver = Keypair::random().public_key();
        let homeserver_id = PubkyId::try_from(homeserver.to_z32().as_str()).expect("valid id");
        let name = || Name::try_from("_pubky").expect("valid name");
        fn svcb(target: &str) -> SVCB<'_> {
            SVCB::new(0, target.try_into().expect("valid target"))
        }

        let https = SignedPacket::builder()
            .https(name(), svcb(&homeserver.to_z32()), 3600)
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&https), Some(homeserver_id.clone()));

        let svcb_record = SignedPacket::builder()
            .svcb(name(), svcb(&homeserver.to_z32()), 3600)
            .sign(&user)
            .expect("signed");
        assert_eq!(published_homeserver(&svcb_record), Some(homeserver_id));

        let other_name = SignedPacket::builder()
            .https(
                Name::try_from("_other").expect("valid name"),
                svcb("example.com"),
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

    /// A published record exposes the homeserver it names and its timestamp; a
    /// missing one has neither.
    #[test]
    fn record_exposes_the_homeserver_and_the_timestamp() {
        let homeserver = random_pubky_id();
        let user = Keypair::random();

        let naming = PkdnsRecord::Published(signed_packet_naming(&user, Some(&homeserver), 42));
        assert_eq!(naming.homeserver(), Some(homeserver));
        assert_eq!(naming.timestamp(), Some(42));

        let naming_none = PkdnsRecord::Published(signed_packet_naming(&user, None, 42));
        assert_eq!(naming_none.homeserver(), None);
        assert_eq!(naming_none.timestamp(), Some(42));

        assert_eq!(PkdnsRecord::Missing.homeserver(), None);
        assert_eq!(PkdnsRecord::Missing.timestamp(), None);
    }
}
