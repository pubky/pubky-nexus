//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use tokio::sync::oneshot;

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

/// A point in the external reference work of a post where a test can hold its handler: see
/// [`pause_references`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferencePause {
    /// The start of `references::stage`, right after the post write it reconciles.
    Stage,
    /// In `references::flush`, after it read the outbox and before it appends it.
    Flush,
}

type PauseSignals = (oneshot::Sender<()>, oneshot::Receiver<()>);

/// Pauses armed by [`pause_references`], per point and author.
static REFERENCE_PAUSES: LazyLock<Mutex<HashMap<(ReferencePause, String), PauseSignals>>> =
    LazyLock::new(Default::default);

/// Holds the next handler of `author`'s posts that reaches `point`, once. Returns a receiver
/// that resolves when one reached it, and the sender that lets it go on.
pub fn pause_references(
    point: ReferencePause,
    author: &str,
) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (reached_tx, reached_rx) = oneshot::channel();
    let (release_tx, release_rx) = oneshot::channel();
    REFERENCE_PAUSES
        .lock()
        .unwrap()
        .insert((point, author.to_string()), (reached_tx, release_rx));
    (reached_rx, release_tx)
}

/// Waits at `point` if [`pause_references`] armed it for `author`.
pub(crate) async fn reference_pause(point: ReferencePause, author: &str) {
    let armed = REFERENCE_PAUSES
        .lock()
        .unwrap()
        .remove(&(point, author.to_string()));
    if let Some((reached, release)) = armed {
        let _ = reached.send(());
        let _ = release.await;
    }
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
