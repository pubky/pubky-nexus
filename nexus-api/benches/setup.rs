use nexus_common::{Level, StackConfig, StackManager};
use std::sync::Once;
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

pub fn run_setup() {
    INIT.call_once(|| {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut config = StackConfig::default();
            config.log_level = Level::Error;
            let _ = StackManager::setup("benchmark", &config).await;
        });
    });
}
