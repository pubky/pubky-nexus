use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::user::UserStream;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};

/// Minimum number of posts a user needs to be recommended
const ACTIVE_USER_POSTS: usize = 5;

async fn create_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_recommended_users".to_string()),
        image: None,
        links: None,
        name: format!("Watcher:UserRecommended:{name}"),
        status: None,
    };
    let user_id = test.create_user(&keypair, &user).await?;
    Ok((keypair, user_id))
}

async fn create_posts(test: &mut WatcherTest, keypair: &Keypair, count: usize) -> Result<()> {
    for i in 0..count {
        let post = PubkyAppPost {
            content: format!("Recommended user test post {i}"),
            kind: PubkyAppPostKind::Short,
            parent: None,
            embed: None,
            attachments: None,
            lock: None,
        };
        test.create_post(keypair, &post).await?;
    }
    Ok(())
}

/// The recommended ids in a stable order, as the query has no ORDER BY
async fn get_sorted_recommended_ids(user_id: &str) -> Result<Option<Vec<String>>> {
    let recommended_ids = UserStream::get_recommended_ids(user_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get recommended ids: {e}"))?;
    Ok(recommended_ids.map(|mut ids| {
        ids.sort();
        ids
    }))
}

/// Scenario:
/// - Alice follows Bob and Carol, Bob follows Carol
/// - Carol is active and reachable at depth 2, but Alice already follows her
/// - ensure Carol is not recommended to Alice
#[tokio_shared_rt::test(shared)]
async fn test_recommended_excludes_directly_followed_user() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&alice_kp, &carol_id).await?;
    test.create_follow(&bob_kp, &carol_id).await?;

    create_posts(&mut test, &carol_kp, ACTIVE_USER_POSTS).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    assert_eq!(
        recommended_ids, None,
        "A directly followed user should not be recommended, even if reachable at depth 2"
    );

    Ok(())
}

/// Scenario:
/// - Alice follows Bob, Bob follows Alice back and also follows Carol
/// - Alice and Carol are both active
/// - ensure only Carol is recommended to Alice, not Alice herself
#[tokio_shared_rt::test(shared)]
async fn test_recommended_excludes_origin_in_follow_cycle() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&bob_kp, &alice_id).await?;
    test.create_follow(&bob_kp, &carol_id).await?;

    // Alice has to be active too, otherwise the post threshold would hide her
    create_posts(&mut test, &alice_kp, ACTIVE_USER_POSTS).await?;
    create_posts(&mut test, &carol_kp, ACTIVE_USER_POSTS).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    assert_eq!(
        recommended_ids,
        Some(vec![carol_id]),
        "A follow cycle should not recommend the user to themselves"
    );

    Ok(())
}

/// Scenario:
/// - Alice follows Bob, Bob follows Carol and Dave
/// - Carol is one post short of the threshold, Dave is exactly on it
/// - ensure only Dave is recommended to Alice
#[tokio_shared_rt::test(shared)]
async fn test_recommended_requires_post_threshold() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;
    let (dave_kp, dave_id) = create_user(&mut test, "Dave").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&bob_kp, &carol_id).await?;
    test.create_follow(&bob_kp, &dave_id).await?;

    create_posts(&mut test, &carol_kp, ACTIVE_USER_POSTS - 1).await?;
    create_posts(&mut test, &dave_kp, ACTIVE_USER_POSTS).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    assert_eq!(
        recommended_ids,
        Some(vec![dave_id]),
        "Only the user with at least {ACTIVE_USER_POSTS} posts should be recommended"
    );

    Ok(())
}

/// Scenario:
/// - follow chain Alice -> Bob -> Carol -> Dave -> Erin
/// - Carol (depth 2), Dave (depth 3) and Erin (depth 4) are all active
/// - ensure Carol and Dave are recommended to Alice, but not Erin
#[tokio_shared_rt::test(shared)]
async fn test_recommended_depth_bounds() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;
    let (dave_kp, dave_id) = create_user(&mut test, "Dave").await?;
    let (erin_kp, erin_id) = create_user(&mut test, "Erin").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&bob_kp, &carol_id).await?;
    test.create_follow(&carol_kp, &dave_id).await?;
    test.create_follow(&dave_kp, &erin_id).await?;

    create_posts(&mut test, &carol_kp, ACTIVE_USER_POSTS).await?;
    create_posts(&mut test, &dave_kp, ACTIVE_USER_POSTS).await?;
    create_posts(&mut test, &erin_kp, ACTIVE_USER_POSTS).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    let mut expected_ids = vec![carol_id, dave_id];
    expected_ids.sort();
    assert_eq!(
        recommended_ids,
        Some(expected_ids),
        "Users at depth 2 and 3 should be recommended, the one at depth 4 should not"
    );

    Ok(())
}

/// Scenario:
/// - Alice follows Bob and Carol, both follow Dave
/// - Dave is active and reachable over two different paths
/// - ensure Dave is recommended to Alice only once
#[tokio_shared_rt::test(shared)]
async fn test_recommended_user_reached_by_two_paths_is_listed_once() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;
    let (dave_kp, dave_id) = create_user(&mut test, "Dave").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&alice_kp, &carol_id).await?;
    test.create_follow(&bob_kp, &dave_id).await?;
    test.create_follow(&carol_kp, &dave_id).await?;

    create_posts(&mut test, &dave_kp, ACTIVE_USER_POSTS).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    assert_eq!(
        recommended_ids,
        Some(vec![dave_id]),
        "A user reachable over two paths should be recommended exactly once"
    );

    Ok(())
}

/// Scenario:
/// - Alice follows Bob, Bob follows Carol and Dave
/// - Carol and Dave are both active, then Carol is deleted: she keeps her posts,
///   so she stays in the graph flagged as deleted
/// - ensure only Dave is recommended to Alice
#[tokio_shared_rt::test(shared)]
async fn test_recommended_excludes_deleted_user() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;
    let (carol_kp, carol_id) = create_user(&mut test, "Carol").await?;
    let (dave_kp, dave_id) = create_user(&mut test, "Dave").await?;

    test.create_follow(&alice_kp, &bob_id).await?;
    test.create_follow(&bob_kp, &carol_id).await?;
    test.create_follow(&bob_kp, &dave_id).await?;

    create_posts(&mut test, &carol_kp, ACTIVE_USER_POSTS).await?;
    create_posts(&mut test, &dave_kp, ACTIVE_USER_POSTS).await?;

    // Delete Carol before the first call for Alice, so the recommended cache is
    // still cold and the result comes from the graph
    test.cleanup_user(&carol_kp).await?;

    let recommended_ids = get_sorted_recommended_ids(&alice_id).await?;
    assert_eq!(
        recommended_ids,
        Some(vec![dave_id]),
        "A deleted user should not be recommended"
    );

    Ok(())
}
