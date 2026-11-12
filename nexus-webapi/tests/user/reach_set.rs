use crate::utils::{get_request, host_url, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::json;

// docker/test-graph/mocks/search-reach.cypher
const OBS: &str = "wnhrmj3b1tt3n6fr7fhedgak4q11e9i1uxm4dmiactgeobyu9wpy";
const FRIEND: &str = "x4rt7xeww7k48jwoomu8gwhsa3t775okm9onhc9dzmwpm8mzupay";
const FOLLOWED: &str = "xbmdh5bobi9593poakgdy8yao7c3z6yjwsbikcw3qmwpa5aonwsy";
const D2: &str = "xzujjk4ubtxcmqcb18itbcgmxf3qyobb7nwi7g88byq3bm1udcqo";
const STRANGER: &str = "y8cjhsxigtj5oc3nuxx75rudprw98za6o9rbah84u1j4mzprbydo";
/// Followed by OBS, with no post to its name.
const LURKER: &str = "zy8gjbx3xoi4j7cgudcajwxg3y6ybc7if8zwiznn4mfy84t5yjco";
const UNKNOWN: &str = "w8phaw75htdp4pkchuicp76yn1ycwhaixaeqw6zhfubg641ogn8y";

/// The Web of Trust as `source=wot` reads it, the observer never in it, followed users first.
#[tokio_shared_rt::test(shared)]
async fn test_reach_set_by_depth() -> Result<()> {
    let route = format!("/v1/user/{OBS}/reach-set");
    let two_steps = json!({"authors": [FRIEND, FOLLOWED, LURKER, D2], "truncated": false});
    assert_eq!(get_request(&route).await?, two_steps, "default depth 2");
    assert_eq!(get_request(&format!("{route}?depth=2")).await?, two_steps);
    assert_eq!(get_request(&format!("{route}?depth=3")).await?, two_steps);
    assert_eq!(
        get_request(&format!("{route}?depth=1")).await?,
        json!({"authors": [FRIEND, FOLLOWED, LURKER], "truncated": false})
    );
    assert_eq!(
        get_request(&format!("/v1/user/{STRANGER}/reach-set")).await?,
        json!({"authors": [], "truncated": false})
    );
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_reach_set_rejects_what_it_cannot_answer() -> Result<()> {
    for depth in ["0", "4", "256", "x"] {
        invalid_get_request(
            &format!("/v1/user/{OBS}/reach-set?depth={depth}"),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }
    invalid_get_request("/v1/user/not-a-user/reach-set", StatusCode::BAD_REQUEST).await?;
    invalid_get_request(
        &format!("/v1/user/{UNKNOWN}/reach-set"),
        StatusCode::NOT_FOUND,
    )
    .await?;
    // A tag held for a user that is gone does not turn the 404 into a 304
    let unknown = httpc_test::new_client("")?
        .reqwest_client()
        .get(format!("{}/v1/user/{UNKNOWN}/reach-set", host_url().await))
        .header("if-none-match", "*")
        .send()
        .await?;
    assert_eq!(unknown.status().as_u16(), 404);
    // `/v0` is frozen: the route is `/v1` only
    invalid_get_request(&format!("/v0/user/{OBS}/reach-set"), StatusCode::NOT_FOUND).await?;
    Ok(())
}

/// The `ETag` names the set; an `If-None-Match` naming it is a 304 with no body.
#[tokio_shared_rt::test(shared)]
async fn test_reach_set_etag_and_not_modified() -> Result<()> {
    let url = format!("{}/v1/user/{OBS}/reach-set", host_url().await);
    let client = httpc_test::new_client("")?;
    let get = |if_none_match: Option<String>, depth: u8| {
        let mut request = client.reqwest_client().get(format!("{url}?depth={depth}"));
        if let Some(tag) = if_none_match {
            request = request.header("if-none-match", tag);
        }
        request.send()
    };

    let first = get(None, 2).await?;
    assert_eq!(first.status().as_u16(), 200);
    let etag = first.headers()["etag"].to_str()?.to_string();
    assert!(etag.starts_with("W/\"") && etag.ends_with('"'), "{etag}");
    let again = get(None, 2).await?;
    assert_eq!(
        again.headers()["etag"].to_str()?,
        etag,
        "a stable set keeps its tag"
    );

    let strong = etag.trim_start_matches("W/").to_string();
    for held in [
        etag.clone(),
        strong,
        format!("\"other\", {etag}"),
        "*".to_string(),
    ] {
        let response = get(Some(held.clone()), 2).await?;
        assert_eq!(response.status().as_u16(), 304, "{held}");
        assert_eq!(response.headers()["etag"].to_str()?, etag);
        assert!(response.text().await?.is_empty());
    }

    // The tags of several fields are one list
    let repeated = client
        .reqwest_client()
        .get(format!("{url}?depth=2"))
        .header("if-none-match", "\"other\"")
        .header("if-none-match", etag.as_str())
        .send()
        .await?;
    assert_eq!(repeated.status().as_u16(), 304);

    // Another tag, a quoted tag holding `*` and commas, or the tag of another set, is
    // answered in full
    for held in ["\"other\"", "\"other,*,tag\""] {
        let stale = get(Some(held.into()), 2).await?;
        assert_eq!(stale.status().as_u16(), 200, "{held}");
    }
    let one_step = get(Some(etag.clone()), 1).await?;
    assert_eq!(one_step.status().as_u16(), 200);
    assert_ne!(one_step.headers()["etag"].to_str()?, etag);
    Ok(())
}
