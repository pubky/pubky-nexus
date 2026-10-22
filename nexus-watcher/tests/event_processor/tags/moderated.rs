use super::resource_utils::{compute_resource_id, find_resource_tag};
use super::utils::find_post_tag;
use crate::event_processor::posts::utils::find_post_details;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::Result;
use chrono::Utc;
use pubky::{recovery_file, Keypair};
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, tag_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser,
};
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

#[tokio_shared_rt::test(shared)]
async fn test_moderation_deletes_pubky_app_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "pubky-app").await?;

    let label = "mod_pubky_app";
    let tag_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, label,
    )
    .await?;

    let post_tag = find_post_tag(&author_id, &post_id, label)
        .await?
        .expect("post tag should exist before moderation");
    assert_eq!(post_tag.taggers_count, 1);

    moderate_tag(&mut test, &moderator_kp, &tag_uri).await?;

    let post_tag = find_post_tag(&author_id, &post_id, label).await?;
    assert!(post_tag.is_none(), "pubky.app tag should be moderated");

    Ok(())
}

/// Nexus no longer indexes another app's tag folder, but the tags indexed from one before stay,
/// keyed by tagger and tag id like their `pubky.app` copy. A moderator tag naming such a tag's
/// app-folder address still takes it down, a resource tag and a post tag alike.
#[tokio_shared_rt::test(shared)]
async fn test_moderation_through_an_app_folder_address_deletes_the_tag() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "app-folder").await?;

    let target_uri = format!("https://example.com/moderate-app-folder/{tagger_id}");
    let resource_label = "mod_app_folder_resource";
    let (resource_tag_uri, resource_id) = put_pubky_app_resource_tag(
        &mut test,
        &tagger_kp,
        &tagger_id,
        &target_uri,
        resource_label,
    )
    .await?;
    let post_label = "mod_app_folder_post";
    let post_tag_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, post_label,
    )
    .await?;
    assert!(
        find_resource_tag(&resource_id, resource_label)
            .await?
            .is_some(),
        "resource tag should exist before moderation"
    );
    assert!(
        find_post_tag(&author_id, &post_id, post_label)
            .await?
            .is_some(),
        "post tag should exist before moderation"
    );

    moderate_tag(
        &mut test,
        &moderator_kp,
        &app_folder_address(&resource_tag_uri),
    )
    .await?;
    moderate_tag(&mut test, &moderator_kp, &app_folder_address(&post_tag_uri)).await?;

    assert!(
        find_resource_tag(&resource_id, resource_label)
            .await?
            .is_none(),
        "resource tag should be moderated through its app-folder address"
    );
    assert!(
        find_post_tag(&author_id, &post_id, post_label)
            .await?
            .is_none(),
        "post tag should be moderated through its app-folder address"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_moderation_keeps_same_label_resource_tag_from_same_user() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "same-user").await?;

    let label = "mod_same_user";
    let pubky_app_tag_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, label,
    )
    .await?;
    let target_uri = format!("https://example.com/same-user/{tagger_id}");
    let (_resource_tag_uri, resource_id) =
        put_pubky_app_resource_tag(&mut test, &tagger_kp, &tagger_id, &target_uri, label).await?;

    moderate_tag(&mut test, &moderator_kp, &pubky_app_tag_uri).await?;

    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "targeted pubky.app tag should be moderated"
    );
    assert!(
        find_resource_tag(&resource_id, label).await?.is_some(),
        "same-label resource tag from same user should remain"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_moderation_keeps_same_label_pubky_app_tag_from_different_user() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (pubky_tagger_kp, pubky_tagger_id) =
        create_tagger(&mut test, "different-user-pubky").await?;
    let (resource_tagger_kp, resource_tagger_id) =
        create_tagger(&mut test, "different-user-resource").await?;

    let label = "mod_diff_user";
    put_pubky_app_post_tag(
        &mut test,
        &pubky_tagger_kp,
        &pubky_tagger_id,
        &author_id,
        &post_id,
        label,
    )
    .await?;
    let target_uri = format!("https://example.com/different-user/{resource_tagger_id}");
    let (resource_tag_uri, resource_id) = put_pubky_app_resource_tag(
        &mut test,
        &resource_tagger_kp,
        &resource_tagger_id,
        &target_uri,
        label,
    )
    .await?;

    moderate_tag(&mut test, &moderator_kp, &resource_tag_uri).await?;

    assert!(
        find_resource_tag(&resource_id, label).await?.is_none(),
        "targeted resource tag should be moderated"
    );
    let post_tag = find_post_tag(&author_id, &post_id, label)
        .await?
        .expect("same-label pubky.app tag from another user should remain");
    assert_eq!(post_tag.taggers_count, 1);
    assert!(post_tag.taggers.contains(&pubky_tagger_id));

    Ok(())
}

