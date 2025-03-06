use nexus_common::types::DynError;
use nexus_api::{builder::NexusApi, mock::MockDb};

#[tokio::main]
async fn main() {
    println!("Starting api service...");
    // Run watcher logic here
    //NexusApi::builder().run().await
    MockDb::run(None).await
}

// #[tokio::main]
// async fn main() -> Result<(), DynError> {
//     println!("Starting api service...");
//     // Run watcher logic here
//     NexusApi::builder().run().await
// }