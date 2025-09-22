use nexus_common::types::DynError;

/// Result of a homeserver event processing
pub enum MockEventProcessorResult {
    Success,
    Error(DynError),
    Panic,
}
