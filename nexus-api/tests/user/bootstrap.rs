//use std::collections::HashSet;

use crate::utils::get_request;
use anyhow::Result;
use nexus_common::models::bootstrap::Bootstrap;

#[tokio_shared_rt::test(shared)]
async fn test_im_alive_full() -> Result<()> {
    let user_id = "zdbg13k5gh4tfz9qz11quohrxetgqxs7awandu8h57147xddcuhy";

    let body = get_request(&format!("/v0/user/{}/im_alive", user_id)).await?;
    let _im_alive_respose: Bootstrap = serde_json::from_value(body).unwrap();

    // // Assert the lists
    // assert_eq!(im_alive_respose.list.stream.len(), 20);
    // assert_eq!(im_alive_respose.list.active_users.len(), 3);
    // assert_eq!(im_alive_respose.list.suggestions.len(), 5);

    // let user_ids: HashSet<String> = im_alive_respose
    //     .users
    //     .iter()
    //     .map(|user_view| user_view.details.id.to_string())
    //     .collect();

    // // Assert all users are included in the users list
    // for post in im_alive_respose.posts {
    //     let author_id = post.details.author;
    //     assert!(
    //         user_ids.contains(&author_id),
    //         "user_ids is missing author `{}`",
    //         author_id
    //     );
    //     post.tags
    //         .iter()
    //         .flat_map(|tags| tags.taggers.iter())
    //         .for_each(|tagger| {
    //             assert!(
    //                 user_ids.contains(tagger),
    //                 "user_ids is missing tagger `{}`",
    //                 tagger
    //             );
    //         });
    // }
    Ok(())
}
