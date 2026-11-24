//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use std::sync::Arc;

use pubky::pkarr::dns::{rdata::SVCB, Name};
use pubky::pkarr::{SignedPacket, Timestamp};
use pubky::{Keypair, PublicKey};
use pubky_social_specs::PubkyId;

use crate::db::PubkyClientResult;
use crate::models::user::{PkdnsHomeserverResolver, PkdnsRecord, UserIngestor};

/// Generates a random public key.
pub fn random_pk() -> PublicKey {
    Keypair::random().public_key()
}

/// Generates a random z32-encoded public key, usable as a user or HS ID.
pub fn random_pubky_id() -> PubkyId {
    PubkyId::from(random_pk())
}

/// A PKDNS packet signed by `keypair` at `timestamp` (microseconds), naming
/// `homeserver` in its `_pubky` record, or naming none.
pub fn signed_packet_naming(
    keypair: &Keypair,
    homeserver: Option<&PubkyId>,
    timestamp: u64,
) -> SignedPacket {
    let builder = SignedPacket::builder().timestamp(Timestamp::from(timestamp));
    let builder = match homeserver {
        Some(homeserver) => builder.https(
            Name::try_from("_pubky").expect("valid name"),
            SVCB::new(0, homeserver.as_ref().try_into().expect("valid target")),
            3600,
        ),
        None => builder,
    };
    builder.sign(keypair).expect("a signed packet")
}

/// A PKDNS resolver that finds no record for any user, without a network.
struct NoRecordResolver;

#[async_trait::async_trait]
impl PkdnsHomeserverResolver for NoRecordResolver {
    async fn resolve_record(&self, _user_pk: &PublicKey) -> PubkyClientResult<PkdnsRecord> {
        Ok(PkdnsRecord::Missing)
    }
}

/// Default user ingestor for tests: empty HS blacklist (ingest everything), and no lookups:
/// every user publishes no homeserver, so no unknown user is ingested. A test that needs
/// a lookup builds its ingestor over its own client.
pub fn default_ingestor_tests() -> Arc<UserIngestor> {
    Arc::new(UserIngestor::new([], Arc::new(NoRecordResolver)))
}
