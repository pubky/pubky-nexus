//! Writes to the sorted sets every viewer shares go through the trust gate: an
//! unranked author's posts stay out of the global timeline and engagement sets,
//! the tag sets and other people's threads. Their own threads and the sets
//! scoped to them are untouched.
//!
//! `WatcherTest::create_user` ranks the users it creates; each test here
//! unranks the authors it needs hidden. No other watcher test rebuilds the
//! ranking, so the change holds for the test's duration.
use super::utils::{
    check_member_global_timeline_user_post, check_member_post_replies,
    check_member_total_engagement_user_posts, check_member_user_post_timeline,
    check_member_user_replies_timeline, short_post, test_user,
};
use crate::event_processor::tags::utils::{
    check_member_post_tag_global_timeline, check_member_total_engagement_post_tag,
};
use crate::event_processor::utils::watcher::{unrank_user, HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::notification::{Notification, NotificationBody};
use nexus_common::models::post::PostCounts;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostEmbed, PubkyAppPostKind, PubkyAppTag,
};

const BIO: &str = "trust gate";

#[tokio_shared_rt::test(shared)]
async fn test_unranked_posts_stay_out_of_shared_sets() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let label = "watchergatelabel";

    let ranked_kp = Keypair::random();
    let ranked_id = test
        .create_user(&ranked_kp, &test_user("Watcher:Gate:Ranked", BIO))
        .await?;
    let unranked_kp = Keypair::random();
    let unranked_id = test
        .create_user(&unranked_kp, &test_user("Watcher:Gate:Unranked", BIO))
        .await?;
    unrank_user(&unranked_id).await?;
    let fan_kp = Keypair::random();
    test.create_user(&fan_kp, &test_user("Watcher:Gate:Fan", BIO))
        .await?;

    let (ranked_post, _) = test
        .create_post(&ranked_kp, &short_post("Watcher:Gate:RankedPost"))
        .await?;
    let (unranked_post, _) = test
        .create_post(&unranked_kp, &short_post("Watcher:Gate:UnrankedPost"))
        .await?;

    // A tag and a reply on each post: the engagement writes must not create a
    // hidden post either.
    for (author_id, post_id) in [(&ranked_id, &ranked_post), (&unranked_id, &unranked_post)] {
        let uri = post_uri_builder(author_id.clone(), post_id.clone());
        let tag = PubkyAppTag {
            uri: uri.clone(),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        test.put(&fan_kp, &tag.hs_path(), tag).await?;
        let reply = PubkyAppPost {
            parent: Some(uri),
            ..short_post("Watcher:Gate:Reply")
        };
        test.create_post(&fan_kp, &reply).await?;
    }

    let ranked_key: &[&str] = &[&ranked_id, &ranked_post];
    assert!(
        check_member_global_timeline_user_post(&ranked_id, &ranked_post)
            .await?
            .is_some()
    );
    assert_eq!(
        check_member_total_engagement_user_posts(ranked_key).await?,
        Some(2),
        "one tag and one reply"
    );
    assert!(check_member_post_tag_global_timeline(ranked_key, label)
        .await?
        .is_some());
    assert_eq!(
        check_member_total_engagement_post_tag(ranked_key, label).await?,
        Some(1)
    );

    let unranked_key: &[&str] = &[&unranked_id, &unranked_post];
    assert!(
        check_member_global_timeline_user_post(&unranked_id, &unranked_post)
            .await?
            .is_none(),
        "global timeline"
    );
    assert!(
        check_member_total_engagement_user_posts(unranked_key)
            .await?
            .is_none(),
        "global engagement"
    );
    assert!(
        check_member_post_tag_global_timeline(unranked_key, label)
            .await?
            .is_none(),
        "tag timeline"
    );
    assert!(
        check_member_total_engagement_post_tag(unranked_key, label)
            .await?
            .is_none(),
        "tag engagement"
    );
    // Their own stream still has it.
    assert!(
        check_member_user_post_timeline(&unranked_id, &unranked_post)
            .await?
            .is_some()
    );
    Ok(())
}

