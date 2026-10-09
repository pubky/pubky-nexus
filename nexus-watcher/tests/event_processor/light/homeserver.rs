use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::user::{HomeserverReachability, HsReachability};
use nexus_watcher::service::TEventProcessorRunner;

/// A light watcher records whether each homeserver it polls answers, for the API to hand
/// to clients.
#[tokio_shared_rt::test(shared)]
async fn test_light_watcher_records_that_the_homeserver_answers() -> Result<()> {
    let test = WatcherTest::setup_light(None).await?;
    let hs_id = test.homeserver_id.to_string();
    HsReachability::remove_from_index_multiple_json(&[&[hs_id.as_str()]]).await?;

    test.event_processor_runner
        .run()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let observed = HsReachability::get_many(&[hs_id.as_str()])
        .await?
        .pop()
        .flatten()
        .expect("the poll was recorded");
    assert_eq!(observed.reachability, HomeserverReachability::Ok);
    assert!(observed.observed_at > 0);

    HsReachability::remove_from_index_multiple_json(&[&[hs_id.as_str()]]).await?;
    Ok(())
}

/// A full watcher records nothing: only light clients fetch from homeservers.
#[tokio_shared_rt::test(shared)]
async fn test_full_watcher_records_no_reachability() -> Result<()> {
    let test = WatcherTest::setup(None).await?;
    let hs_id = test.homeserver_id.to_string();

    test.event_processor_runner
        .run()
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    assert_eq!(
        HsReachability::get_many(&[hs_id.as_str()]).await?,
        vec![None]
    );
    Ok(())
}
