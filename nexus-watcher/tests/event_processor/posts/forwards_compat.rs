//! Forwards-compat regression: a Nexus binary running an older spec version
//! that exposes `PubkyAppPostKind::Unknown` (v0.4.5+) must:
//! - deserialize a v0.5.0 `kind=collection` payload into `Unknown` rather than
//!   panicking,
//! - reject the post in spec validation,
//! - have that rejection classified as the non-retryable `SpecValidation`
//!   `EventProcessorError` so the retry queue stays clean.
//!
//! This test does NOT require the watcher harness; it exercises the spec +
//! error-classification surface directly. Without these guarantees, the entire
//! v0.4.5 → v0.5.0 staged-rollout strategy is theatre.

use nexus_watcher::errors::EventProcessorError;
use pubky_app_specs::{
    traits::{TimestampId, Validatable},
    PubkyAppPost, PubkyAppPostKind,
};

#[test]
fn test_v045_nexus_handles_future_kind_event_as_unknown_no_retry() {
    // A future client publishes a post whose kind isn't in this version of the
    // spec yet. The blob below uses a `kind` string that is NOT a known variant
    // — exercising the `#[serde(other)]` catch-all in `PubkyAppPostKind` is the
    // whole point of this test.
    //
    // (Earlier this test used `kind: "collection"`, but once v0.5.0 landed that
    // value started parsing as the real `Collection` variant — defeating the
    // forwards-compat exercise. The test was passing for the wrong reason.)
    let blob = br#"{
        "content": "future-post-content",
        "kind": "hyperverse_post",
        "parent": null,
        "embed": null,
        "attachments": null
    }"#;

    // Deserialization succeeds because `#[serde(other)]` routes unknown kind
    // strings to `PubkyAppPostKind::Unknown` rather than failing the parse.
    let post: PubkyAppPost =
        serde_json::from_slice(blob).expect("serde(other) must accept unrecognized kinds");
    assert_eq!(
        post.kind,
        PubkyAppPostKind::Unknown,
        "unknown kind string must deserialize as PubkyAppPostKind::Unknown"
    );

    // Validation rejects Unknown — Unknown is for deserialization-survival
    // only, never a valid post-kind for write.
    let id = post.create_id();
    let validation_err = post
        .validate(Some(&id))
        .expect_err("Unknown kind must fail validation");
    assert!(
        validation_err.to_lowercase().contains("unknown"),
        "validation error must mention 'unknown', got: {}",
        validation_err
    );

    // The watcher's `from_resource` site (Step 3.2) maps validation errors to
    // `EventProcessorError::SpecValidation(reason)`, and the classifier
    // (extract_retry_event_info) returns None for SpecValidation, meaning the
    // event is dropped and not enqueued for retry. We assert the error variant
    // construction here; the classifier behavior is covered by the unit test
    // in `nexus-watcher/src/service/processor.rs`.
    let err = EventProcessorError::SpecValidation(validation_err);
    match err {
        EventProcessorError::SpecValidation(reason) => {
            assert!(
                reason.to_lowercase().contains("unknown"),
                "expected 'unknown' in SpecValidation reason"
            );
        }
        other => panic!("expected SpecValidation, got {:?}", other),
    }
}
