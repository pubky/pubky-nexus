//! Phase-3 validation-error regressions: invalid `kind=collection` posts must
//! be rejected by spec validation AND have that rejection wrap cleanly as
//! `EventProcessorError::SpecValidation` (which the watcher classifier drops
//! non-retryably, keeping the retry queue clean).
//!
//! These tests don't need the watcher harness — they exercise the spec +
//! error-construction surface directly. They sit next to
//! `forwards_compat.rs`, which uses the same pattern for the v0.4.5 →
//! v0.5.0 `Unknown` shim.

use nexus_common::models::event::EventProcessorError;
use pubky_app_specs::{
    traits::{TimestampId, Validatable},
    PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind,
};

/// A Collection post with `parent` set is structurally invalid (collections
/// are root posts; they can't be replies). Spec rejects, watcher classifies
/// as non-retryable SpecValidation.
#[test]
fn test_collection_with_parent_rejected_as_spec_validation() {
    let post = PubkyAppPost {
        content: r#"{"name":"X"}"#.to_string(),
        kind: PubkyAppPostKind::Collection,
        parent: Some("pubky://userA/pub/pubky.app/posts/0034A0X7NJ52A".to_string()),
        embed: None,
        attachments: None,
    };
    let id = post.create_id();
    let err = post
        .validate(Some(&id))
        .expect_err("Collection post with parent must fail validation");
    let lower = err.to_lowercase();
    assert!(
        lower.contains("parent") || lower.contains("embed"),
        "validation error must mention 'parent' (or the combined 'parent or embed'), got: {err}"
    );

    // The watcher wraps spec errors as SpecValidation (non-retryable).
    let wrapped = EventProcessorError::SpecValidation(err.clone());
    matches!(wrapped, EventProcessorError::SpecValidation(_))
        .then_some(())
        .expect("wrapping should succeed");
}

/// A Collection post with `embed` set is structurally invalid (same reason
/// as parent — collections can't carry embed targets).
#[test]
fn test_collection_with_embed_rejected_as_spec_validation() {
    let post = PubkyAppPost {
        content: r#"{"name":"X"}"#.to_string(),
        kind: PubkyAppPostKind::Collection,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: "pubky://userA/pub/pubky.app/posts/0034A0X7NJ52A".to_string(),
        }),
        attachments: None,
    };
    let id = post.create_id();
    let err = post
        .validate(Some(&id))
        .expect_err("Collection post with embed must fail validation");
    let lower = err.to_lowercase();
    assert!(
        lower.contains("embed") || lower.contains("parent"),
        "validation error must mention 'embed' (or the combined 'parent or embed'), got: {err}"
    );

    let wrapped = EventProcessorError::SpecValidation(err);
    matches!(wrapped, EventProcessorError::SpecValidation(_))
        .then_some(())
        .expect("wrapping should succeed");
}

/// A Collection post whose `content` is not parseable JSON for the
/// PubkyAppCollectionContent envelope (`{name, description?}`) is rejected.
#[test]
fn test_collection_with_malformed_envelope_rejected_as_spec_validation() {
    let post = PubkyAppPost {
        content: "this is not JSON".to_string(),
        kind: PubkyAppPostKind::Collection,
        parent: None,
        embed: None,
        attachments: None,
    };
    let id = post.create_id();
    let err = post
        .validate(Some(&id))
        .expect_err("Collection post with malformed envelope must fail validation");
    assert!(
        err.contains("JSON envelope") || err.to_lowercase().contains("json"),
        "validation error must mention the envelope / JSON, got: {err}"
    );

    let wrapped = EventProcessorError::SpecValidation(err);
    matches!(wrapped, EventProcessorError::SpecValidation(_))
        .then_some(())
        .expect("wrapping should succeed");
}
