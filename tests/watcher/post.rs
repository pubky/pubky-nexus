use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::models::{
    post::PostView,
    pubky_app::{PostKind, PubkyAppPost, PubkyAppUser},
};

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

    println!("New post served: {:?}", result_post.details);
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
