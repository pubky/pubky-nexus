use std::sync::Arc;
use tokio::time::Duration;
use anyhow::Result;
use nexus_watcher::service::RollingWindow;
use nexus_watcher::events::processor::EventProcessorFactory;

#[tokio_shared_rt::test(shared)]
async fn test_rolling_window() -> Result<()> {
    let mut window = RollingWindow::new(3);
    let mut iter = vec!["homeserver1", "homeserver2", "homeserver3"].into_iter().map(String::from);

    //window.fill(&mut iter, Arc::new(EventProcessorFactory::default()), Duration::from_secs(10), shutdown_rx);

    Ok(())
}