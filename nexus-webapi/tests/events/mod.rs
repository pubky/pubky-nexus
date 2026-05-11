use crate::utils::host_url;
use anyhow::Result;

/// Regression tests for overflow-safe cursor/limit handling (PR #683).

#[tokio_shared_rt::test(shared)]
async fn test_events_huge_cursor() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;

    // A cursor at u64::MAX must not panic or overflow; it should return
    // an empty event list and echo back the cursor value.
    let res = client
        .do_get(&format!("{url}/v0/events?cursor=18446744073709551615"))
        .await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("cursor:"), "response must have a cursor line");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_limit_zero() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;

    // limit=0 must return HTTP 200 with an empty event list and cursor at 0.
    let res = client.do_get(&format!("{url}/v0/events?limit=0")).await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert_eq!(body.trim(), "cursor: 0");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_large_cursor_with_limit() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;

    // isize::MAX cursor with a non-zero limit must not overflow and must
    // return HTTP 200 (empty events because the index is out of range).
    let res = client
        .do_get(&format!(
            "{url}/v0/events?cursor=9223372036854775807&limit=100"
        ))
        .await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("cursor:"), "response must have a cursor line");

    Ok(())
}
