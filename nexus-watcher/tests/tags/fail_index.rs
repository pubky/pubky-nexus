use crate::utils::watcher::{retrieve_and_handle_event_line, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};
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
    let shadow_keypair = Keypair::random();
    let shadow_user = PubkyAppUser {
        bio: Some("test_homeserver_tag_user_not_found".to_string()),
        image: None,
        links: None,
        name: "Watcher:CannotTag:Tagger:Sync".to_string(),
        status: None,
    };
    let shadow_user_id = test.create_user(&shadow_keypair, &shadow_user).await?;

    // => Create user tag
    let label = "friendly";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/profile.json"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        shadow_user_id,
        tag.create_id()
    );

    // PUT user tag
    test.put(tag_url.as_str(), tag_blob).await?;

    // Create raw event line to retrieve the content from the homeserver
    let tag_event = format!("PUT {tag_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&tag_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that tagged node exists, which should not be possible. Event processor should be disconnected"
    );

    // Sync all the previous events
    test.event_processor.run().await.unwrap();

    // => Create post tag
    let post = PubkyAppPost {
        content: "Watcher:CannotTag:Post:unSync".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&tagged_user_id, &post).await?;

    let label = "merkle_tree";

    let tag = PubkyAppTag {
        uri: format!("pubky://{tagged_user_id}/pub/pubky.app/posts/{post_id}"),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        shadow_user_id,
        tag.create_id()
    );
    // PUT post tag
    test.put(&tag_url, tag_blob).await?;

    // Create raw event line to retrieve the content from the homeserver
    let tag_event = format!("PUT {tag_url}");

    // Simulate the event processor to handle the event.
    // If the event processor were activated, the test would not catch the missing dependency
    // error, and it would pass successfully
    let sync_fail = retrieve_and_handle_event_line(&tag_event)
        .await
        .map_err(|e| {
            error!("SYNC ERROR: {:?}", e);
        })
        .is_err();

    assert!(
        sync_fail,
        "It seems that tagged node exists, which should not be possible. Event processor should be disconnected"
    );

    Ok(())
}
