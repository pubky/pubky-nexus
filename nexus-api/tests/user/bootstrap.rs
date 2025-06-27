//use std::collections::HashSet;

use std::collections::HashSet;

use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_common::models::bootstrap::Bootstrap;

#[tokio_shared_rt::test(shared)]
async fn test_bootstrap_user() -> Result<()> {
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";

    let body = get_request(&format!("/v0/bootstrap/{user_id}")).await?;
    let user_bootstrap_respose: Bootstrap = serde_json::from_value(body).unwrap();

    // Assert the lists
    assert_eq!(user_bootstrap_respose.list.stream.len(), 20);
    assert_eq!(user_bootstrap_respose.list.influencers.len(), 3);
    assert_eq!(user_bootstrap_respose.list.recommended.len(), 5);

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
async fn test_bootstrap_user_does_not_exist() -> Result<()> {
    let endpoint = format!(
        "/v0/bootstrat/{}",
        "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhi"
    );
    invalid_get_request(&endpoint, StatusCode::NOT_FOUND).await?;
    Ok(())
}
