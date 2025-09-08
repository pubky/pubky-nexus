use crate::events::TEventProcessorFactory;
use crate::events::errors::EventProcessorError;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use tokio::time::timeout;
use std::sync::Arc;
use tracing::error;

type ProcessorResultType = Result<(u64, u64), DynError>;

pub async fn run_processors(
    event_processor_factory: Arc<dyn TEventProcessorFactory>
) -> ProcessorResultType {
    let hs_ids = Homeserver::get_all_from_graph()
        .await
        .expect("No Homeserver IDs found in graph");

    let mut processed_homeservers = 0;
    let mut skipped_homeservers = 0;

    for hs_id in hs_ids {
        let Ok(event_processor) = event_processor_factory.build(hs_id.clone()).await else {
            error!("Failed to build event processor for homeserver: {}", hs_id);
            continue;
        };
        match timeout(event_processor_factory.timeout(), event_processor.run(event_processor_factory.shutdown_rx())).await {
            Ok(Ok(_)) => processed_homeservers += 1,
            Ok(Err(e)) => {
                if let Some(EventProcessorError::ShutdownRequested) = e.as_ref().downcast_ref::<EventProcessorError>() {
                    skipped_homeservers += 1;
                    continue;
                }
                error!("Event processor failed for {}: {:?}", hs_id, e);
                skipped_homeservers += 1;
            }
            Err(_) => {
                error!("Event processor timed out for {}", hs_id);
                skipped_homeservers += 1;
            }
        } 
    }

    Ok((processed_homeservers, skipped_homeservers))
}