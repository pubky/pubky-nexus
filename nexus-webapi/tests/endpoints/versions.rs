//! The API answers under `/v0` and `/v1` from one route declaration, so the same request
//! gets the same answer on both. One representative route per module, against the mock data.

use crate::utils::host_url;

use anyhow::Result;
use nexus_webapi::routes::v0::endpoints::{V0_PREFIX, V1_PREFIX};
use serde_json::Value;

const PUBKY_USER: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const PUBKY_POST: &str = "2ZCW1TGR5BKG0";
const PUBKY_FILE: &str = "2ZK2H8P2T5NG0";
// Aldert, from docker/test-graph/mocks/posts.cypher
const AUTHOR: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const AMSTERDAM: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
const NOTIFIED_USER: &str = "qrqnkbwqt8rd6aya9zruro76gyc4qs77qzek7xc11axbdtbu6esy";
const TAGGER: &str = "78guxwtzgtgpskij51om7t66awmqxznr6p7ogonfohoags6ahc5y";
const TAG_ID: &str = "2Z1N8QBQK9EG0";
const RESOURCE_ID: &str = "450a72e3da164bfc3ac5f4056f9e5c7c"; // https://example.com/article

/// One route per module, relative to the version prefix. Every one of them is a 200 on the
/// mock data, so a difference in the answer is a difference between the mounts.
fn representative_routes() -> Vec<String> {
    let file_uri = url::form_urlencoded::byte_serialize(
        format!("pubky://{PUBKY_USER}/pub/pubky.app/files/{PUBKY_FILE}").as_bytes(),
    )
    .collect::<String>();
    vec![
        // info
        "/info".to_string(),
        // user
        format!("/user/{PUBKY_USER}/details"),
        // post
        format!("/post/{PUBKY_USER}/{PUBKY_POST}/details"),
        // stream
        format!("/stream/posts?author_id={AUTHOR}&source=author&sorting=timeline&limit=5"),
        // search
        "/search/users/by_name/Jo".to_string(),
        // tag
        format!("/tags/{TAGGER}/{TAG_ID}"),
        // resource
        format!("/resource/{RESOURCE_ID}/tags"),
        // notification
        format!("/user/{NOTIFIED_USER}/notifications?limit=2"),
        // bootstrap
        format!("/bootstrap/{AMSTERDAM}"),
        // events
        "/events?cursor=0&limit=5".to_string(),
        // file
        format!("/files/file/{file_uri}"),
    ]
}

/// A response's body as the client typed it: JSON as a value, anything else (the events
/// list is plain text) as its text.
fn body(response: &httpc_test::Response) -> Result<Value> {
    Ok(response
        .json_body()
        .or_else(|_| response.text_body().map(Value::String))?)
}

/// The bootstrap's `influencers` is a random window of the ranking on every call, the
/// `users` list carries their views, and its `posts` and `files` come back in no fixed
/// order, so two answers never match whole: compare the rest, with the posts in id order
/// and the files in uri order.
fn deterministic_part(route: &str, mut body: Value) -> Value {
    if route.starts_with("/bootstrap/") {
        body["ids"]["influencers"].take();
        body["users"].take();
        if let Some(posts) = body["posts"].as_array_mut() {
            posts.sort_by_key(|post| post["details"]["id"].to_string());
        }
        if let Some(files) = body["files"].as_array_mut() {
            files.sort_by_key(|file| file["uri"].to_string());
        }
    }
    body
}

#[tokio_shared_rt::test(shared)]
async fn test_every_module_answers_the_same_on_both_versions() -> Result<()> {
    let client = httpc_test::new_client(host_url().await)?;

    for route in representative_routes() {
        let v0 = client.do_get(&format!("{V0_PREFIX}{route}")).await?;
        let v1 = client.do_get(&format!("{V1_PREFIX}{route}")).await?;
        assert_eq!(v0.status(), 200, "{V0_PREFIX}{route}");
        assert_eq!(v1.status(), v0.status(), "{route}");
        assert_eq!(
            deterministic_part(&route, body(&v1)?),
            deterministic_part(&route, body(&v0)?),
            "{route}"
        );
    }

    Ok(())
}
