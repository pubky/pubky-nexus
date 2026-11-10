use super::resource_utils::{
    compute_resource_id, count_resource_tags, find_resource_tag, resource_exists_in_graph,
};
use super::utils::find_tag_uri;
use crate::event_processor::posts::utils::resource_key;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::tag::TagResource;
use nexus_watcher::events::handlers;
use pubky::{Keypair, ResourcePath};
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{tag_uri_builder, PubkyAppTag, PubkyAppUser};

fn tag_on(target_uri: &str, label: &str) -> PubkyAppTag {
    PubkyAppTag {
        uri: target_uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    }
}

fn mapky_path(tag_id: &str) -> ResourcePath {
    format!("/pub/mapky/tags/{tag_id}").parse().unwrap()
}

async fn new_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("app_folder_skipped".to_string()),
        image: None,
        links: None,
        name: format!("Watcher:AppFolderTag:{name}"),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;
    Ok((user_kp, user_id))
}

/// Another app's tag folder is skipped, PUT and DEL alike. A tag written only there is never
/// indexed. A tag indexed from `pubky.app` keeps its edge and its address when the same file is
/// written to, then deleted from, an app folder: the tag id ignores the folder, so an app moving
/// its tags out of its folder doesn't lose them when it deletes the old files.
#[tokio_shared_rt::test(shared)]
async fn test_app_folder_tag_put_and_del_are_skipped() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = new_user(&mut test, "Skipped").await?;

    // Written only in the app folder: never indexed
    let only_uri = format!("https://example.com/app-folder-only/{user_id}");
    let only = tag_on(&only_uri, "app-folder-only");
    let only_path = mapky_path(&only.create_id());
    test.put(&user_kp, &only_path, &only).await?;
    assert!(
        !resource_exists_in_graph(&compute_resource_id(&only_uri)).await?,
        "a tag in an app folder must not create a Resource node"
    );
    test.del(&user_kp, &only_path).await?;

    // Indexed from `pubky.app`, then copied to and deleted from the app folder
    let target_uri = format!("https://example.com/app-folder-copy/{user_id}");
    let label = "app-folder-copy";
    let resource_id = compute_resource_id(&target_uri);
    let tag = tag_on(&target_uri, label);
    let tag_id = tag.create_id();
    let pubky_app_path = tag.hs_path();
    test.put(&user_kp, &pubky_app_path, &tag).await?;

    let app_folder_path = mapky_path(&tag_id);
    test.put(&user_kp, &app_folder_path, &tag).await?;
    assert_eq!(
        find_tag_uri(&user_id, &tag_id).await,
        Some(tag_uri_builder(user_id.clone(), tag_id.clone())),
        "the app-folder PUT leaves the edge's address alone"
    );

    test.del(&user_kp, &app_folder_path).await?;
    assert_eq!(
        count_resource_tags(&resource_id).await?,
        1,
        "the app-folder DEL leaves the edge"
    );

    test.del(&user_kp, &pubky_app_path).await?;
    assert_eq!(count_resource_tags(&resource_id).await?, 0);
    test.cleanup_user(&user_kp).await?;

    Ok(())
}

/// A resource tag edge indexed from an app folder before Nexus skipped them stays indexed when
/// its file is deleted: the DEL is skipped like any app-folder event.
#[tokio_shared_rt::test(shared)]
async fn test_legacy_app_folder_tag_survives_its_del() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;
    let (user_kp, user_id) = new_user(&mut test, "Legacy").await?;

    let target_uri = format!("https://example.com/app-folder-legacy/{user_id}");
    let label = "app-folder-legacy";
    let resource_id = compute_resource_id(&target_uri);
    let tag = tag_on(&target_uri, label);
    let tag_id = tag.create_id();
    let app_folder_path = mapky_path(&tag_id);

    // The file is on the homeserver; its edge is written as the app-folder flow once wrote it
    test.put(&user_kp, &app_folder_path, &tag).await?;
    TagResource::put_to_graph_resource(
        &user_id,
        &resource_id,
        &target_uri,
        "https",
        &tag_id,
        &format!("pubky://{user_id}/pub/mapky/tags/{tag_id}"),
        label,
        Utc::now().timestamp_millis(),
    )
    .await?;

    test.del(&user_kp, &app_folder_path).await?;
    let survived = find_resource_tag(&resource_id, label).await?;

    // Clean up before asserting, so a failure leaves nothing behind
    handlers::tag::del(resource_key(&user_id, &tag_id)?).await?;
    test.cleanup_user(&user_kp).await?;

    let survived = survived.expect("the legacy edge survives the app-folder DEL");
    assert_eq!(survived.tagger, user_id);
    assert!(!resource_exists_in_graph(&resource_id).await?);

    Ok(())
}
