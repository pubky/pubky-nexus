use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::user::UserView;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

#[tokio_shared_rt::test(shared)]
async fn test_user_view_batch_retrieval() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Create 5 users
    let mut user_ids = Vec::with_capacity(5);
    let mut user_kps = Vec::with_capacity(5);

    for i in 0..5 {
        let user_kp = Keypair::random();

        let user = PubkyAppUser {
            bio: Some(format!("test_batch_retrieval_user_{}", i)),
            image: None,
            links: None,
            name: format!("Watcher:BatchRetrieval:User{}", i),
            status: Some(format!("User status {}", i)),
        };

        let user_id = test.create_user(&user_kp, &user).await?;
        user_ids.push(user_id);
        user_kps.push(user_kp);
    }

    // Step 2: Fetch user views individually using get_by_id
    let mut individual_views = Vec::with_capacity(5);
    for user_id in &user_ids {
        let user_view = UserView::get_by_id(user_id, None, None)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        individual_views.push(user_view);
    }

    // Step 3: Fetch user views in batch using get_by_ids
    let batch_views = UserView::get_by_ids(&user_ids, None, None)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;

    // Step 4: Assert that both methods return the same results
    assert_eq!(
        individual_views.len(),
        batch_views.len(),
        "Both methods should return the same number of user views"
    );

    for (i, (individual_view, batch_view)) in
        individual_views.iter().zip(batch_views.iter()).enumerate()
    {
        match (individual_view, batch_view) {
            (Some(ind_view), Some(batch_view)) => {
                // Both should have the same user details
                assert_eq!(
                    ind_view.details.name, batch_view.details.name,
                    "User {i}: Names should match"
                );
                assert_eq!(
                    ind_view.details.bio, batch_view.details.bio,
                    "User {i}: Bios should match"
                );
                assert_eq!(
                    ind_view.details.status, batch_view.details.status,
                    "User {i}: Statuses should match"
                );

                // Both should have the same user counts
                assert_eq!(
                    ind_view.counts.posts, batch_view.counts.posts,
                    "User {i}: Post counts should match"
                );
                assert_eq!(
                    ind_view.counts.followers, batch_view.counts.followers,
                    "User {i}: Follower counts should match"
                );
                assert_eq!(
                    ind_view.counts.following, batch_view.counts.following,
                    "User {i}: Following counts should match"
                );
            }
            (None, None) => {
                // Both returned None, which is consistent
                continue;
            }
            _ => panic!("User {i}: Inconsistent results between individual and batch retrieval"),
        }
    }

    // Step 5: Cleanup
    for user_kp in user_kps {
        test.cleanup_user(&user_kp).await?;
    }

    Ok(())
}
