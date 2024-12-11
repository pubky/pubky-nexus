use crate::watcher::utils::WatcherTest;
use anyhow::Result;
use log::{error, info};
use pubky_app_specs::{PubkyAppPost, PubkyAppPostKind, PubkyAppUser};
use pubky_common::crypto::Keypair;
use pubky_nexus::events::Event;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_unresolved_user() -> Result<()> {
    let mut test = WatcherTest::setup().await?;
    // Index homeserver events manually
    test = test.remove_event_processing().await;

    let keypair = Keypair::random();

    let shadow_user = PubkyAppUser {
        bio: Some("test_homeserver_unresolved_user".to_string()),
        image: None,
        links: None,
        name: "Watcher:UnresolvedUser:ShadowUser".to_string(),
        status: None,
    };

    let shadow_user_id = test.create_user(&keypair, &shadow_user).await?;

    let post = PubkyAppPost {
        content: "Watcher:UnresolvedUser:ShadowUser:Post".to_string(),
        kind: PubkyAppPostKind::Short,
        parent: None,
        embed: None,
        attachments: None,
    };

    let post_id = test.create_post(&shadow_user_id, &post).await?;

    let post_event_line = format!(
        "PUT pubky://{}/pub/pubky.app/posts/{}",
        shadow_user_id, post_id
    );

    let event = match Event::from_str(&post_event_line, test.client.clone()) {
        Ok(event) => event,
        Err(e) => {
            error!("Error while creating event line from line: {}", e);
            None
        }
    };
    
    if let Some(event) = event {
        match event.clone().handle().await {
            Ok(_) => info!("EVENT indexed!"),
            Err(e) => error!("Error while indexing event: {:?}", e),
        }
    }

    Ok(())
}
