use super::resource_utils::compute_resource_id;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::resource::view::ResourceView;
use nexus_common::models::tag::TagDetails;
use pubky::Keypair;
use pubky::ResourcePath;
use pubky_social_specs::legacy_v0::traits::HashId;
use pubky_social_specs::legacy_v0::{PubkyAppTag, PubkyAppUser};

const APP: &str = "mapky";

/// A resource's tag list follows its puts and dels: each put adds its tagger to
/// the label, each del removes only its own tagger or label, and the last del
/// takes the resource with it.
#[tokio_shared_rt::test(shared)]
async fn test_resource_tag_list_follows_puts_and_dels() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let (alice_kp, alice_id) = create_user(&mut test, "Alice").await?;
    let (bob_kp, bob_id) = create_user(&mut test, "Bob").await?;

    // A fresh user id in the URI keeps the resource unique to this run
    let target_uri = format!("https://example.com/tag-list/{alice_id}");
    let resource_id = compute_resource_id(&target_uri);

    let alice_rust = put_resource_tag(&mut test, &alice_kp, &target_uri, "rust").await?;
    let bob_rust = put_resource_tag(&mut test, &bob_kp, &target_uri, "rust").await?;
    let alice_nostr = put_resource_tag(&mut test, &alice_kp, &target_uri, "nostr").await?;

    let mut rust_taggers = vec![alice_id.as_str(), bob_id.as_str()];
    rust_taggers.sort_unstable();
    let tags = tag_list(&resource_id).await?;
    assert_eq!(taggers(&tags, "rust"), Some(rust_taggers));
    assert_eq!(taggers(&tags, "nostr"), Some(vec![alice_id.as_str()]));

    test.del(&bob_kp, &bob_rust).await?;
    let tags = tag_list(&resource_id).await?;
    assert_eq!(taggers(&tags, "rust"), Some(vec![alice_id.as_str()]));
    assert_eq!(taggers(&tags, "nostr"), Some(vec![alice_id.as_str()]));

    test.del(&alice_kp, &alice_nostr).await?;
    let tags = tag_list(&resource_id).await?;
    assert_eq!(taggers(&tags, "rust"), Some(vec![alice_id.as_str()]));
    assert_eq!(taggers(&tags, "nostr"), None);

    // The last tag takes the resource with it
    test.del(&alice_kp, &alice_rust).await?;
    let view = ResourceView::get_by_id(&resource_id, None, None, None, None).await?;
    assert!(view.is_none(), "The untagged resource should be gone");

    for user_kp in [&alice_kp, &bob_kp] {
        test.cleanup_user(user_kp).await?;
    }

    Ok(())
}

async fn create_user(test: &mut WatcherTest, name: &str) -> Result<(Keypair, String)> {
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: Some("test_resource_tag_list".to_string()),
        image: None,
        links: None,
        name: format!("Watcher:ResourceTagList:{name}"),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;
    Ok((user_kp, user_id))
}

/// PUTs a `label` tag on `uri` under the app's tag path, so the watcher indexes
/// it as a Resource tag
async fn put_resource_tag(
    test: &mut WatcherTest,
    user_kp: &Keypair,
    uri: &str,
    label: &str,
) -> Result<ResourcePath> {
    let tag = PubkyAppTag {
        uri: uri.to_string(),
        label: label.to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_path: ResourcePath = format!("/pub/{APP}/tags/{}", tag.create_id()).parse()?;
    test.put(user_kp, &tag_path, &tag).await?;
    Ok(tag_path)
}

async fn tag_list(resource_id: &str) -> Result<Vec<TagDetails>> {
    ResourceView::get_by_id(resource_id, None, None, None, None)
        .await?
        .map(|view| view.tags)
        .ok_or_else(|| anyhow::anyhow!("Resource {resource_id} should have a tag list"))
}

/// The taggers listed for `label`, sorted, or `None` when the list lacks it
fn taggers<'a>(tags: &'a [TagDetails], label: &str) -> Option<Vec<&'a str>> {
    tags.iter().find(|tag| tag.label == label).map(|tag| {
        let mut taggers: Vec<&str> = tag.taggers.iter().map(String::as_str).collect();
        taggers.sort_unstable();
        taggers
    })
}
