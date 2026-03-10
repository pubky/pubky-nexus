use std::time::Duration;

use super::utils::find_post_details;
use super::utils::{
    check_member_global_timeline_user_post, check_member_user_post_timeline, find_post_counts,
};
use crate::event_processor::users::utils::{check_member_user_influencer, find_user_counts};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::models::event::Event;
use nexus_common::models::homeserver::Homeserver;
use nexus_common::models::post::{PostCounts, PostDetails};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser, PubkyId};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_put_post_event() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_event".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostEvent:User".to_string(),
        status: None,
    };

    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostEvent:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (_, events_in_redis_before) = Event::get_events_from_redis(None, 1000).await.unwrap();

    let (post_id, post_path) = test.create_post(&user_kp, &post).await?;

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
    let (_, events_in_redis_after) = Event::get_events_from_redis(None, 1000).await.unwrap();
    assert!(events_in_redis_after > events_in_redis_before);

    //User:Details:user_id:post_id
    let post_detail_cache: PostDetails = PostDetails::get_from_index(&user_id, &post_id)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");

    assert_eq!(post_details.id, post_detail_cache.id);
    assert_eq!(post_details.content, post_detail_cache.content);
    assert_eq!(post_details.uri, post_detail_cache.uri);
    assert_eq!(post_details.indexed_at, post_detail_cache.indexed_at);

    reindex_and_ensure_cache_and_graph_unchanged(
        &mut test,
        &post_details,
        &post_detail_cache,
        &user_id,
        &post_id,
    )
    .await?;

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

    // Has influencer score. Sorted:Users:Influencers
    let influencer_score = check_member_user_influencer(&user_id).await.unwrap();
    assert!(influencer_score.is_some());
    assert_eq!(influencer_score.unwrap(), 0);

    let exist_count = find_user_counts(&user_id).await;
    assert_eq!(exist_count.posts, 1);

    // Cleanup
    test.cleanup_user(&user_kp).await?;
    test.cleanup_post(&user_kp, &post_path).await?;

    // // TODO: Impl DEL post. Assert the new post does not exist in Nexus
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    Ok(())
}

async fn reindex_and_ensure_cache_and_graph_unchanged(
    test: &mut WatcherTest,
    post_details_from_graph: &PostDetails,
    post_detail_from_cache: &PostDetails,
    user_id: &str,
    post_id: &str,
) -> Result<()> {
    // Wait for a few ms, so that re-indexing determines a different indexed_at (epoch timestamp in ms)
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Reset the cursor, to ensure the events are re-indexed
    let homeserver = Homeserver::new(PubkyId::try_from(&test.homeserver_id).unwrap());
    homeserver.put_to_graph().await.unwrap();
    homeserver.put_to_index().await.unwrap();
    test.ensure_event_processing_complete().await?;

    // Check that nothing changed in the graph (DB)
    let post_details_2 = find_post_details(user_id, post_id).await.unwrap();
    assert_eq!(post_details_from_graph, &post_details_2);

    // Check that nothing changed in the index (cache)
    let post_detail_cache_2: PostDetails = PostDetails::get_from_index(user_id, post_id)
        .await
        .unwrap()
        .expect("The new post detail was not served from Nexus cache");
    assert_eq!(post_detail_from_cache, &post_detail_cache_2);

    Ok(())
}
