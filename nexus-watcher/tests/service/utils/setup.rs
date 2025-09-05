use std::collections::HashMap;

use anyhow::{Error, Result};
use nexus_watcher::service::NexusWatcher;

use crate::service::utils::MockEventProcessor;

pub async fn setup() -> Result<HashMap<String, MockEventProcessor>> {
    // Initialize the test stack
    if let Err(e) = NexusWatcher::builder().init_test_stack().await {
        return Err(Error::msg(format!("could not initialise the stack, {e:?}")));
    }

    Ok(HashMap::new())
}
