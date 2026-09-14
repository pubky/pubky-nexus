//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use std::sync::Arc;
use std::time::Duration;

use pubky::{Keypair, PublicKey};
use pubky_app_specs::PubkyId;

use crate::media::MediaSubprocess;
use crate::models::user::UserIngestor;

/// Generates a random public key.
pub fn random_pk() -> PublicKey {
    Keypair::random().public_key()
}

/// Generates a random z32-encoded public key, usable as a user or HS ID.
pub fn random_pubky_id() -> PubkyId {
    PubkyId::from(random_pk())
}

/// Default user ingestor for tests: empty HS blacklist (ingest everything).
pub fn default_ingestor_tests() -> Arc<UserIngestor> {
    Arc::new(UserIngestor::default())
}

/// Media subprocess runner for tests: a deadline long enough never to fire on real work.
pub fn default_subprocess_tests() -> MediaSubprocess {
    MediaSubprocess::new(Duration::from_secs(30))
}