/// A thread takes ranked repliers and the post's own author, ranked or not.
#[tokio_shared_rt::test(shared)]
async fn test_threads_keep_the_authors_own_replies() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let owner_kp = Keypair::random();
    let owner_id = test
        .create_user(&owner_kp, &test_user("Watcher:Gate:Owner", BIO))
        .await?;
    unrank_user(&owner_id).await?;
    let ranked_kp = Keypair::random();
    let ranked_id = test
        .create_user(&ranked_kp, &test_user("Watcher:Gate:RankedReplier", BIO))
        .await?;
    let stranger_kp = Keypair::random();
    let stranger_id = test
        .create_user(&stranger_kp, &test_user("Watcher:Gate:Stranger", BIO))
        .await?;
    unrank_user(&stranger_id).await?;

    let (post_id, _) = test
        .create_post(&owner_kp, &short_post("Watcher:Gate:Thread"))
        .await?;
    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(owner_id.clone(), post_id.clone())),
        ..short_post("Watcher:Gate:ThreadReply")
    };
    let mut replies = Vec::new();
    for kp in [&owner_kp, &ranked_kp, &stranger_kp] {
        let (reply_id, _) = test.create_post(kp, &reply).await?;
        replies.push(reply_id);
    }

    let expected = [
        (&owner_id, &replies[0], true),
        (&ranked_id, &replies[1], true),
        (&stranger_id, &replies[2], false),
    ];
    for (author_id, reply_id, kept) in expected {
        let score = check_member_post_replies(&owner_id, &post_id, &[author_id, reply_id]).await?;
        assert_eq!(score.is_some(), kept, "{author_id}'s reply");
    }
    // The stranger's own replies stream still has it.
    assert!(
        check_member_user_replies_timeline(&stranger_id, &replies[2])
            .await?
            .is_some()
    );
    Ok(())
}

/// An unranked user's reply, hidden from the thread, sends no notification;
/// their follow still does, like a ranked user's reply and follow.
#[tokio_shared_rt::test(shared)]
async fn test_unranked_replies_send_no_notifications() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let owner_kp = Keypair::random();
    let owner_id = test
        .create_user(&owner_kp, &test_user("Watcher:Gate:NotifiedOwner", BIO))
        .await?;
    let (post_id, _) = test
        .create_post(&owner_kp, &short_post("Watcher:Gate:NotifiedPost"))
        .await?;
    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(owner_id.clone(), post_id)),
        ..short_post("Watcher:Gate:NotifyingReply")
    };

    let mut actors = Vec::new();
    for (name, ranked) in [
        ("Watcher:Gate:RankedFan", true),
        ("Watcher:Gate:UnrankedFan", false),
    ] {
        let kp = Keypair::random();
        let user_id = test.create_user(&kp, &test_user(name, BIO)).await?;
        if !ranked {
            unrank_user(&user_id).await?;
        }
        test.create_post(&kp, &reply).await?;
        test.create_follow(&kp, &owner_id).await?;
        actors.push((user_id, ranked));
    }

    let notifications = Notification::get_by_id(&owner_id, Pagination::default()).await?;
    for (user_id, ranked) in actors {
        let sent = notifications
            .iter()
            .filter(|notification| notification.body.actor() == user_id)
            .count();
        let expected = if ranked { 2 } else { 1 };
        assert_eq!(sent, expected, "{user_id}: a reply and a follow");
    }
    Ok(())
}

