use super::{assert_unavailable, light_request, setup_light_stack};
use anyhow::Result;
use axum::http::{Method, StatusCode};
use chrono::Utc;
use nexus_common::db::{exec_single_row, queries};
use nexus_common::models::post::{PostDetails, PostRelationships};
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{UserDetails, UserSearch};
use nexus_common::utils::test_utils::random_pubky_id;
use pubky_app_specs::{post_uri_builder, PubkyAppPostKind};

// A post from the mock graph; the 501s answer before any read.
const AUTHOR_ID: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
const POST_ID: &str = "2ZCW1TGR5BKG0";

/// A light Nexus keeps no post content, so it has no content search, and no item lists,
/// so it has no collection feeds.
#[tokio_shared_rt::test(shared)]
async fn test_light_content_search_and_collection_feeds_are_unavailable() -> Result<()> {
    for uri in [
        "/v0/search/posts/by_content?q=hello".to_string(),
        format!("/v0/stream/posts?source=collection&author_id={AUTHOR_ID}&post_id={POST_ID}"),
        format!("/v0/stream/posts/keys?source=collection&author_id={AUTHOR_ID}&post_id={POST_ID}"),
        format!("/v0/stream/posts?source=post_collections&author_id={AUTHOR_ID}&post_id={POST_ID}"),
        format!(
            "/v0/stream/posts/keys?source=post_collections&author_id={AUTHOR_ID}&post_id={POST_ID}"
        ),
    ] {
        let (status, body) = light_request(Method::GET, &uri, None).await?;
        assert_unavailable(status, &body, &uri);
    }
    Ok(())
}

/// Post details are served without the content, with its hash, so a client fetches the
/// content from the homeserver and can cache it until the hash changes.
///
/// The post is a fixture of this test's own: a light API caches what it reads without
/// content, so reading the shared mock data would leave it stripped for later tests.
#[tokio_shared_rt::test(shared)]
async fn test_light_post_details_carry_no_content() -> Result<()> {
    setup_light_stack().await;

    let author_id = random_pubky_id();
    UserDetails::from_pubky(author_id.clone())
        .put_to_graph()
        .await?;

    // A valid 13-character Crockford id.
    let post_id = "0034ABCDEFGH0";
    let content = "Light:Posts:Fixture content";
    let fixture = PostDetails {
        content: content.to_string(),
        content_hash: Some(PostDetails::hash_content(content)),
        id: post_id.to_string(),
        indexed_at: Utc::now().timestamp_millis(),
        author: author_id.to_string(),
        kind: PubkyAppPostKind::Short,
        uri: post_uri_builder(author_id.to_string(), post_id.into()),
        ..PostDetails::default()
    };
    fixture.put_to_graph(&PostRelationships::default()).await?;

    let (status, body) = light_request(
        Method::GET,
        &format!("/v0/post/{author_id}/{post_id}/details"),
        None,
    )
    .await?;

    // The read cached the post and put it on the timelines; the cleanup removes all of it.
    PostDetails::delete_from_index(&author_id, post_id, None).await?;
    exec_single_row(queries::del::delete_post(&author_id, post_id)).await?;
    UserSearch::delete(&author_id).await?;
    UserDetails::delete(&author_id).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], post_id);
    assert!(body.get("content").is_none(), "no content: {body}");
    assert_eq!(body["content_hash"], PostDetails::hash_content(content));
    assert_eq!(body["uri"], fixture.uri);
    Ok(())
}
