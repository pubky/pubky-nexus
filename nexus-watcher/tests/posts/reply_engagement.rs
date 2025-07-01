use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
};
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    traits::HashId, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
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
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let parent_post_id = test.create_post(&author_id, &parent_post).await?;

    // Create reply
    let parent_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{parent_post_id}");

    let reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&author_id, &reply).await?;

    // Check if reply post is not in global timeline index: Sorted:Posts:Global:Timeline:user_id:post_id
    let global_timeline = check_member_global_timeline_user_post(&author_id, &reply_id)
        .await
        .unwrap();
    assert!(
        global_timeline.is_none(),
        "Replies cannot be added in the global timeline"
    );

    // Create a reply of a reply
    let reply_uri = format!("pubky://{author_id}/pub/pubky.app/posts/{reply_id}");

    let reply_of_reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:ReplyOfReply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(reply_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_reply_id = test.create_post(&author_id, &reply_of_reply).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be incremented in the total engagement list after receiving a reply"
    );
    // Check if reply post is not in global timeline index: Sorted:Posts:Global:Timeline:user_id:post_id
    let global_timeline = check_member_global_timeline_user_post(&author_id, &reply_id)
        .await
        .unwrap();
    assert!(
        global_timeline.is_none(),
        "Replies cannot be added in the global timeline"
    );

    // Create a repost of a reply
    let reply_repost = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Repost".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: Some(PubkyAppPostEmbed {
            kind: PubkyAppPostKind::Short,
            uri: reply_uri.clone(),
        }),
        attachments: None,
    };

    let reply_repost_id = test.create_post(&author_id, &reply_repost).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be incremented in the total engagement list after being reposted"
    );
    // Check if reply post is not in global timeline index: Sorted:Posts:Global:Timeline:user_id:post_id
    let global_timeline = check_member_global_timeline_user_post(&author_id, &reply_repost_id)
        .await
        .unwrap();
    assert!(
        global_timeline.is_some(),
        "Repost has to be added in the global timeline"
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
        uri: format!("pubky://{author_id}/pub/pubky.app/posts/{reply_id}"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        tagger_user_id,
        tag.create_id()
    );

    test.put(&tag_url, tag).await?;

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
    // Check if reply post is not in global timeline index: Sorted:Posts:Global:Timeline:user_id:post_id
    let global_timeline = check_member_global_timeline_user_post(&author_id, &reply_repost_id)
        .await
        .unwrap();
    assert!(
        global_timeline.is_none(),
        "Repost cannot be in global timeline after deletion"
    );

    test.del(&tag_url).await?;

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
