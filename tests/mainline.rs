use anyhow::Result;
use mainline::Testnet;
use pubky_homeserver::Homeserver;

#[tokio::test]
async fn test_mainline() -> Result<()> {
    // let mut test = WatcherTest::setup().await?;
    env_logger::try_init()?;

    let testnet = Testnet::new(10)?;

    let homeserver = Homeserver::start_test(&testnet).await.unwrap();
    let url = homeserver.url();

    let client = pubky::Client::builder().testnet(&testnet).build().unwrap();

    let response = client
        .get(format!("{url}events/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert_eq!(response, "");

    Ok(())
}
