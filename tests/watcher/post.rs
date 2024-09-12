use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::{models::{
    post::{PostStream, PostView, POST_PER_USER_KEY_PARTS, POST_TIMELINE_KEY_PARTS},
    pubky_app::{PostKind, PubkyAppPost, PubkyAppUser}, user::{UserCounts, UserStream, USER_PIONEERS_KEY_PARTS},
}, RedisOps};

#[tokio::test]
async fn test_homeserver_post() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test Poster".to_string(),
        status: None,
    };

    let user_id = test.create_user(&keypair, &user).await?;

    let post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let post_id = test.create_post(&user_id, &post).await?;

    // Assert the new post can be served from Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(result_post.details.id, post_id);
    assert_eq!(result_post.details.content, post.content);
    assert_eq!(
        result_post.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}")
    );
    assert_eq!(result_post.counts.reposts, 0);
    assert!(result_post.details.indexed_at > 0);
    assert_eq!(result_post.counts.tags, 0);
    assert_eq!(result_post.counts.replies, 0);

    // Check the cache state
    // TODO: all that checks are the same also for reply and repost. Maybe add in a watcher/post/utils.rs
    let post_key: &[&str] = &[&user_id, &post_id];
    println!("{:?}", post_key);
    let global_timeline = PostStream::check_sorted_set_member(&POST_TIMELINE_KEY_PARTS, post_key).await.unwrap();
    assert_eq!(global_timeline.is_some(), true);
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[&user_id]].concat();
    let post_timeline = PostStream::check_sorted_set_member(&post_stream_key_parts, &[&post_id]).await.unwrap();
    assert_eq!(post_timeline.is_some(), true);
    // Both timestamp has to be the same
    assert_eq!(global_timeline.unwrap(), post_timeline.unwrap());
    // Has pioneer score
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[&user_id])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pioneer_score, 0);

    let exist_count = UserCounts::try_from_index_json(&[&user_id]).await.unwrap()
        .expect("User count not found");

    // Post + Reply
    assert_eq!(exist_count.posts, 2);

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
