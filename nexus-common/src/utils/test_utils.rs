//! # Test Utilities
//!
//! Shared helpers for unit and integration tests.

use pubky::{Keypair, PublicKey};
use pubky_app_specs::PubkyId;

/// Generates a random public key.
pub fn random_pk() -> PublicKey {
    Keypair::random().public_key()
}

/// Generates a random z32-encoded public key, usable as a user or HS ID.
pub fn random_pubky_id() -> PubkyId {
    PubkyId::from(random_pk())
}
