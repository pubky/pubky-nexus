pub mod test_utils;

use tokio::sync::watch::Receiver;

// blake3 truncated to 128 bits. Shared by resource_id and IndexKey::for_uri — fine, separate keyspaces.
pub fn hash_str_hex(input: &str) -> String {
    let hash = blake3::hash(input.as_bytes());
    hex::encode(&hash.as_bytes()[..16])
}

/// Creates a watch channel that can be used for shutdown signalling.
///
/// On Ctrl-C, it sends a signal that can be picked up by the receiver returned.
pub fn create_shutdown_rx() -> Receiver<bool> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });
    shutdown_rx
}
