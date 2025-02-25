use super::utils::{
    check_member_global_timeline_user_post, check_member_user_post_timeline, find_post_counts,
};
use crate::watcher::posts::utils::find_post_details;
use crate::watcher::users::utils::{check_member_user_pioneer, find_user_counts};
use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use pubky_nexus::models::post::{PostCounts, PostDetails};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_post_event() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_event".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEvent:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;

    // GRAPH_OP: Assert if the event writes the graph
    // Cannot use PostDetails::get_from_graph because it indexes also,
    // Sorted:Posts:Global:Timeline and Sorted:Posts:User. That operation has to be executed in the ingest_user
    let post_details = find_post_details(&user_id, &post_id).await.unwrap();

    assert_eq!(post_details.id, post_id);
    assert_eq!(post_details.content, post.content);
    assert_eq!(
        post_details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}")
    );
    assert!(post_details.indexed_at > 0);

    // CACHE_OP: Check if the event writes in the graph

    //User:Details:user_id:post_id
    let post_detail_cache: PostDetails = PostDetails::get_from_index(&user_id, &post_id)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");

    assert_eq!(post_details.id, post_detail_cache.id);
    assert_eq!(post_details.content, post_detail_cache.content);
    assert_eq!(post_details.uri, post_detail_cache.uri);
    assert_eq!(post_details.indexed_at, post_detail_cache.indexed_at);

    // User:Counts:user_id:post_id
    let post_counts: PostCounts = find_post_counts(&user_id, &post_id).await;
    assert_eq!(post_counts.reposts, 0);
    assert_eq!(post_counts.replies, 0);
    assert_eq!(post_counts.tags, 0);

    // Sorted:Post:Global:Timeline
    let global_timeline = check_member_global_timeline_user_post(&user_id, &post_id)
        .await
        .unwrap();
    assert!(global_timeline.is_some());
    assert_eq!(global_timeline.unwrap(), post_details.indexed_at as isize);

    // Sorted:Posts:User:user_id
    let post_timeline = check_member_user_post_timeline(&user_id, &post_id)
        .await
        .unwrap();
    assert!(post_timeline.is_some());
    assert_eq!(post_timeline.unwrap(), post_details.indexed_at as isize);

    // Has pioneer score. Sorted:Users:Pioneers
    let pioneer_score = check_member_user_pioneer(&user_id).await.unwrap();
    assert!(pioneer_score.is_some());
    assert_eq!(pioneer_score.unwrap(), 0);

    let exist_count = find_user_counts(&user_id).await;
    assert_eq!(exist_count.posts, 1);

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &post_id).await?;

    // // TODO: Impl DEL post. Assert the new post does not exist in Nexus
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    Ok(())
}
