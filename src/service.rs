use pubky_nexus::_service::NexusApi;

#[tokio::main]
async fn main() {
    NexusApi::builder().run().await
}
