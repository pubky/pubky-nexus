//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use std::sync::Arc;

use pubky::{Keypair, PublicKey};
use pubky_app_specs::PubkyId;

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
