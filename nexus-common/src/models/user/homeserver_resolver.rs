use pubky::pkarr::dns::rdata::RData;
use pubky::pkarr::SignedPacket;
use pubky::{Pubky, PublicKey};
use pubky_app_specs::PubkyId;

use crate::db::{PubkyClientResult, PubkyConnector};
use crate::types::DynError;

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

    /// The record as kept on the user's `HOSTED_BY` edge, when there is one.
    pub fn stored(&self) -> Option<StoredRecord> {
        match self {
            Self::Published(packet) => Some(StoredRecord::from(packet)),
            Self::Missing => None,
        }
    }
}

/// A PKDNS record as kept on a user's `HOSTED_BY` edge: the signed packet's
/// canonical bytes, hex-encoded, and the packet's timestamp. The bytes carry
/// the key, the signature, the timestamp and the encoded DNS packet, so anyone
/// can check that the key holder published that record, see [`Self::verify`].
/// The timestamp is kept beside them for Cypher, which cannot read the packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRecord {
    /// `SignedPacket::as_bytes` as lowercase hex.
    pub packet: String,
    /// The packet's timestamp in microseconds, as stored in the graph.
    pub timestamp: i64,
}

impl StoredRecord {
    /// Decodes the packet and checks its signature: the record is proof of the
    /// key holder's publication only when this succeeds.
    pub fn verify(&self) -> Result<SignedPacket, DynError> {
        let bytes = hex::decode(&self.packet)?;
        let (key, payload) = bytes
            .split_at_checked(32)
            .ok_or("stored record shorter than a public key")?;
        let public_key = pubky::pkarr::PublicKey::try_from(key)?;
        Ok(SignedPacket::from_relay_payload(
            &public_key,
            &payload.to_vec().into(),
        )?)
    }
}

impl From<&SignedPacket> for StoredRecord {
    fn from(packet: &SignedPacket) -> Self {
        Self {
            packet: hex::encode(packet.as_bytes()),
            timestamp: i64::try_from(packet.timestamp().as_u64()).unwrap_or(i64::MAX),
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

    /// A stored record decodes back to the packet it came from, signature
    /// checked; a tampered one does not verify.
    #[test]
    fn stored_record_verifies_the_packet_it_came_from() {
        let user = Keypair::random();
        let homeserver = random_pubky_id();
        let packet = signed_packet_naming(&user, Some(&homeserver), 42);
        let stored = PkdnsRecord::Published(packet.clone())
            .stored()
            .expect("a published record is stored");
        assert_eq!(stored.timestamp, 42);

        let verified = stored.verify().expect("the stored record verifies");
        assert_eq!(verified.as_bytes(), packet.as_bytes());
        assert_eq!(verified.public_key().to_z32(), user.public_key().to_z32());
        assert_eq!(published_homeserver(&verified), Some(homeserver));

        // Flip the last nibble of the timestamp (hex 192..208): the packet still
        // parses, so only the signature check can reject it.
        let mut tampered = stored.clone();
        let nibble = tampered.packet.remove(207);
        tampered
            .packet
            .insert(207, if nibble == '0' { '1' } else { '0' });
        assert!(
            tampered.verify().is_err(),
            "a tampered record must not verify"
        );

        assert_eq!(PkdnsRecord::Missing.stored(), None);
    }
}
