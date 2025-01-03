use crate::watcher::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use pubky_app_specs::{traits::HashId, PubkyAppPost, PubkyAppTag, PubkyAppUser};
use pubky_common::crypto::Keypair;

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_tag_user_not_found() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_homeserver_mute_cannot_complete".to_string()),
        image: None,
        links: None,
        name: "Watcher:Tag:User:Sync".to_string(),
        status: None,
    };
    let tagged_id = test.create_user(&keypair, &user).await?;

    // Create a key but it would not be synchronised in nexus
    let shadow_keypair = Keypair::random();
    let shadow_tagger_id = shadow_keypair.public_key().to_z32();

    // In that case, that user will act as a NotSyncUser or user not registered in pubky.app
    // It will not have a profile.json
    test.register_user(&shadow_keypair).await?;
    
    // => Create user tag
    let label = "friendly";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", tagged_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", shadow_tagger_id, tag.create_id());

    // PUT user tag
    test.put(tag_url.as_str(), tag_blob).await?;
    test.del(&tag_url).await?;

    // => Now create the tag in the opposite direction
    let label = "friendly_opposite";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/profile.json", shadow_tagger_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };

    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", tagged_id, tag.create_id());

    // PUT user tag
    test.put(tag_url.as_str(), tag_blob).await?;
    test.del(&tag_url).await?;

    // => Create post tag
    let post = PubkyAppPost {
        content: "Watcher:Tag:User:Sync:Post".to_string(),
        kind: PubkyAppPost::default().kind,
        parent: None,
        embed: None,
        attachments: None,
    };
    let post_id = test.create_post(&tagged_id, &post).await?;
    
    let label = "merkle_tree";

    let tag = PubkyAppTag {
        uri: format!("pubky://{}/pub/pubky.app/posts/{}", tagged_id, post_id),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_blob = serde_json::to_vec(&tag)?;
    let tag_url = format!(
        "pubky://{}/pub/pubky.app/tags/{}",
        shadow_tagger_id,
        tag.create_id()
    );

    // PUT post tag
    test.put(&tag_url, tag_blob).await?;
    test.del(&tag_url).await?;

    Ok(())
}
