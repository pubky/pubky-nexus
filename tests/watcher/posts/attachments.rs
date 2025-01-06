use anyhow::Result;
use mainline::Testnet;
use std::time::Duration;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_post_attachments() -> Result<()> {
    // let mut test = WatcherTest::setup().await?;
    env_logger::try_init();

    let testnet = Testnet::new(10)?;

    std::thread::sleep(Duration::from_secs(5));
    panic!("nothing");
}
