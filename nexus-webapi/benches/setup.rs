use nexus_common::{models::post::create_post_content_index, Level, StackConfig, StackManager};
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
            let _ = StackManager::setup(&config).await;
            // index is create through migration script, we call it explicitly here
            create_post_content_index().await.unwrap();
        });
    });
}
