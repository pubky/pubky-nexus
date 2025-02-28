use std::{env, sync::Once};
use pubky_nexus::_service::NexusApi;
use tokio::runtime::Runtime;

static INIT: Once = Once::new();

pub fn run_setup() {
    INIT.call_once(|| {
        let rt = Runtime::new().unwrap();
        env::set_var("RUST_LOG", "error");
        rt.block_on(async {
            NexusApi::builder().init_stack().await;
        });
    });
}
