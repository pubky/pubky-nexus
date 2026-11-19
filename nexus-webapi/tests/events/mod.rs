use crate::utils::host_url;
use anyhow::Result;
use deadpool_redis::redis;
use nexus_common::db::{get_redis_conn, RedisOps};
use nexus_common::models::event::EventLine;
use nexus_common::utils::test_utils::random_pubky_id;

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

    // limit=0 is invalid and must be rejected with 400.
    let res = client.do_get(&format!("{url}/v0/events?limit=0")).await?;
    assert_eq!(res.status(), 400);

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

/// The stored list mixes lines with the homeserver key and lines without one. By default every
/// line is served as the homeserver sent it, so the output is what it was before the key was
/// stored; with `homeserver=true` the lines are served as stored. The cursor is the same in
/// both modes, and a read without one starts at the list's first line.
#[tokio_shared_rt::test(shared)]
async fn test_events_homeserver_key_is_opt_in() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;

    let user = random_pubky_id();
    let other = random_pubky_id();
    let homeserver = random_pubky_id();
    let received = [
        format!("PUT pubky://{user}/pub/pubky.app/posts/0032SSN7Q4EVG"),
        // Keyless, with a key-shaped tail of its own
        format!("DEL pubky://{user}/pub/pubky.app/follows/{other}"),
        // Keyed, with a space in its path
        format!("PUT pubky://{user}/pub/pubky.app/files/my holiday photo.jpg"),
    ];
    let stored = [
        EventLine::new(&received[0], &homeserver)
            .as_str()
            .to_string(),
        received[1].clone(),
        EventLine::new(&received[2], &homeserver)
            .as_str()
            .to_string(),
    ];
    let events_key = format!("{}:{}", EventLine::prefix().await, EventLine::LIST);
    // RPUSH returns the length after the push, so the lines sit at [start, start + 3) whatever
    // is appended after them
    let mut conn = get_redis_conn().await?;
    let length: u64 = redis::cmd("RPUSH")
        .arg(&events_key)
        .arg(&stored)
        .query_async(&mut conn)
        .await?;
    let start = length - stored.len() as u64;
    let cursor_line = format!("cursor: {length}");

    let body = |query: &str| {
        let url = format!("{url}/v0/events?cursor={start}&limit=3{query}");
        let client = &client;
        async move {
            let res = client.do_get(&url).await?;
            assert_eq!(res.status(), 200, "{url}");
            res.text_body()
        }
    };
    let plain = body("").await?;
    let stripped = body("&homeserver=false").await?;
    let keyed = body("&homeserver=true").await?;

    let first_stored: String = redis::cmd("LINDEX")
        .arg(&events_key)
        .arg(0)
        .query_async(&mut conn)
        .await?;
    let from_the_start = client
        .do_get(&format!("{url}/v0/events?limit=1&homeserver=true"))
        .await?
        .text_body()?;

    // The lines stay in the list: it is append-only, and removing them would shift the index
    // of every later line under the cursors of whoever reads it.
    assert_eq!(plain, format!("{}\n{cursor_line}", received.join("\n")));
    assert_eq!(stripped, plain);
    assert_eq!(keyed, format!("{}\n{cursor_line}", stored.join("\n")));
    assert_eq!(from_the_start, format!("{first_stored}\ncursor: 1"));
    Ok(())
}
