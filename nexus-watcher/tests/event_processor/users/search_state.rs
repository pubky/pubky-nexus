use super::utils::find_user_details;
use crate::event_processor::posts::utils::{short_post, test_user};
use crate::event_processor::tags::utils::check_member_user_tag_taggers;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::{UserDetails, UserSearch};
use nexus_watcher::events::handlers::user;
use pubky::Keypair;
use pubky_app_specs::{user_uri_builder, PubkyAppTag, PubkyAppUser, PubkyId};

async fn found_by_name(name: &str, user_id: &str) -> Result<bool> {
    Ok(UserSearch::get_by_name(name, None, None)
        .await?
        .is_some_and(|found| found.0.iter().any(|id| id == user_id)))
}

async fn found_by_id(user_id: &str) -> Result<bool> {
    Ok(UserSearch::get_by_id(user_id, None, None)
        .await?
        .is_some_and(|found| !found.0.is_empty()))
}

/// Writes a profile to the graph only, as a `sync_put` that failed right after
/// its graph write leaves it.
async fn put_profile_to_graph_only(user_id: &str, user: &PubkyAppUser) -> Result<()> {
    let pubky_id = PubkyId::try_from(user_id).map_err(anyhow::Error::msg)?;
    UserDetails::from_homeserver(user.clone(), &pubky_id)
        .put_profile_to_graph(&user_uri_builder(user_id.to_string()))
        .await?;
    Ok(())
}

/// Three renames in a row, with no read in between: only the last name is
/// searchable, and the hash records it.
#[tokio_shared_rt::test(shared)]
async fn test_rename_chain_keeps_only_the_last_name() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let names = [
        "Watcher:SearchState:ChainA",
        "Watcher:SearchState:ChainB",
        "Watcher:SearchState:ChainC",
    ];

    let user_id = test
        .create_user(&user_kp, &test_user(names[0], "chain"))
        .await?;
    for name in &names[1..] {
        test.create_profile(&user_kp, &test_user(*name, "chain"))
            .await?;
    }

    assert!(
        found_by_name(names[2], &user_id).await?,
        "the last name is indexed"
    );
    for stale in &names[..2] {
        assert!(
            !found_by_name(stale, &user_id).await?,
            "{stale} must be gone"
        );
    }
    assert_eq!(
        UserSearch::indexed_names(&[&user_id]).await?,
        vec![Some(names[2].to_lowercase())]
    );

    // A hard delete after the renames drops the last indexed name and the id
    test.cleanup_user(&user_kp).await?;
    assert!(!found_by_name(names[2], &user_id).await?);
    assert!(!found_by_id(&user_id).await?);
    assert_eq!(UserSearch::indexed_names(&[&user_id]).await?, vec![None]);
    Ok(())
}

/// A profile event that failed after its graph write leaves the graph on the
/// new name and the search on the old one. The retry, and any later event,
/// still finds the old member through the hash and drops it.
#[tokio_shared_rt::test(shared)]
async fn test_search_recovers_after_a_failed_sync_following_the_graph_write() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let user_kp = Keypair::random();
    let user_id = test
        .create_user(
            &user_kp,
            &test_user("Watcher:SearchState:BeforeFailure", "failure"),
        )
        .await?;

    let renamed = test_user("Watcher:SearchState:AfterFailure", "failure");
    put_profile_to_graph_only(&user_id, &renamed).await?;
    assert_eq!(
        find_user_details(&user_id).await?.name,
        renamed.name,
        "the graph holds the new name"
    );
    assert!(
        found_by_name("Watcher:SearchState:BeforeFailure", &user_id).await?,
        "the search still holds the old name"
    );

    // The retry of the same event
    let pubky_id = PubkyId::try_from(user_id.as_str()).map_err(anyhow::Error::msg)?;
    user::sync_put(renamed.clone(), user_uri_builder(user_id.clone()), pubky_id).await?;

    assert!(!found_by_name("Watcher:SearchState:BeforeFailure", &user_id).await?);
    assert!(found_by_name("Watcher:SearchState:AfterFailure", &user_id).await?);

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// A tombstone that failed after its graph write is not yet in the deleted
/// set, so a tag event racing it can re-add the user to the users-by-tag
/// index. The retry marks the user deleted and evicts them, later tag events
/// cannot bring them back, and a recreated profile restores them.
#[tokio_shared_rt::test(shared)]
async fn test_tag_event_racing_a_tombstone_is_evicted_by_the_retry() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let tagged_kp = Keypair::random();
    let tagged_id = test
        .create_user(&tagged_kp, &test_user("Watcher:SearchState:Tagged", "race"))
        .await?;
    let tagger_kp = Keypair::random();
    test.create_user(&tagger_kp, &test_user("Watcher:SearchState:Tagger", "race"))
        .await?;
    // An edge keeps the node, so the delete tombstones
    test.create_post(&tagged_kp, &short_post("Watcher:SearchState:Post"))
        .await?;

    let label = "racetag";
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag.clone()).await?;
    assert_eq!(
        check_member_user_tag_taggers(&tagged_id, label).await?,
        Some(1)
    );

    // The delete fails right after the graph tombstone
    let tagged_pubky_id = PubkyId::try_from(tagged_id.as_str()).map_err(anyhow::Error::msg)?;
    UserDetails::tombstone(&tagged_pubky_id)
        .put_to_graph()
        .await?;

    // A tag event races it: the deleted set does not know yet
    test.put(&tagger_kp, &tag_path, tag.clone()).await?;
    assert_eq!(
        check_member_user_tag_taggers(&tagged_id, label).await?,
        Some(1),
        "before the retry the user is still in the users-by-tag index"
    );

    // The retry of the delete
    user::del(tagged_pubky_id).await?;
    assert!(
        check_member_user_tag_taggers(&tagged_id, label)
            .await?
            .is_none(),
        "the retry evicts the user"
    );
    assert!(!found_by_name("Watcher:SearchState:Tagged", &tagged_id).await?);
    assert!(!found_by_id(&tagged_id).await?);

    test.put(&tagger_kp, &tag_path, tag).await?;
    assert!(
        check_member_user_tag_taggers(&tagged_id, label)
            .await?
            .is_none(),
        "a tag event after the tombstone cannot re-add the user"
    );

    test.create_profile(
        &tagged_kp,
        &test_user("Watcher:SearchState:TaggedBack", "race"),
    )
    .await?;
    assert_eq!(
        check_member_user_tag_taggers(&tagged_id, label).await?,
        Some(1),
        "a recreated profile restores the retained tag"
    );
    assert!(found_by_name("Watcher:SearchState:TaggedBack", &tagged_id).await?);

    test.cleanup_user(&tagger_kp).await?;
    Ok(())
}