/// A moderator tag whose target names nothing Nexus deletes (a URL) is a no-op that succeeds:
/// nothing is deleted or indexed.
#[tokio_shared_rt::test(shared)]
async fn test_moderation_of_an_unresolved_target_is_a_no_op() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let moderator_id = moderator_kp.public_key().to_z32();
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let tagged_before = find_user_counts(&moderator_id).await.tagged;

    // Switch OFF event processing so the tag is handled only by the call below
    test = test.remove_event_processing().await;

    let url = format!("https://example.com/moderate-other/{author_id}");
    let tag = PubkyAppTag {
        uri: url.clone(),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    test.put(&moderator_kp, &tag.hs_path(), &tag).await?;

    let tag_event = format!("PUT pubky://{moderator_id}/pub/pubky.app/tags/{tag_id}");
    let event_handler = test.event_processor_runner.event_handler.clone();
    retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .unwrap_or_else(|err| panic!("moderating {url} should succeed: {err:?}"));

    let post = find_post_details(&author_id, &post_id).await?;
    assert!(!post.deleted, "the v0 post must not be moderated");
    assert!(
        find_resource_tag(&compute_resource_id(&url), MODERATION_LABEL)
            .await?
            .is_none(),
        "the moderator tag must not be indexed on the URL"
    );
    assert_eq!(
        find_user_counts(&moderator_id).await.tagged,
        tagged_before,
        "the moderator tag must not be indexed"
    );

    Ok(())
}

/// A post has one node whichever epoch's address names it: a moderator tag written as the
/// `social/v1` address of an indexed `pubky.app` post takes that post down.
#[tokio_shared_rt::test(shared)]
async fn test_moderation_through_the_social_v1_address_of_a_post_takes_it_down() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;

    let social_v1_post = format!("pubky://{author_id}/pub/social/v1/posts/{post_id}");
    moderate_tag(&mut test, &moderator_kp, &social_v1_post).await?;

    // Edge-free, so hard-deleted rather than tombstoned.
    assert!(
        find_post_details(&author_id, &post_id).await.is_err(),
        "the post must be taken down through its v1 address"
    );

    Ok(())
}

/// A takedown of a tag is recorded under its address, so it holds when the same tag is replayed.
#[tokio_shared_rt::test(shared)]
async fn test_takedown_of_a_tag_holds_on_replay() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "tag-replay").await?;

    let label = "mod_tag_replay";
    let tag_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, label,
    )
    .await?;
    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_some(),
        "the tag should exist before moderation"
    );

    moderate_tag(&mut test, &moderator_kp, &tag_uri).await?;
    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "the tag should be moderated"
    );

    let replayed_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, label,
    )
    .await?;
    assert_eq!(replayed_uri, tag_uri, "the same tag, replayed");
    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "the takedown holds when the tag is replayed"
    );

    Ok(())
}

async fn create_moderator(test: &mut WatcherTest) -> Result<Keypair> {
    let moderator_recovery_file =
        fs::read("./tests/event_processor/utils/moderator_key.pkarr").await?;
    let moderator_kp = recovery_file::decrypt_recovery_file(&moderator_recovery_file, "password")?;
    let moderator = test_user("Mod:Moderator", "moderator");
    test.create_user(&moderator_kp, &moderator).await?;
    Ok(moderator_kp)
}

async fn create_author_and_post(test: &mut WatcherTest) -> Result<(Keypair, String, String)> {
    let author_kp = Keypair::random();
    let author = test_user("Mod:Author", "author");
    let author_id = test.create_user(&author_kp, &author).await?;
    let post = PubkyAppPost {
        content: format!("Watcher:TagModerate:Post:{author_id}"),
        ..Default::default()
    };
    let (post_id, _) = test.create_post(&author_kp, &post).await?;
    Ok((author_kp, author_id, post_id))
}

async fn create_tagger(test: &mut WatcherTest, suffix: &str) -> Result<(Keypair, String)> {
    let tagger_kp = Keypair::random();
    let tagger = test_user("Mod:Tagger", suffix);
    let tagger_id = test.create_user(&tagger_kp, &tagger).await?;
    Ok((tagger_kp, tagger_id))
}

async fn put_pubky_app_post_tag(
    test: &mut WatcherTest,
    tagger_kp: &Keypair,
    tagger_id: &str,
    author_id: &str,
    post_id: &str,
    label: &str,
) -> Result<String> {
    let tag = PubkyAppTag {
        uri: post_uri_builder(author_id.to_string(), post_id.to_string()),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let tag_uri = tag_uri_builder(tagger_id.to_string(), tag_id);
    let tag_path = tag.hs_path();
    test.put(tagger_kp, &tag_path, tag).await?;
    Ok(tag_uri)
}

async fn put_pubky_app_resource_tag(
    test: &mut WatcherTest,
    tagger_kp: &Keypair,
    tagger_id: &str,
    target_uri: &str,
    label: &str,
) -> Result<(String, String)> {
    let tag = PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let resource_id = compute_resource_id(target_uri);
    let tag_uri = tag_uri_builder(tagger_id.to_string(), tag_id);
    let tag_path = tag.hs_path();
    test.put(tagger_kp, &tag_path, tag).await?;
    Ok((tag_uri, resource_id))
}

/// The address the same tag file had in another app's folder (`mapky`).
fn app_folder_address(pubky_app_tag_uri: &str) -> String {
    pubky_app_tag_uri.replace("/pub/pubky.app/tags/", "/pub/mapky/tags/")
}

async fn moderate_tag(
    test: &mut WatcherTest,
    moderator_kp: &Keypair,
    target_tag_uri: &str,
) -> Result<()> {
    let tag = PubkyAppTag {
        uri: target_tag_uri.to_string(),
        label: MODERATION_LABEL.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path = tag.hs_path();
    test.put(moderator_kp, &tag_path, tag).await
}

fn test_user(name: &str, bio: &str) -> PubkyAppUser {
    PubkyAppUser {
        bio: Some(bio.to_string()),
        name: name.to_string(),
        ..Default::default()
    }
}
