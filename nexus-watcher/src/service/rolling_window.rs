use crate::events::TEventProcessorFactory;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::types::DynError;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use tracing::error;

type ProcessorResultType = Result<(u64, u64), DynError>;

pub async fn run_processors(
    event_processor_factory: Arc<dyn TEventProcessorFactory>,
    shutdown_rx: Receiver<bool>,
) -> ProcessorResultType {
    let hs_ids = Homeserver::get_all_from_graph()
        .await
        .expect("No Homeserver IDs found in graph");

    let mut processed_homeservers = 0;
    let mut skipped_homeservers = 0;
    
    for hs_id in hs_ids {
        let event_processor = event_processor_factory.build(hs_id).await?;
        match event_processor.run(shutdown_rx.clone()).await {
            Ok(_) => {
                processed_homeservers += 1;
            }
            Err(e) => {
                error!("Error while processing events: {:?}", e);
                skipped_homeservers += 1;
            }
        }
    }

    Ok((processed_homeservers, skipped_homeservers))
}
