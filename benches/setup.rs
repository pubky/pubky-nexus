use pubky_nexus::{Config, StackManager};
use std::{env, sync::Once};
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

pub fn run_setup() {
    INIT.call_once(|| {
        let rt = Runtime::new().unwrap();
        env::set_var("RUST_LOG", "error");
        rt.block_on(async {
            let config = Config::from_env();
            StackManager::setup(&config).await;
        });
    });
}
