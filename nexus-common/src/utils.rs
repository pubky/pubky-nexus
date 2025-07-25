use tokio::sync::watch::Receiver;

/// Creates a watch channel that can be used for shutdown signalling.
///
/// On Ctrl-C, it sends a signal that can be picked up by the receiver returned.
pub fn create_channel() -> Receiver<bool> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        let _ = shutdown_tx.send(true);
    });
    shutdown_rx
}
