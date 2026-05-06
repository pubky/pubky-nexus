use super::resource_utils::{compute_resource_id, find_resource_tag};
use super::utils::find_post_tag;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::{recovery_file, Keypair, ResourcePath};
use pubky_app_specs::traits::HashId;
use pubky_app_specs::{post_uri_builder, tag_uri_builder, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use tokio::fs;

const MODERATION_LABEL: &str = "label_to_moderate";

#[tokio_shared_rt::test(shared)]
async fn test_moderation_deletes_pubky_app_tag() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
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

#[tokio_shared_rt::test(shared)]
async fn test_moderation_deletes_universal_tag() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "universal").await?;

    let target_uri = format!("https://example.com/moderate-universal/{tagger_id}");
    let label = "mod_universal";
    let (tag_uri, resource_id) = put_universal_resource_tag(
        &mut test,
        &tagger_kp,
        &tagger_id,
        "mapky",
        &target_uri,
        label,
    )
    .await?;

    assert!(
        find_resource_tag(&resource_id, label).await?.is_some(),
        "universal tag should exist before moderation"
    );

    moderate_tag(&mut test, &moderator_kp, &tag_uri).await?;

    assert!(
        find_resource_tag(&resource_id, label).await?.is_none(),
        "universal tag should be moderated"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_moderation_deletes_app_specific_known_post_tag() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "known-post").await?;

    let label = "mod_known_post";
    let tag_uri = put_app_specific_post_tag(
        &mut test, &tagger_kp, &tagger_id, "mapky", &author_id, &post_id, label,
    )
    .await?;

    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_some(),
        "app-specific known post tag should exist before moderation"
    );

    moderate_tag(&mut test, &moderator_kp, &tag_uri).await?;

    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "app-specific known post tag should be moderated"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_moderation_keeps_same_label_universal_tag_from_same_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (tagger_kp, tagger_id) = create_tagger(&mut test, "same-user").await?;

    let label = "mod_same_user";
    let pubky_app_tag_uri = put_pubky_app_post_tag(
        &mut test, &tagger_kp, &tagger_id, &author_id, &post_id, label,
    )
    .await?;
    let target_uri = format!("https://example.com/same-user/{tagger_id}");
    let (_universal_tag_uri, resource_id) = put_universal_resource_tag(
        &mut test,
        &tagger_kp,
        &tagger_id,
        "mapky",
        &target_uri,
        label,
    )
    .await?;

    moderate_tag(&mut test, &moderator_kp, &pubky_app_tag_uri).await?;

    assert!(
        find_post_tag(&author_id, &post_id, label).await?.is_none(),
        "targeted pubky.app tag should be moderated"
    );
    assert!(
        find_resource_tag(&resource_id, label).await?.is_some(),
        "same-label universal tag from same user should remain"
    );

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_moderation_keeps_same_label_pubky_app_tag_from_different_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    let moderator_kp = create_moderator(&mut test).await?;
    let (_author_kp, author_id, post_id) = create_author_and_post(&mut test).await?;
    let (pubky_tagger_kp, pubky_tagger_id) =
        create_tagger(&mut test, "different-user-pubky").await?;
    let (universal_tagger_kp, universal_tagger_id) =
        create_tagger(&mut test, "different-user-universal").await?;

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
    let target_uri = format!("https://example.com/different-user/{universal_tagger_id}");
    let (universal_tag_uri, resource_id) = put_universal_resource_tag(
        &mut test,
        &universal_tagger_kp,
        &universal_tagger_id,
        "mapky",
        &target_uri,
        label,
    )
    .await?;

    moderate_tag(&mut test, &moderator_kp, &universal_tag_uri).await?;

    assert!(
        find_resource_tag(&resource_id, label).await?.is_none(),
        "targeted universal tag should be moderated"
    );
    let post_tag = find_post_tag(&author_id, &post_id, label)
        .await?
        .expect("same-label pubky.app tag from another user should remain");
    assert_eq!(post_tag.taggers_count, 1);
    assert!(post_tag.taggers.contains(&pubky_tagger_id));

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
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
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

async fn put_app_specific_post_tag(
    test: &mut WatcherTest,
    tagger_kp: &Keypair,
    tagger_id: &str,
    app: &str,
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
    let tag_uri = format!("pubky://{tagger_id}/pub/{app}/tags/{tag_id}");
    let tag_path: ResourcePath = format!("/pub/{app}/tags/{tag_id}").parse()?;
    test.put(tagger_kp, &tag_path, tag).await?;
    Ok(tag_uri)
}

async fn put_universal_resource_tag(
    test: &mut WatcherTest,
    tagger_kp: &Keypair,
    tagger_id: &str,
    app: &str,
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
    let tag_uri = format!("pubky://{tagger_id}/pub/{app}/tags/{tag_id}");
    let tag_path: ResourcePath = format!("/pub/{app}/tags/{tag_id}").parse()?;
    test.put(tagger_kp, &tag_path, tag).await?;
    Ok((tag_uri, resource_id))
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
        image: None,
        links: None,
        name: name.to_string(),
        status: None,
    }
}
