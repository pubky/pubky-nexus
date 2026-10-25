use crate::utils::{host_url, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::models::event::resource_changes::{
    ResourceChange, ResourceChangeOp, RESOURCE_CHANGES_KEY,
};
use serde_json::json;

// Exclusive in nextest because this test replaces the feed list/epoch.
#[tokio_shared_rt::test(shared)]
async fn pages_headers_bounds_and_read_only_empty_feed() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;
    let mut conn = get_redis_conn().await?;
    let epoch_key = format!("{RESOURCE_CHANGES_KEY}:Epoch");
    let _: () = conn
        .del(&[RESOURCE_CHANGES_KEY, epoch_key.as_str()])
        .await?;
    let path = format!("{url}/v0/resources/changes");
    let empty = client.do_get(&path).await?;
    assert_eq!(empty.status(), 200);
    assert_eq!(empty.json_body()?, json!({"lines": [], "next_cursor": 0}));
    assert_eq!(empty.header("x-events-length").as_deref(), Some("0"));
    assert_eq!(empty.header("x-events-epoch"), None);
    assert!(!conn.exists::<_, bool>(&epoch_key).await?);
    assert!(!conn.exists::<_, bool>(RESOURCE_CHANGES_KEY).await?);

    let author = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
    for (index, op) in [ResourceChangeOp::Put, ResourceChangeOp::Del]
        .into_iter()
        .enumerate()
    {
        ResourceChange::tag(
            op,
            "19f876713c227a568d939946496a0d30".into(),
            "https://example.com/".into(),
            author.into(),
            "example".into(),
            &format!("pubky://{author}/pub/pubky.app/tags/example"),
            index as i64,
        )
        .unwrap()
        .store()
        .await?;
    }
    let first = client.do_get(&format!("{path}?limit=1")).await?;
    assert_eq!(first.status(), 200);
    assert_eq!(first.header("x-events-length").as_deref(), Some("2"));
    let epoch = first.header("x-events-epoch").expect("initialized epoch");
    assert_eq!(first.json_body()?["next_cursor"], 1);
    assert_eq!(first.json_body()?["lines"][0]["op"], "put");
    assert!(first
        .header("access-control-expose-headers")
        .unwrap()
        .contains("x-events-epoch"));
    let second = client.do_get(&format!("{path}?cursor=1&limit=1")).await?;
    assert_eq!(second.json_body()?["lines"][0]["op"], "del");
    assert_eq!(second.json_body()?["next_cursor"], 2);
    assert_eq!(
        second.header("x-events-epoch").as_deref(),
        Some(epoch.as_str())
    );
    for cursor in [2, u64::MAX] {
        let response = client
            .do_get(&format!("{path}?cursor={cursor}&limit=1000"))
            .await?;
        assert_eq!(
            response.json_body()?,
            json!({"lines": [], "next_cursor": cursor})
        );
    }
    for query in [
        "limit=0",
        "limit=1001",
        "cursor=-1",
        "cursor=18446744073709551616",
    ] {
        invalid_get_request(
            &format!("/v0/resources/changes?{query}"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }
    ResourceChange::reseed().await?;
    let rebuilt = client.do_get(&path).await?;
    assert_ne!(
        rebuilt.header("x-events-epoch").as_deref(),
        Some(epoch.as_str())
    );
    Ok(())
}
