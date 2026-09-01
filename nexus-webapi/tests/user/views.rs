use crate::{
    tags::user::PUBKY_PEER,
    utils::server::TestServiceServer,
    utils::{get_request, invalid_get_request},
};
use anyhow::Result;
use axum::http::StatusCode;
use deadpool_redis::redis::AsyncCommands;
use nexus_common::db::get_redis_conn;
use nexus_common::db::RedisOps;
use nexus_common::models::user::{SocialGraphStatus, USER_SOCIAL_GRAPH_KEY_PARTS};

#[tokio_shared_rt::test(shared)]
async fn test_user_endpoint() -> Result<()> {
    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}")).await?;

    assert_eq!(res["details"]["name"], "Aldert");
    assert_eq!(res["details"]["status"], "working");
    assert_eq!(res["details"]["id"], user_id);
    assert_eq!(res["details"]["image"], "pubky://4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro/pub/pubky.app/files/003286NSMY490");

    assert_eq!(res["counts"]["tagged"], 10);
    assert_eq!(res["counts"]["tags"], 4);
    assert_eq!(res["counts"]["unique_tags"], 4);
    assert_eq!(res["counts"]["posts"], 4);
    assert_eq!(res["counts"]["replies"], 0);
    assert_eq!(res["counts"]["following"], 15);
    assert_eq!(res["counts"]["followers"], 10);
    assert_eq!(res["counts"]["friends"], 8);
    assert_eq!(res["counts"]["bookmarks"], 0);

    // Test tags on Ar's profile
    let ar_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = get_request(&format!("/v0/user/{ar_id}")).await?;

    //let user_profile: UserView = serde_json::from_value(body)?;
    if let Some(tags) = res.get("tags").and_then(|t| t.as_array()) {
        assert_eq!(tags.len(), 3);
        assert!(
            tags.iter().any(|tag| tag["label"] == "pkarr"),
            "Ar profile should tagged as 'pkarr'"
        );
        assert!(
            tags.iter().any(|tag| tag["label"] == "synonym"),
            "Ar profile should tagged as 'synonym'"
        );
        assert!(
            !tags.iter().any(|tag| tag["label"] == "nonsense"),
            "Ar profile should tagged as 'nonsense'"
        );
    }

    // Look for Aldert pk user id using Flavio's viewer id
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = get_request(&format!("/v0/user/{user_id}?viewer_id={viewer_id}")).await?;

    assert_eq!(
        res["relationship"]["followed_by"], true,
        "Aldert should follow Flavio"
    );
    assert_eq!(
        res["relationship"]["following"], false,
        "Flavio should not follow Aldert"
    );

    // Look for a non existing pk
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    invalid_get_request(&format!("/v0/user/{user_id}"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_relationship() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = get_request(&format!("/v0/user/{user_id}/relationship/{viewer_id}")).await?;

    assert!(res["following"].is_boolean());
    assert!(res["followed_by"].is_boolean());

    // Test non-existing relationship
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    let viewer_id = "q5ef4o3jqxnpadzuk1h3qgqi1kefwf6zs7yuofmr6ynymt593ieo";
    invalid_get_request(
        &format!("/v0/user/{user_id}/relationship/{viewer_id}"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_view_tags() -> Result<()> {
    let user_id = PUBKY_PEER;
    let viewer_id = "58jc5bujzoj35g55pqjo6ykfdu9t156j8cxkh5ubdwgsnch1qagy";
    let res = get_request(&format!("/v0/user/{user_id}?viewer_id={viewer_id}")).await?;

    assert!(res["tags"][0]["relationship"].as_bool().unwrap());
    assert!(res["tags"][1]["relationship"].as_bool().unwrap());
    assert!(!res["tags"][2]["relationship"].as_bool().unwrap());
    assert!(!res["tags"][3]["relationship"].as_bool().unwrap());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_counts() -> Result<()> {
    let user_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = get_request(&format!("/v0/user/{user_id}/counts")).await?;

    assert!(res["tagged"].is_number());
    assert_eq!(res["tagged"], 95);
    assert!(res["tags"].is_number());
    assert_eq!(res["tags"], 7);
    assert!(res["unique_tags"].is_number());
    assert_eq!(res["unique_tags"], 3);
    assert!(res["replies"].is_number());
    assert_eq!(res["replies"], 23);
    assert!(res["posts"].is_number());
    assert_eq!(res["posts"], 87);
    assert!(res["followers"].is_number());
    assert_eq!(res["followers"], 27);
    assert!(res["following"].is_number());
    assert_eq!(res["following"], 7);
    assert!(res["friends"].is_number());
    assert_eq!(res["friends"], 6);
    assert!(res["bookmarks"].is_number());
    assert_eq!(res["bookmarks"], 14);
    assert!(res["collections"].is_number());
    assert_eq!(res["collections"], 0, "user authored no collections");

    // Test non-existing user
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    invalid_get_request(&format!("/v0/user/{user_id}/counts"), StatusCode::NOT_FOUND).await?;

    Ok(())
}

/// Counts derived from the seed in `docker/test-graph/mocks/posts.cypher`.
#[tokio_shared_rt::test(shared)]
async fn test_user_counts_collections_and_bookmark_exclusion() -> Result<()> {
    let bogota = "ep441mndnsjeesenwz78r9paepm6e4kqm4ggiyy9uzpoe43eu9ny";
    let res = get_request(&format!("/v0/user/{bogota}/counts")).await?;
    assert_eq!(res["collections"], 4, "Bogota authored 4 collections");

    let cairo = "f5tcy5gtgzshipr6pag6cn9uski3s8tjare7wd3n7enmyokgjk1o";
    let res = get_request(&format!("/v0/user/{cairo}/counts")).await?;
    assert_eq!(res["collections"], 1, "Cairo authored 1 collection");

    // Eixample bookmarks two normal posts (a root and a reply) and one
    // collection; the collection is excluded from the bookmark count.
    let eixample = "8attbeo9ftu5nztqkcfw3gydksehr7jbspgfi64u4h8eo5e7dbiy";
    let res = get_request(&format!("/v0/user/{eixample}/counts")).await?;
    assert_eq!(
        res["bookmarks"], 2,
        "collection-follow excluded from bookmarks"
    );
    assert_eq!(res["collections"], 0, "Eixample authored no collections");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_get_details() -> Result<()> {
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = get_request(&format!("/v0/user/{user_id}/details")).await?;

    assert!(res["name"].is_string());
    assert!(res["bio"].is_string());
    assert!(res["id"].is_string());
    assert!(res["status"].is_string());
    assert!(res["links"].is_array());
    assert!(res["indexed_at"].is_number());

    // Test non-existing user
    let user_id = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";
    invalid_get_request(
        &format!("/v0/user/{user_id}/details"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

// ##### Social graph status #####
// Fixture (docker/test-graph/mocks/wot.cypher): D1 0.4, D2 0.2, D1B 0.1, and no
// other user carries a trust score, so the ranking is exactly three deep and
// `ceil(3 * 0.05)` puts only its top in `established`.
const WOT_D1: &str = "qjftuwjog819ki1wktuy5tndebce36bmxxwtjjm3z1fr97jk9yuo";
const WOT_D2: &str = "smf4xrqfhx7stnufkjzhbjyu3rbgb3gga64srqmzcyyoyzefse9y";
const WOT_D1B: &str = "t5ixbtatg4tq5q5ixg16qqrg1bmem75ksg6cweuftuydwzw91pzy";
const UNRANKED_USER: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

/// Returns the field itself rather than the whole body, so a caller can tell an
/// explicit `null` from a field that was omitted: `Value` indexing answers
/// `Null` for both, and the frontend branches on the field being there.
async fn social_graph_status(user_id: &str) -> Result<serde_json::Value> {
    let res = get_request(&format!("/v0/user/{user_id}")).await?;
    Ok(res
        .get("social_graph_status")
        .cloned()
        .expect("social_graph_status must always be serialized, null included"))
}

/// Both halves live in one test on purpose: the ranking is a single global key,
/// so a separate test that dropped it could race this one and see its own null.
#[tokio_shared_rt::test(shared)]
async fn test_social_graph_status() -> Result<()> {
    assert_eq!(social_graph_status(WOT_D1).await?, "established");
    assert_eq!(social_graph_status(WOT_D2).await?, "networked");
    assert_eq!(social_graph_status(WOT_D1B).await?, "networked");
    // Carries no trust at all, so it is absent from a ranking that exists.
    assert_eq!(social_graph_status(UNRANKED_USER).await?, "new");

    // With no ranking at all the field must be null, never "new". Production
    // ships with an empty seed set, so a non-optional field would hang a NEW
    // badge on every profile in the app.
    TestServiceServer::get_test_server().await;
    let mut redis_conn = get_redis_conn().await?;
    let key = format!("Sorted:{}", USER_SOCIAL_GRAPH_KEY_PARTS.join(":"));
    let _: () = redis_conn.del(&key).await?;

    // Probed without `?` so an error between the delete and the rebuild cannot
    // strand the ranking for every other test.
    let probe = async {
        let unavailable = social_graph_status(WOT_D1).await?;
        // The batch read has to return one slot per requested id even with no
        // ranking, or the positional zip in `UserView::get_by_ids` truncates and
        // every user stream comes back empty.
        let stream = get_request("/v0/stream/users?source=most_followed&limit=5").await?;
        anyhow::Ok((unavailable, stream.as_array().map(Vec::len).unwrap_or_default()))
    }
    .await;

    SocialGraphStatus::reindex().await?;
    let (unavailable, streamed) = probe?;

    // Present and null, never omitted: the frontend branches on the field
    // existing, and `is_null` alone would also pass for a missing key.
    assert!(
        unavailable.is_null(),
        "expected null without a ranking, got {unavailable}"
    );
    assert_eq!(streamed, 5, "a missing ranking must not empty user streams");
    assert_eq!(social_graph_status(WOT_D1).await?, "established");

    Ok(())
}

/// The path production is on today: with no seed set nothing carries trust, so
/// the rebuild has nothing to write and must drop the key. Leaving a stale set
/// behind would serve an old ranking forever.
#[tokio_shared_rt::test(shared)]
async fn test_replacing_a_sorted_set_with_nothing_drops_it() -> Result<()> {
    TestServiceServer::get_test_server().await;
    let key_parts = ["Test", "SocialGraphReplace"];
    let mut redis_conn = get_redis_conn().await?;

    SocialGraphStatus::replace_index_sorted_set(&key_parts, &[(1.0, "someone")], None, None).await?;
    let populated: bool = redis_conn.exists("Sorted:Test:SocialGraphReplace").await?;

    SocialGraphStatus::replace_index_sorted_set(&key_parts, &[], None, None).await?;
    let after_empty: bool = redis_conn.exists("Sorted:Test:SocialGraphReplace").await?;

    assert!(populated, "a non-empty replace should create the key");
    assert!(!after_empty, "an empty replace should drop the key");
    Ok(())
}
