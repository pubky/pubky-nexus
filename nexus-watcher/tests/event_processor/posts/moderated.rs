use crate::event_processor::{
    posts::utils::{find_post_details, short_post, test_user},
    tags::utils::check_member_post_tag_global_timeline,
    users::utils::find_user_counts,
    utils::watcher::{HomeserverHashIdPath, WatcherTest},
};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::post::PostCounts;
use pubky::{recovery_file, Keypair};
use pubky_app_specs::{
    post_uri_builder, PubkyAppPost, PubkyAppPostKind, PubkyAppTag, PubkyAppUser,
};
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

#[tokio_shared_rt::test(shared)]
async fn test_moderated_post_lifecycle() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // 1. User signup and writes a post
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_post_to_moderate".to_string()),
        image: None,
        links: None,
        name: "Watcher:PostModerate:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let post = PubkyAppPost {
        content: "Watcher:PostModerate:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let (post_id, _post_path) = test.create_post(&user_kp, &post).await?;

    // 2. Confirm this post does exist
    let post_details = find_post_details(&user_id, &post_id).await.unwrap();
    assert_eq!(post_details.id, post_id);

    // 3. Load moderation service key and place a tag on that post with label "label_to_moderate"
    let moderator_recovery_file = fs::read("./tests/event_processor/utils/moderator_key.pkarr")
        .await
        .unwrap();
    let moderator_key =
        recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password").unwrap();

    test.create_user(&moderator_key, &user).await?;

    let tag = PubkyAppTag {
        uri: post_uri_builder(user_id.clone(), post_id.clone()),
        label: "label_to_moderate".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    // Put tag
    test.put(&moderator_key, &tag_path, tag).await?;

    // 4. Confirm the post does not exist
    let post_details = find_post_details(&user_id, &post_id).await;
    assert!(post_details.is_err());

    Ok(())
}

/// Moderating a post that still has a tag from another user must tombstone the
/// post (content becomes "[DELETED]") instead of DETACH DELETEing the node,
/// so the tagger's counts and the tag timeline index stay consistent.
#[tokio_shared_rt::test(shared)]
async fn test_moderated_post_with_tag_is_tombstoned() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Author signup and post creation
    let author_kp = Keypair::random();
    let author = test_user("Watcher:PostModerateTagged:Author", "author");
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = short_post("Watcher:PostModerateTagged:Post");
    let (post_id, _post_path) = test.create_post(&author_kp, &post).await?;

    // A second user tags the post
    let tagger_kp = Keypair::random();
    let tagger = test_user("Watcher:PostModerateTagged:Tagger", "tagger");
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;

    let label = "mod_tagged_post";
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.clone(), post_id.clone()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(&tagger_kp, &tag_path, tag).await?;

    // Sanity: the tag is indexed before moderation
    let post_key: [&str; 2] = [&author_id, &post_id];
    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        1,
        "tagger's tagged count should be 1 before moderation"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_some(),
        "tag timeline entry should exist before moderation"
    );

    // Moderator flags the post
    let moderator_kp = create_moderator(&mut test).await?;
    moderate_post(&mut test, &moderator_kp, &author_id, &post_id).await?;

    // The post node survives as a tombstone with its content replaced
    let post_details = find_post_details(&author_id, &post_id)
        .await
        .expect("moderated post with edges should still exist as a tombstone");
    assert_eq!(
        post_details.content, "[DELETED]",
        "moderated post content should exactly be [DELETED]"
    );

    // The tagger's state stays intact: counts and tag timeline are untouched
    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        1,
        "tagger's tagged count should stay intact after moderation"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_some(),
        "tag timeline entry should stay intact after moderation"
    );

    // End-to-end kill shot: the tagger deletes their tag on the tombstoned post.
    // Because the TAGGED edge survived moderation, the DEL event runs the normal
    // cleanup path. With a DETACH DELETE (the old behavior) the edge is gone,
    // tag::del hits its idempotent no-op and the state below stays orphaned.
    test.del(&tagger_kp, &tag_path).await?;

    assert_eq!(
        find_user_counts(&tagger_id).await.tagged,
        0,
        "tagger's tagged count should drop to 0 once they delete their tag"
    );
    assert!(
        check_member_post_tag_global_timeline(&post_key, label)
            .await?
            .is_none(),
        "tag timeline entry should be gone once the tagger deletes their tag"
    );

    Ok(())
}

/// Moderating a post without any edges must still fully delete it from both
/// the graph and the Redis indexes, exactly as before the tombstone gating.
#[tokio_shared_rt::test(shared)]
async fn test_moderated_bare_post_is_hard_deleted() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    // Author signup and post creation
    let author_kp = Keypair::random();
    let author = test_user("Watcher:PostModerateBare:Author", "author");
    let author_id = test.create_user(&author_kp, &author).await?;

    let post = short_post("Watcher:PostModerateBare:Post");
    let (post_id, _post_path) = test.create_post(&author_kp, &post).await?;

    // Sanity: the post exists before moderation
    assert!(find_post_details(&author_id, &post_id).await.is_ok());

    // Moderator flags the post
    let moderator_kp = create_moderator(&mut test).await?;
    moderate_post(&mut test, &moderator_kp, &author_id, &post_id).await?;

    // The edge-free post is hard-deleted from the graph, not tombstoned
    assert!(
        find_post_details(&author_id, &post_id).await.is_err(),
        "bare moderated post should be fully deleted from the graph"
    );

    // The cached counts are gone as well
    let post_counts = PostCounts::get_from_index(&author_id, &post_id)
        .await
        .unwrap();
    assert!(
        post_counts.is_none(),
        "bare moderated post counts should be deleted from the index"
    );

    Ok(())
}

async fn create_moderator(test: &mut WatcherTest) -> Result<Keypair> {
    let moderator_recovery_file =
        fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp = recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password")?;
    let moderator = test_user("Watcher:PostModerate:Moderator", "moderator");
    test.create_user(&moderator_kp, &moderator).await?;
    Ok(moderator_kp)
}

async fn moderate_post(
    test: &mut WatcherTest,
    moderator_kp: &Keypair,
    author_id: &str,
    post_id: &str,
) -> Result<()> {
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.to_string(), post_id.to_string()),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(moderator_kp, &tag_path, tag).await
}
