use super::utils::{check_member_user_tag_taggers, find_user_tag};
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{fetch_all_rows_from_graph, queries, RedisOps};
use nexus_common::models::tag::user::TagUser;
use nexus_common::models::user::UsersByTagSearch;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

/// The backfill enumerates pairs from a graph snapshot but derives every
/// score from the live taggers set, so a delete landing after the snapshot
/// was taken must not be resurrected by the later backfill write.
#[tokio_shared_rt::test(shared)]
async fn test_users_by_tags_backfill_does_not_resurrect_deleted_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_user = PubkyAppUser {
        bio: Some("test_users_by_tags_backfill".to_string()),
        image: None,
        links: None,
        name: "Watcher:BackfillRace:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_users_by_tags_backfill".to_string()),
        image: None,
        links: None,
        name: "Watcher:BackfillRace:TaggerUser".to_string(),
        status: None,
    };
    let _tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    let label = "ghosttag";
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Snapshot the pair enumeration exactly as a live backfill would
    let rows = fetch_all_rows_from_graph(queries::get::get_user_tag_pairs()).await?;
    let mut snapshot_pairs = Vec::new();
    for row in rows {
        let user_id: String = row.get("user_id")?;
        let pair_label: String = row.get("label")?;
        if user_id == tagged_user_id {
            snapshot_pairs.push((user_id, pair_label));
        }
    }
    assert!(
        snapshot_pairs.contains(&(tagged_user_id.clone(), label.to_string())),
        "The snapshot must contain the pair before the delete"
    );

    // The delete lands while the backfill still holds the stale snapshot
    test.del(&tagger_kp, &tag_path).await?;

    // Replay the backfill writes from the stale snapshot
    for (user_id, pair_label) in &snapshot_pairs {
        UsersByTagSearch::sync_index_score(user_id, pair_label).await?;
    }

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert!(
        score.is_none(),
        "A stale backfill snapshot must not resurrect a deleted tag"
    );

    test.cleanup_user(&tagged_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

/// Deleting a user with edges tombstones them, and tombstoned users must
/// leave the users-by-tag index and stay out even when tag events for them
/// keep arriving.
#[tokio_shared_rt::test(shared)]
async fn test_users_by_tags_excludes_deleted_users() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_user = PubkyAppUser {
        bio: Some("test_users_by_tags_excludes_deleted".to_string()),
        image: None,
        links: None,
        name: "Watcher:DeletedUser:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_users_by_tags_excludes_deleted".to_string()),
        image: None,
        links: None,
        name: "Watcher:DeletedUser:TaggerUser".to_string(),
        status: None,
    };
    let _tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    let label = "deadtag";
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert_eq!(score, Some(1));

    // The tagged user has a TAGGED edge, so deletion tombstones them and
    // must evict them from the index
    test.cleanup_user(&tagged_kp).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert!(
        score.is_none(),
        "A tombstoned user cannot stay in the users-by-tag index"
    );

    // A tag event arriving after the tombstone cannot resurrect them: the
    // derive re-checks the tombstone atomically
    let late_tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    test.put(&tagger_kp, &tag_path, late_tag).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert!(
        score.is_none(),
        "A tag event after the tombstone cannot re-add the user"
    );

    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

/// A tombstoned user who recreates their profile gets their retained tag
/// entries back: eviction and restoration are the same label sync, decided
/// by the tombstone state at derive time.
#[tokio_shared_rt::test(shared)]
async fn test_users_by_tags_restores_recreated_users() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let tagged_kp = Keypair::random();
    let tagged_user = PubkyAppUser {
        bio: Some("test_users_by_tags_restores_recreated".to_string()),
        image: None,
        links: None,
        name: "Watcher:RestoredUser:TaggedUser".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_kp, &tagged_user).await?;

    let tagger_kp = Keypair::random();
    let tagger_user = PubkyAppUser {
        bio: Some("test_users_by_tags_restores_recreated".to_string()),
        image: None,
        links: None,
        name: "Watcher:RestoredUser:TaggerUser".to_string(),
        status: None,
    };
    let _tagger_user_id = test.create_user(&tagger_kp, &tagger_user).await?;

    let label = "phoenixtag";
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert_eq!(score, Some(1));

    // Tombstone the tagged user (the TAGGED edge keeps the node) and verify
    // the eviction
    test.cleanup_user(&tagged_kp).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert!(score.is_none(), "Tombstoned user must be evicted");

    // Recreate the profile without touching the tag: the retained label must
    // come back with its taggers count
    let restored_user = PubkyAppUser {
        bio: Some("test_users_by_tags_restores_recreated".to_string()),
        image: None,
        links: None,
        name: "Watcher:RestoredUser:TaggedUserBack".to_string(),
        status: None,
    };
    test.create_profile(&tagged_kp, &restored_user).await?;

    let score = check_member_user_tag_taggers(&tagged_user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert_eq!(
        score,
        Some(1),
        "A recreated profile must restore its retained tag entries"
    );

    // The graph agrees: the retained edge still carries one tagger
    let graph_tag = find_user_tag(&tagged_user_id, label)
        .await
        .unwrap()
        .expect("Retained tag must exist in the graph");
    assert_eq!(graph_tag.taggers_count, 1);

    test.cleanup_user(&tagged_kp).await?;
    test.cleanup_user(&tagger_kp).await?;

    Ok(())
}

/// Semantics of the atomic derive: the score equals the taggers set
/// cardinality, and the member disappears when the set empties.
#[tokio_shared_rt::test(shared)]
async fn test_sync_index_score_derives_from_taggers_set() -> Result<()> {
    // Setup only initializes the shared stack connectors
    let _test = WatcherTest::setup(None).await?;

    let user_id = "syncscorefixtureuser";
    let label = "synccheck";
    let taggers = ["syncscoretaggerone", "syncscoretaggertwo"];

    TagUser::put_index_set(&[user_id, label], &taggers, None, None).await?;
    UsersByTagSearch::sync_index_score(user_id, label).await?;
    let score = check_member_user_tag_taggers(user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert_eq!(
        score,
        Some(2),
        "Score must equal the taggers set cardinality"
    );

    TagUser(taggers.iter().map(|t| t.to_string()).collect())
        .remove_from_index_set(&[user_id, label])
        .await?;
    UsersByTagSearch::sync_index_score(user_id, label).await?;
    let score = check_member_user_tag_taggers(user_id, label)
        .await
        .expect("Failed to check the users-by-tag score");
    assert!(
        score.is_none(),
        "Member must be removed when the taggers set empties"
    );

    Ok(())
}
