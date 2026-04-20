use crate::event_processor::utils::watcher::{
    retrieve_and_handle_event_line, HomeserverHashIdPath, WatcherTest,
};
use anyhow::{anyhow, Result};
use chrono::Utc;
use nexus_watcher::service::TEventProcessorRunner;
use pubky::Keypair;
use pubky_app_specs::{post_uri_builder, traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use tracing::error;

/// Verifies that tagging fails with MissingDependency when the tagger or tagged resource
/// is not yet indexed in the graph.
#[tokio_shared_rt::test(shared)]
async fn test_homeserver_tag_cannot_add_while_index() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let tagged_keypair = Keypair::random();
    let tagged_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_user_not_found".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotTag:Tagged:Sync".to_string(),
        status: None,
    };
    let tagged_user_id = test.create_user(&tagged_keypair, &tagged_user).await?;

    // Switch OFF event processing — shadow_user signs up on the homeserver but is not indexed
    test = test.remove_event_processing().await;

    let shadow_user_kp = Keypair::random();
    let shadow_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_user_not_found".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotTag:Tagger:Sync".to_string(),
        status: None,
    };
    let shadow_user_id = test.create_user(&shadow_user_kp, &shadow_user).await?;

    // => User tag: shadow_user tags tagged_user's profile
    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: "friendly".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_id = tag.create_id();
    let tag_path = tag.hs_path();

    test.put(&shadow_user_kp, &tag_path, &tag).await?;

    // Full URI required by Event::parse_event
    let tag_event = format!("PUT pubky://{shadow_user_id}/pub/pubky.app/tags/{tag_id}");

    let event_handler = test.event_processor_runner.event_handler.clone();
    let sync_fail = retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "Tag indexing should fail: tagger (shadow_user) is not yet indexed"
    );

    // Sync all pending events so shadow_user is now in the graph
    test.event_processor_runner
        .build(test.homeserver_id.clone())
        .await
        .map_err(|e| anyhow!(e))?
        .run()
        .await
        .map_err(|e| anyhow!(e))?;

    // => Post tag: shadow_user tags a post that hasn't been indexed yet
    let post = PubkyAppPost {
        content: "Watcher:CannotTag:Post:unSync".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let (post_id, _post_path) = test.create_post(&tagged_keypair, &post).await?;

    let post_tag = PubkyAppTag {
        uri: post_uri_builder(tagged_user_id, post_id),
        label: "merkle_tree".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let post_tag_id = post_tag.create_id();
    let post_tag_path = post_tag.hs_path();

    test.put(&shadow_user_kp, &post_tag_path, &post_tag).await?;

    let tag_event = format!("PUT pubky://{shadow_user_id}/pub/pubky.app/tags/{post_tag_id}");

    let event_handler = test.event_processor_runner.event_handler.clone();
    let sync_fail = retrieve_and_handle_event_line(&tag_event, event_handler)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "Tag indexing should fail: tagged post is not yet indexed"
    );

    Ok(())
}
