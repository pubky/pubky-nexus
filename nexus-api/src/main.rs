use nexus_api::builder::NexusApi;
use nexus_common::types::DynError;

#[tokio::main]
async fn main() -> Result<(), DynError> {
    println!("Starting api service...");
    // Run watcher logic here
    NexusApi::builder().run().await
}
