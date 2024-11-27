use super::utils::check_member_total_engagement_user_posts;
use crate::{service::stream::author, watcher::utils::WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{
    traits::HashId, PostEmbed, PostKind, PubkyAppPost, PubkyAppTag, PubkyAppUser,
};
use pubky_common::crypto::Keypair;

#[tokio::test]
async fn test_homeserver_reply_engagement_control() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_reply_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyEng:User".to_string(),
        status: None,
    };

    let author_id = test.create_user(&keypair, &user).await?;

    // Create root Post
    let parent_post = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Post".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let parent_post_id = test.create_post(&author_id, &parent_post).await?;

    // Create reply
    let parent_uri = format!(
        "pubky://{}/pub/pubky.app/posts/{}",
        author_id, parent_post_id
    );

    let reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Reply".to_string(),
        kind: PostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&author_id, &reply).await?;

    // Create a reply of a reply
    let reply_uri = format!("pubky://{}/pub/pubky.app/posts/{}", author_id, reply_id);

    let reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:ReplyOfReply".to_string(),
        kind: PostKind::Short,
        parent: Some(reply_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_reply_id = test.create_post(&author_id, &reply).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be incremented in the total engagement list after receiving a reply"
    );

    // Create a repost of a reply
    let repost = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Repost".to_string(),
        kind: PostKind::Short,
        parent: None,
        embed: Some(PostEmbed {
            kind: PostKind::Short,
            uri: reply_uri.clone(),
        }),
        attachments: None,
    };

    let reply_repost_id = test.create_post(&author_id, &repost).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be incremented in the total engagement list after being reposted"
    );

    let tagger_keypair = Keypair::random();

    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_reply_engagement_control".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyEngagement:Tagger".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&tagger_keypair, &tagger).await?;
    let label = "ignore_score";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", author_id, reply_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;

    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    test.create_tag(&tag_url, tag_blob).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be incremented in the total engagement list after being tagged"
    );

    // Start deleting the posts and tags added to the reply
    // Delete the reply
    test.cleanup_post(&author_id, &reply_reply_id).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be decremented in the total engagement list after deleting a reply"
    );

    test.cleanup_post(&author_id, &reply_repost_id).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be decremented in the total engagement list after deleting a repost"
    );

    test.delete_tag(&tag_url).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be decremented in the total engagement list after deleting a tag"
    );

    Ok(())
}
