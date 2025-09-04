use crate::service::utils::{
    MockEventProcessor, MockEventProcessorFactory, MockEventProcessorResult,
};
use anyhow::{Error, Result};
use nexus_common::models::homeserver::Homeserver;
use nexus_watcher::service::{rolling_window::run_processors, NexusWatcher};
use pubky::Keypair;
use pubky_app_specs::PubkyId;
use std::{collections::HashMap, time::Duration, sync::Arc};

#[tokio_shared_rt::test(shared)]
async fn test_sequential_event_processing() -> Result<()> {

    if let Err(e) = NexusWatcher::builder().init_test_stack().await {
        return Err(Error::msg(format!("could not initialise the stack, {e:?}")));
    }

    let mut event_processor_hashmap: HashMap<String, MockEventProcessor> = HashMap::new();
    create_random_homeservers(&mut event_processor_hashmap, None, MockEventProcessorResult::Success("Success finished!".to_string())).await;
    create_random_homeservers(&mut event_processor_hashmap, None, MockEventProcessorResult::Success("Success finished!".to_string())).await;
    create_random_homeservers(&mut event_processor_hashmap, None, MockEventProcessorResult::Success("Success finished!".to_string())).await;
    create_random_homeservers(&mut event_processor_hashmap, None, MockEventProcessorResult::Success("Success finished!".to_string())).await;


    let factory = MockEventProcessorFactory::new(event_processor_hashmap);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let result = run_processors(Arc::new(factory), shutdown_rx).await.unwrap();
    assert_eq!(result.0, 4);
    assert_eq!(result.1, 0);

    Ok(())
}

async fn create_random_homeservers(event_processor_hashmap: &mut HashMap<String, MockEventProcessor>, timeout: Option<Duration>, processor_status: MockEventProcessorResult) {
    let homeserver_keypair = Keypair::random();
    let homeserver_public_key = homeserver_keypair.public_key().to_z32();

    let config_hs = PubkyId::try_from(homeserver_public_key.as_str()).unwrap();
    Homeserver::persist_if_unknown(config_hs).await.unwrap();
    
    let event_processor = MockEventProcessor {
        homeserver_id: homeserver_public_key.clone(),
        timeout,
        processor_status,
    };
    event_processor_hashmap.insert(homeserver_public_key.clone(), event_processor);
}
