use pubky::pkarr::dns::rdata::RData;
use pubky::pkarr::SignedPacket;
use pubky::{Pubky, PublicKey};
use pubky_app_specs::PubkyId;

use crate::db::{PubkyClientError, PubkyClientResult, PubkyConnector};
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

    /// The record as kept on the user's `HOSTED_BY` edge, when there is one and
    /// its timestamp fits the graph.
    pub fn stored(&self) -> Option<StoredRecord> {
        match self {
            Self::Published(packet) => StoredRecord::try_from(packet).ok(),
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
    /// Decodes the packet and checks that `user` signed it: the record is proof
    /// of the user's publication only when this succeeds.
    pub fn verify(&self, user: &PublicKey) -> Result<SignedPacket, DynError> {
        let bytes = hex::decode(&self.packet)?;
        let (key, payload) = bytes
            .split_at_checked(32)
            .ok_or("stored record shorter than a public key")?;
        if key != user.as_bytes() {
            return Err("stored record is signed by another key".into());
        }
        Ok(SignedPacket::from_relay_payload(
            user,
            &payload.to_vec().into(),
        )?)
    }
}

/// Fails when the packet's timestamp, a `u64`, does not fit the graph's `i64`.
impl TryFrom<&SignedPacket> for StoredRecord {
    type Error = std::num::TryFromIntError;

    fn try_from(packet: &SignedPacket) -> Result<Self, Self::Error> {
        Ok(Self {
            packet: hex::encode(packet.as_bytes()),
            timestamp: i64::try_from(packet.timestamp().as_u64())?,
        })
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

    /// Looks up the PKDNS record of `user_pk`, refusing a packet nexus cannot
    /// apply:
    /// - One signed by another key fails the lookup. pkarr 6.0.1 on mainline
    ///   6.2.0 checks a DHT answer against the key the answer carries, not the
    ///   key asked for, so any DHT node can answer with another key's packet.
    /// - One whose timestamp does not fit the graph counts as no record. pkarr
    ///   reads a DHT item's `seq` as a `u64`, so an item signed with a negative
    ///   `seq` arrives with such a timestamp; its signature does not verify as a
    ///   pkarr packet, so it could not be kept as proof either.
    async fn resolve_own_record(&self, user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        let record = self.resolve_record(user_pk).await?;
        let PkdnsRecord::Published(packet) = &record else {
            return Ok(record);
        };
        if packet.public_key() != *user_pk.as_inner() {
            return Err(PubkyClientError::RequestFailed {
                message: format!(
                    "PKDNS answer for {} is signed by {}",
                    user_pk.z32(),
                    packet.public_key().to_z32()
                ),
            });
        }
        if StoredRecord::try_from(packet).is_err() {
            tracing::warn!(
                user_id = %user_pk.z32(),
                timestamp = packet.timestamp().as_u64(),
                "PKDNS record timestamp does not fit the graph; taken as no record"
            );
            return Ok(PkdnsRecord::Missing);
        }
        Ok(record)
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

    /// A published record exposes the homeserver it names, if any; a missing
    /// one names none.
    #[test]
    fn record_exposes_the_homeserver() {
        let homeserver = random_pubky_id();
        let user = Keypair::random();

        let naming = PkdnsRecord::Published(signed_packet_naming(&user, Some(&homeserver), 42));
        assert_eq!(naming.homeserver(), Some(homeserver));

        let naming_none = PkdnsRecord::Published(signed_packet_naming(&user, None, 42));
        assert_eq!(naming_none.homeserver(), None);

        assert_eq!(PkdnsRecord::Missing.homeserver(), None);
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

        let verified = stored
            .verify(&user.public_key())
            .expect("the stored record verifies");
        assert_eq!(verified.as_bytes(), packet.as_bytes());
        assert_eq!(published_homeserver(&verified), Some(homeserver));

        assert!(
            stored.verify(&Keypair::random().public_key()).is_err(),
            "a record signed by another key must not verify"
        );

        // Flip the last nibble of the timestamp (hex 192..208): the packet still
        // parses, so only the signature check can reject it.
        let mut tampered = stored.clone();
        let nibble = tampered.packet.remove(207);
        tampered
            .packet
            .insert(207, if nibble == '0' { '1' } else { '0' });
        assert!(
            tampered.verify(&user.public_key()).is_err(),
            "a tampered record must not verify"
        );

        assert_eq!(PkdnsRecord::Missing.stored(), None);
    }

    /// Answers every lookup with one fixed record.
    struct FixedRecordResolver(PkdnsRecord);

    #[async_trait::async_trait]
    impl PkdnsHomeserverResolver for FixedRecordResolver {
        async fn resolve_record(&self, _: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
            Ok(self.0.clone())
        }
    }

    /// A lookup answered with another key's packet fails; the user's own packet
    /// and a missing record pass through.
    #[tokio::test]
    async fn own_record_refuses_another_key() {
        let user = Keypair::random();
        let homeserver = random_pubky_id();

        let foreign = FixedRecordResolver(PkdnsRecord::Published(signed_packet_naming(
            &Keypair::random(),
            Some(&homeserver),
            42,
        )));
        assert!(foreign
            .resolve_own_record(&user.public_key())
            .await
            .is_err());

        let own_record = PkdnsRecord::Published(signed_packet_naming(&user, Some(&homeserver), 42));
        let own = FixedRecordResolver(own_record.clone());
        assert_eq!(
            own.resolve_own_record(&user.public_key()).await.ok(),
            Some(own_record)
        );

        let missing = FixedRecordResolver(PkdnsRecord::Missing);
        assert_eq!(
            missing.resolve_own_record(&user.public_key()).await.ok(),
            Some(PkdnsRecord::Missing)
        );
    }

    /// The user's own packet with a timestamp past `i64::MAX` is not stored,
    /// and a lookup answered with it counts as no record.
    #[tokio::test]
    async fn record_past_the_graph_timestamp_range_counts_as_none() {
        let user = Keypair::random();
        let homeserver = random_pubky_id();
        let packet = signed_packet_naming(&user, Some(&homeserver), i64::MAX as u64 + 1);

        let record = PkdnsRecord::Published(packet);
        assert_eq!(record.stored(), None);

        let resolver = FixedRecordResolver(record);
        assert_eq!(
            resolver.resolve_own_record(&user.public_key()).await.ok(),
            Some(PkdnsRecord::Missing)
        );
    }
}
