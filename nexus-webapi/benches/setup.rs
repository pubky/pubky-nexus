use nexus_common::{Level, StackConfig, StackManager};
use std::sync::Once;
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

pub fn run_setup() {
    INIT.call_once(|| {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let config = StackConfig {
                log_level: Level::Error,
                ..Default::default()
            };
            let _ = StackManager::setup("benchmark", &config).await;
        });
    });
}
