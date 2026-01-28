use std::collections::HashSet;

use crate::utils::get_request;
use crate::utils::server::TestServiceServer;
use anyhow::Result;
use nexus_common::models::bootstrap::{Bootstrap, ViewType};

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_user() -> Result<()> {
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";

    let body = get_request(&format!("/v0/bootstrap/{user_id}")).await?;
    let user_bootstrap_respose: Bootstrap = serde_json::from_value(body).unwrap();

    // Assert the user is indexed
    assert!(user_bootstrap_respose.indexed, "User should be indexed");

    // Assert the lists
    assert_eq!(user_bootstrap_respose.ids.stream.len(), 20);
    assert_eq!(user_bootstrap_respose.ids.influencers.len(), 3);
    assert_eq!(user_bootstrap_respose.ids.recommended.len(), 5);
    assert!(user_bootstrap_respose.ids.hot_tags.len() <= 40);
    assert_eq!(user_bootstrap_respose.ids.muted.len(), 1);

    let user_ids: HashSet<String> = user_bootstrap_respose
        .users
        .0
        .iter()
        .map(|user_view| user_view.details.id.to_string())
        .collect();

    // Assert all users are included in the users list
    for post in user_bootstrap_respose.posts.0 {
        let author_id = post.details.author;
        assert!(
            user_ids.contains(&author_id),
            "user_ids is missing author `{author_id}`"
        );
        post.tags
            .iter()
            .flat_map(|tags| tags.taggers.iter())
            .for_each(|tagger| {
                assert!(
                    user_ids.contains(tagger),
                    "user_ids is missing tagger `{tagger}`"
                );
            });
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_user_not_indexed() -> Result<()> {
    // Use a random pubky ID that doesn't exist in the system
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhi";

    let body = get_request(&format!("/v0/bootstrap/{user_id}")).await?;
    let user_bootstrap_response: Bootstrap = serde_json::from_value(body).unwrap();

    // Assert the user is not indexed
    assert!(
        !user_bootstrap_response.indexed,
        "User should not be indexed"
    );

    // Even though user is not indexed, we should still get some data
    // (influencers, hot_tags, etc.) but no recommended users
    assert_eq!(
        user_bootstrap_response.ids.recommended.len(),
        0,
        "Non-indexed users should not have recommended users"
    );
    // Influencers and hot_tags should still be populated (global data)
    assert!(user_bootstrap_response.ids.influencers.len() <= 3);
    assert!(user_bootstrap_response.ids.hot_tags.len() <= 40);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_respects_user_limit() -> Result<()> {
    // Ensure DB connections are initialized
    let _ = TestServiceServer::get_test_server().await;

    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";
    let max_user_views = 10;

    let bootstrap = Bootstrap::get_by_id_with_limit(user_id, ViewType::Full, max_user_views)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    assert_eq!(
        bootstrap.users.0.len(),
        max_user_views,
        "Bootstrap should limit user views to {max_user_views}"
    );
    Ok(())
}
