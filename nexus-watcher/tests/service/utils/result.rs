/// Result of a homeserver event processing
pub enum MockEventProcessorResult {
    Success,
    Error(String),
    Panic,
}