/// A reply, repost and tag from an unranked user move the post's engagement
/// scores like anyone's, but its counts leave out the reply and repost, which
/// every feed hides; a ranked user's count in full.
#[tokio_shared_rt::test(shared)]
async fn test_unranked_engagement_scores_but_counts_only_tags() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let label = "watchergatescore";

    let owner_kp = Keypair::random();
    let owner_id = test
        .create_user(&owner_kp, &test_user("Watcher:Gate:ScoredOwner", BIO))
        .await?;
    let (post_id, _) = test
        .create_post(&owner_kp, &short_post("Watcher:Gate:ScoredPost"))
        .await?;
    let uri = post_uri_builder(owner_id.clone(), post_id.clone());
    let post_key: &[&str] = &[&owner_id, &post_id];

    let mut seen = Vec::new();
    for (name, ranked) in [
        ("Watcher:Gate:UnrankedEngager", false),
        ("Watcher:Gate:RankedEngager", true),
    ] {
        let kp = Keypair::random();
        let user_id = test.create_user(&kp, &test_user(name, BIO)).await?;
        if !ranked {
            unrank_user(&user_id).await?;
        }
        let reply = PubkyAppPost {
            parent: Some(uri.clone()),
            ..short_post("Watcher:Gate:ScoringReply")
        };
        let repost = PubkyAppPost {
            embed: Some(PubkyAppPostEmbed {
                kind: PubkyAppPostKind::Short,
                uri: uri.clone(),
            }),
            ..short_post("")
        };
        let tag = PubkyAppTag {
            uri: uri.clone(),
            label: label.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        test.create_post(&kp, &reply).await?;
        test.create_post(&kp, &repost).await?;
        test.put(&kp, &tag.hs_path(), tag).await?;
        let counts = PostCounts::get_by_id(&owner_id, &post_id)
            .await?
            .expect("the post's counts");
        seen.push((
            check_member_total_engagement_user_posts(post_key).await?,
            check_member_total_engagement_post_tag(post_key, label).await?,
            (counts.tags, counts.replies, counts.reposts),
        ));
    }

    // A label's engagement moves only with that label's tags.
    let unranked_then_ranked = vec![(Some(3), Some(1), (1, 0, 0)), (Some(6), Some(2), (2, 1, 1))];
    assert_eq!(
        seen, unranked_then_ranked,
        "(global, tag) engagement and (tags, replies, reposts)"
    );
    Ok(())
}

/// A thread owner's follow lets an unranked replier into their threads and
/// reply notifications; an unfollow takes the reply out, a new follow puts it
/// back.
#[tokio_shared_rt::test(shared)]
async fn test_followed_repliers_reach_the_owners_threads() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let owner_kp = Keypair::random();
    let owner_id = test
        .create_user(&owner_kp, &test_user("Watcher:Gate:TrustingOwner", BIO))
        .await?;
    let replier_kp = Keypair::random();
    let replier_id = test
        .create_user(&replier_kp, &test_user("Watcher:Gate:TrustedReplier", BIO))
        .await?;
    unrank_user(&replier_id).await?;
    let follow_path = test.create_follow(&owner_kp, &replier_id).await?;

    let (post_id, _) = test
        .create_post(&owner_kp, &short_post("Watcher:Gate:TrustingThread"))
        .await?;
    let reply = PubkyAppPost {
        parent: Some(post_uri_builder(owner_id.clone(), post_id.clone())),
        ..short_post("Watcher:Gate:TrustedReply")
    };
    let (reply_id, _) = test.create_post(&replier_kp, &reply).await?;
    let reply_key: &[&str] = &[&replier_id, &reply_id];

    let thread_replies = || async {
        let member = check_member_post_replies(&owner_id, &post_id, reply_key).await?;
        let counts = PostCounts::get_by_id(&owner_id, &post_id).await?;
        anyhow::Ok((member.is_some(), counts.map(|counts| counts.replies)))
    };

    let followed = thread_replies().await?;
    let notifications = Notification::get_by_id(&owner_id, Pagination::default()).await?;
    let notified = notifications.iter().any(|notification| {
        matches!(notification.body, NotificationBody::Reply { .. })
            && notification.body.actor() == replier_id
    });
    test.del(&owner_kp, &follow_path).await?;
    let unfollowed = thread_replies().await?;
    test.create_follow(&owner_kp, &replier_id).await?;
    let refollowed = thread_replies().await?;

    assert_eq!(followed, (true, Some(1)), "the owner follows the replier");
    assert!(notified, "the owner is notified of the reply");
    assert_eq!(
        unfollowed,
        (false, Some(0)),
        "the unfollow takes the reply out"
    );
    assert_eq!(refollowed, (true, Some(1)), "a new follow puts it back");
    Ok(())
}
