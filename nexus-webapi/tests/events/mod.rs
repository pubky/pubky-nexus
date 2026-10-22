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

// ---- Filtering by author (`users`, `POST /v0/events/by_users`) ----

use nexus_common::models::event::EventLine;
use serde_json::json;

use nexus_webapi::routes::v0::events::{
    EVENTS_EPOCH_HEADER as EPOCH_HEADER, EVENTS_LENGTH_HEADER as LENGTH_HEADER,
};

/// A fresh author key, so this test's lines are the only ones by it in the log.
fn fresh_author() -> String {
    pubky::Keypair::random().public_key().to_z32()
}

fn line(author: &str, n: usize) -> String {
    format!("PUT pubky://{author}/pub/pubky.app/posts/{n:013}")
}

/// Appends `lines` to the shared event log. They are left in place: removing them
/// would shift positions under tests paging the same log, and their fresh authors
/// already keep them out of every other test's results.
async fn append(lines: Vec<String>) -> Result<Vec<String>> {
    host_url().await; // the test server sets up the stack
    for l in &lines {
        EventLine::new(l.clone()).store().await?;
    }
    Ok(lines)
}

/// Splits a plain-text events body into its lines and its cursor.
fn parse(body: &str) -> (Vec<String>, u64) {
    let mut lines: Vec<String> = body.lines().map(str::to_string).collect();
    let cursor_line = lines.pop().expect("cursor line");
    let cursor = cursor_line
        .strip_prefix("cursor: ")
        .expect("last line is the cursor")
        .parse()
        .expect("numeric cursor");
    (lines, cursor)
}

/// Pages `POST /v0/events/by_users` from 0 to the end of the log.
async fn collect(users: &[&str], limit: usize) -> Result<Vec<String>> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;
    let (mut all, mut cursor) = (Vec::new(), 0u64);
    for _ in 0..10_000 {
        let res = client
            .do_post(
                &format!("{url}/v0/events/by_users"),
                json!({ "users": users, "cursor": cursor, "limit": limit }),
            )
            .await?;
        assert_eq!(res.status(), 200);
        let (lines, next) = parse(&res.text_body()?);
        all.extend(lines);
        if next == cursor {
            return Ok(all);
        }
        cursor = next;
    }
    panic!("paging did not reach the end of the log");
}

#[tokio_shared_rt::test(shared)]
async fn test_events_by_users_returns_only_listed_authors_in_order() -> Result<()> {
    let (a, b, c) = (fresh_author(), fresh_author(), fresh_author());
    append(vec![line(&a, 1), line(&b, 2), line(&c, 3), line(&a, 4)]).await?;

    assert_eq!(collect(&[&a], 500).await?, vec![line(&a, 1), line(&a, 4)]);
    assert_eq!(
        collect(&[&a, &b], 500).await?,
        vec![line(&a, 1), line(&b, 2), line(&a, 4)]
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_by_users_small_limit_pages_without_gaps_or_repeats() -> Result<()> {
    let a = fresh_author();
    let lines = append((0..5).map(|n| line(&a, n)).collect()).await?;

    assert_eq!(collect(&[&a], 1).await?, lines);
    assert_eq!(collect(&[&a], 2).await?, lines);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_by_users_unknown_author_reaches_the_end_empty() -> Result<()> {
    assert!(collect(&[&fresh_author()], 10).await?.is_empty());
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_get_rejects_users_and_points_to_post() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;
    let res = client
        .do_get(&format!("{url}/v0/events?users={}", fresh_author()))
        .await?;
    assert_eq!(res.status(), 400);
    assert!(res
        .json_body()?
        .to_string()
        .contains("POST /v0/events/by_users"));
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_by_users_post_rejects_invalid_requests() -> Result<()> {
    let url = host_url().await;
    let client = httpc_test::new_client("")?;
    let endpoint = format!("{url}/v0/events/by_users");
    let a = fresh_author();
    let too_many: Vec<String> = vec![a.clone(); 1001];
    for body in [
        json!({ "users": [] }),
        json!({ "users": ["not-a-key"] }),
        json!({ "users": too_many }),
        json!({ "users": [a.clone()], "limit": 0 }),
        json!({ "users": [a.clone()], "limit": 1001 }),
        json!({ "users": [a.clone()], "cursr": 5 }),
    ] {
        let res = client.do_post(&endpoint, body.clone()).await?;
        assert_eq!(res.status(), 400, "body={body:.60}");
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_events_identity_headers_are_present_and_consistent() -> Result<()> {
    append(vec![line(&fresh_author(), 1)]).await?; // the epoch exists once a line is stored
    let url = host_url().await;
    let client = httpc_test::new_client("")?;
    let first = client.do_get(&format!("{url}/v0/events?limit=1")).await?;
    let second = client
        .do_post(
            &format!("{url}/v0/events/by_users"),
            json!({ "users": [fresh_author()], "limit": 1 }),
        )
        .await?;

    let epoch = first.header(EPOCH_HEADER).expect("epoch header");
    assert!(!epoch.is_empty());
    assert_eq!(second.header(EPOCH_HEADER), Some(epoch));

    let len: u64 = first
        .header(LENGTH_HEADER)
        .expect("length header")
        .parse()
        .expect("numeric length");
    assert!(len >= 1);
    let later: u64 = second.header(LENGTH_HEADER).unwrap().parse().unwrap();
    assert!(later >= len, "the log only grows between requests");
    Ok(())
}
