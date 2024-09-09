use super::utils::WatcherTest;
use anyhow::Result;
use pkarr::Keypair;
use pubky_nexus::models::{
    post::PostView,
    pubky_app::{PostEmbed, PostKind, PubkyAppPost, PubkyAppUser},
};

#[tokio::test]
async fn test_homeserver_repost() -> Result<()> {
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

    let post_post = PubkyAppPost {
        content: "This is a test post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
    };

    let post_id = test.create_post(&user_id, &post_post).await?;

    // Assert the new post can be served from Nexus
    let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    println!("New post served: {:?}", result_post.details);
    assert_eq!(result_post.details.id, post_id);
    assert_eq!(result_post.details.content, post_post.content);
    assert_eq!(
        result_post.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{post_id}")
    );
    assert_eq!(result_post.counts.reposts, 0);
    assert!(result_post.details.indexed_at > 0);
    assert_eq!(result_post.counts.tags, 0);
    assert_eq!(result_post.counts.replies, 0);

    // Create repost
    let post_uri = format!("pubky://{}/pub/pubky.app/posts/{}", user_id, post_id);

    let repost = PubkyAppPost {
        content: "This is a repost post!".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: post_uri.clone(),
        }),
    };

    let repost_id = test.create_post(&user_id, &repost).await?;

    // Assert the new repost can be served from Nexus
    let result_repost = PostView::get_by_id(&user_id, &repost_id, None, None, None)
        .await
        .unwrap()
        .expect("The repost was not served from Nexus");

    println!("New repost served: {:?}", result_repost.details);
    assert_eq!(result_repost.details.id, repost_id);
    assert_eq!(result_repost.details.content, repost.content);
    assert_eq!(
        result_repost.details.uri,
        format!("pubky://{user_id}/pub/pubky.app/posts/{repost_id}")
    );
    assert_eq!(result_repost.counts.reposts, 0);
    assert!(result_repost.details.indexed_at > 0);
    assert_eq!(result_repost.counts.tags, 0);
    assert_eq!(result_repost.counts.replies, 0);
    assert_eq!(result_repost.relationships.reposted, Some(post_uri));

    // Assert the post post has changed stats
    let result_post = PostView::get_by_id(&user_id, &repost_id, None, None, None)
        .await
        .unwrap()
        .expect("The new post was not served from Nexus");

    assert_eq!(result_post.counts.replies, 1);

    // // TODO: Impl DEL post. Assert the repost does not exist in Nexus
    // test.cleanup_post(&user_id, &repost_id).await?;
    // let result_post = PostView::get_by_id(&user_id, &post_id, None, None, None)
    //     .await
    //     .unwrap();

    // assert!(result_post.is_none(), "The post should have been deleted");

    // Cleanup
    test.cleanup_user(&user_id).await?;
    test.cleanup_post(&user_id, &post_id).await?;

    Ok(())
}
