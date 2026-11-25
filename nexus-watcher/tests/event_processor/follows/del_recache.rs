//! A read must not index a follow again: a read during a follow delete would
//! bring the unfollow back into Redis (#1244).

use super::utils::find_follow_relationship;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::RedisOps,
    models::follow::{Followers, Following, UserFollows},
};
use pubky::Keypair;
use pubky_social_specs::legacy_v0::PubkyAppUser;

fn test_user(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        bio: Some("test_follow_read_during_delete_does_not_recache".to_string()),
        image: None,
        links: None,
        name: name.to_string(),
        status: None,
    }
}

/// A read of both follow lists that misses Redis during a follow delete
/// (Redis cleared, graph edge not yet deleted) gets the follow from the graph
/// but does not index it again, so the index is empty once the delete finishes.
#[tokio_shared_rt::test(shared)]
async fn test_follow_read_during_delete_does_not_recache() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let follower_kp = Keypair::random();
    let follower_id = test
        .create_user(
            &follower_kp,
            &test_user("Watcher:FollowDelRecache:Follower"),
        )
        .await?;
    let followee_kp = Keypair::random();
    let followee_id = test
        .create_user(
            &followee_kp,
            &test_user("Watcher:FollowDelRecache:Followee"),
        )
        .await?;
    test.create_follow(&follower_kp, &followee_id).await?;

    // The delete window: sync_del's index cleanup, graph edge still there.
    Followers(vec![follower_id.clone()])
        .del_from_index(&followee_id)
        .await?;
    Following(vec![followee_id.clone()])
        .del_from_index(&follower_id)
        .await?;
    assert!(find_follow_relationship(&follower_id, &followee_id).await?);

    // A read in the window still answers from the graph...
    let following = Following::get_by_id(&follower_id, None, Some(10)).await?;
    assert!(following.is_some_and(|f| f.0.contains(&followee_id)));
    let followers = Followers::get_by_id(&followee_id, None, Some(50)).await?;
    assert!(followers.is_some_and(|f| f.0.contains(&follower_id)));

    // ...and sync_del finishes with its last step, the graph delete...
    Followers::del_from_graph(&follower_id, &followee_id).await?;
    assert!(!find_follow_relationship(&follower_id, &followee_id).await?);

    // ...and the follow is not left in Redis.
    let (_, cached) = Following::check_set_member(&[&follower_id], &followee_id).await?;
    assert!(!cached, "Following:{follower_id} still lists {followee_id}");
    let (_, cached) = Followers::check_set_member(&[&followee_id], &follower_id).await?;
    assert!(!cached, "Followers:{followee_id} still lists {follower_id}");

    test.cleanup_user(&follower_kp).await?;
    test.cleanup_user(&followee_kp).await?;
    Ok(())
}
