use super::utils::{
    check_member_global_timeline_user_post, check_member_total_engagement_user_posts,
};
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder,
    traits::{HasIdPath, HashId},
    PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_reply_engagement_control() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let user_kp = Keypair::random();

    let user = PubkyAppUser {
        bio: Some("test_homeserver_reply_repost".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyEng:User".to_string(),
        status: None,
    };

    let author_id = test.create_user(&user_kp, &user).await?;

    // Create root Post
    let parent_post = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let parent_post_id = test.create_post(&user_kp, &parent_post).await?;

    // Create reply
    let parent_absolute_uri = post_uri_builder(author_id.clone(), parent_post_id);

    let reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:Reply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(parent_absolute_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_id = test.create_post(&user_kp, &reply).await?;

    // Check if reply post is not in global timeline index: Sorted:Posts:Global:Timeline:user_id:post_id
    let global_timeline = check_member_global_timeline_user_post(&author_id, &reply_id)
        .await
        .unwrap();
    assert!(
        global_timeline.is_none(),
        "Replies cannot be added in the global timeline"
    );

    // Create a reply of a reply
    let reply_absolute_uri = post_uri_builder(author_id.clone(), reply_id.clone());

    let reply_of_reply = PubkyAppPost {
        content: "Watcher:ReplyEngagement:User:ReplyOfReply".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: Some(reply_absolute_uri.clone()),
        embed: None,
        attachments: None,
    };

    let reply_reply_id = test.create_post(&user_kp, &reply_of_reply).await?;

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
            uri: reply_absolute_uri.clone(),
        }),
        attachments: None,
    };

    let reply_repost_id = test.create_post(&user_kp, &reply_repost).await?;

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

    let tagger_kp = Keypair::random();

    let tagger = PubkyAppUser {
        bio: Some("test_homeserver_reply_engagement_control".to_string()),
        image: None,
        links: None,
        name: "Watcher:ReplyEngagement:Tagger".to_string(),
        status: None,
    };
    let _tagger_user_id = test.create_user(&tagger_kp, &tagger).await?;
    let label = "ignore_score";

    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), reply_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_relative_url = PubkyAppTag::create_path(&tag.create_id());
    test.put(&tagger_kp, &tag_relative_url, tag).await?;

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
    test.cleanup_post(&user_kp, &reply_reply_id).await?;

    // Check if reply post is not in total engagement index: Sorted:Posts:Global:TotalEngagement:user_id:post_id
    let total_engagement = check_member_total_engagement_user_posts(&[&author_id, &reply_id])
        .await
        .unwrap();
    assert!(
        total_engagement.is_none(),
        "Replies score cannot be decremented in the total engagement list after deleting a reply"
    );

    test.cleanup_post(&user_kp, &reply_repost_id).await?;

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

    test.del(&tagger_kp, &tag_relative_url).await?;

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
