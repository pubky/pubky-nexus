//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use std::collections::HashSet;
use std::sync::{Arc, LazyLock, Mutex};

use pubky::pkarr::dns::{rdata::SVCB, Name};
use pubky::pkarr::{SignedPacket, Timestamp};
use pubky::{Keypair, PublicKey};
use pubky_social_specs::PubkyId;

use crate::db::kv::{RedisError, RedisResult};
use crate::models::user::{PubkyConnectorResolver, UserIngestor};

/// Authors whose resource changes feed appends of moved tags fail: see [`fail_move_appends`].
static MOVE_APPEND_FAULTS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Makes every append of a moved tag of `author` to the resource changes feed fail with a
/// Redis error, as an unreachable Redis does, until it is called again with `fail` false.
pub fn fail_move_appends(author: &str, fail: bool) {
    let mut faults = MOVE_APPEND_FAULTS.lock().unwrap();
    if fail {
        faults.insert(author.to_string());
    } else {
        faults.remove(author);
    }
}

/// The error [`fail_move_appends`] injects for `author`, if any.
pub(crate) fn move_append_fault(author: &str) -> RedisResult<()> {
    if MOVE_APPEND_FAULTS.lock().unwrap().contains(author) {
        return Err(RedisError::IoError("injected move append failure".into()));
    }
    Ok(())
}

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

/// Default user ingestor for tests: empty HS blacklist (ingest everything).
pub fn default_ingestor_tests() -> Arc<UserIngestor> {
    Arc::new(UserIngestor::new([], Arc::new(PubkyConnectorResolver)))
}
