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

/// Authors whose resource changes feed del lines fail right after they are appended: see
/// [`fail_feed_dels_after_append`].
static FEED_DEL_FAULTS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Makes every del line of `author` appended to the resource changes feed report a Redis
/// error once it is in the list, as a handler that stops between its append and its graph
/// delete does, until it is called again with `fail` false.
pub fn fail_feed_dels_after_append(author: &str, fail: bool) {
    let mut faults = FEED_DEL_FAULTS.lock().unwrap();
    if fail {
        faults.insert(author.to_string());
    } else {
        faults.remove(author);
    }
}

/// The error [`fail_feed_dels_after_append`] injects for `author`, if any.
pub(crate) fn feed_del_fault(author: &str) -> RedisResult<()> {
    if FEED_DEL_FAULTS.lock().unwrap().contains(author) {
        return Err(RedisError::IoError(
            "injected failure after a del append".into(),
        ));
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
