use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::fetch_row_from_graph;
use nexus_common::db::graph::Query;
use nexus_common::db::kv::RedisOps;
use nexus_common::models::user::{user_hs_cursor_key, UserDetails};
use pubky::{Keypair, PublicKey};
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyId};

pub async fn create_external_test_homeserver(test: &mut WatcherTest) -> Result<PublicKey> {
    let homeserver_id = test.testnet.create_random_homeserver().await?.public_key();
    Ok(homeserver_id)
}

/// Asserts a user was properly ingested:
/// - graph node exists
/// - `USER_HS_CURSOR` Redis key path points to the HS
/// - `HOSTED_BY` graph edge (with `resolved_at` stamped) binds user to HS
pub async fn assert_user_ingested(user_id: &str, hs_pk: &PublicKey) {
    let hs_id = hs_pk.to_z32();

    let user = UserDetails::get_by_id(user_id)
        .await
        .expect("UserDetails::get_by_id failed");
    assert!(user.is_some(), "User {user_id} should be ingested");

    let key = user_hs_cursor_key(user_id);
    let cursor = UserDetails::check_sorted_set_member(None, &key, &[&hs_id])
        .await
        .expect("check_sorted_set_member failed");
    assert!(
        cursor.is_some(),
        "USER_HS_CURSOR should map {user_id} -> {hs_id}"
    );

    let query = Query::new(
        "assert_user_hosted_by",
        "MATCH (u:User {id: $user_id})-[r:HOSTED_BY]->(hs:Homeserver)
         RETURN hs.id AS homeserver_id, r.resolved_at AS resolved_at",
    )
    .param("user_id", user_id.to_string());
    let row = fetch_row_from_graph(query)
        .await
        .expect("fetch_row_from_graph failed")
        .unwrap_or_else(|| panic!("User {user_id} should have a HOSTED_BY edge"));
    let bound_hs_id: String = row.get("homeserver_id").expect("missing homeserver_id");
    assert_eq!(
        bound_hs_id, hs_id,
        "HOSTED_BY should point {user_id} -> {hs_id}"
    );
    let resolved_at: i64 = row.get("resolved_at").expect("resolved_at should be set");
    assert!(
        resolved_at > 0,
        "resolved_at should be positive, got {resolved_at}"
    );
}

/// Registers a fresh user on `hs_pk`, which the test doesn't index.
pub async fn user_on(test: &WatcherTest, hs_pk: &PublicKey) -> Result<(Keypair, PubkyId)> {
    let user_kp = Keypair::random();
    test.register_user_in_hs(&user_kp, hs_pk).await?;
    let user_id = PubkyId::from(user_kp.public_key());
    Ok((user_kp, user_id))
}

/// A short post with this text, replying to `parent` if given.
pub fn short_post(content: &str, parent: Option<String>) -> PubkyAppPost {
    PubkyAppPost {
        content: content.to_string(),
        kind: PubkyAppPostKind::Short,
        parent,
        embed: None,
        attachments: None,
        lock: None,
    }
}
