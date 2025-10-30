use crate::event_processor::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::{anyhow, Result};
use chrono::Utc;
use nexus_watcher::service::TEventProcessorRunner;
use pubky::Keypair;
use pubky_app_specs::{
    post_uri_builder,
    traits::{HasIdPath, HashId},
    PubkyAppPost, PubkyAppTag, PubkyAppUser,
};
use tracing::error;

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

    // Switch OFF the event processor to simulate the pending events to index
    // In that case, shadow user
    test = test.remove_event_processing().await;

    // Create a key but it would not be synchronised in nexus
    let shadow_user_kp = Keypair::random();
    let shadow_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_user_not_found".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotTag:Tagger:Sync".to_string(),
        status: None,
    };
    let _shadow_user_id = test.create_user(&shadow_user_kp, &shadow_user).await?;

    // => Create user tag
    let label = "friendly";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_relative_url = PubkyAppTag::create_path(&tag.create_id());

    // PUT user tag
    test.put(&shadow_user_kp, &tag_relative_url, tag_blob)
        .await?;

    // Create raw event line to retrieve the content from the homeserver
    let tag_event = format!("PUT {tag_relative_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let moderation_ref = test.event_processor_runner.moderation.clone();
    let sync_fail = retrieve_and_handle_event_line(&tag_event, moderation_ref)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "It seems that tagged node exists, which should not be possible. Event processor should be disconnected"
    );

    // Build the event processor and run it to sync all the previous events with the event processor
    // We do this because earlier, the runner's event processing has been turned off temporarily
    // but at this point we are ready to run the event processing
    test.event_processor_runner
        .build(test.homeserver_id.clone())
        .await
        .map_err(|e| anyhow!(e))?
        .run()
        .await
        .map_err(|e| anyhow!(e))?;

    // => Create post tag
    let post = PubkyAppPost {
        content: "Watcher:CannotTag:Post:unSync".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&tagged_keypair, &post).await?;

    let label = "merkle_tree";

    let tag = PubkyAppTag {
        uri: post_uri_builder(tagged_user_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_relative_url = PubkyAppTag::create_path(&tag.create_id());
    // PUT post tag
    test.put(&shadow_user_kp, &tag_relative_url, tag_blob)
        .await?;

    // Create raw event line to retrieve the content from the homeserver
    let tag_event = format!("PUT {tag_relative_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let moderation_ref = test.event_processor_runner.moderation.clone();
    let sync_fail = retrieve_and_handle_event_line(&tag_event, moderation_ref)
        .await
        .map_err(|e| error!("SYNC ERROR: {:?}", e))
        .is_err();

    assert!(
        sync_fail,
        "It seems that tagged node exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
