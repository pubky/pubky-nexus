use crate::event_processor::users::utils::find_user_details;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::user::{UserDetails, UserSearch};
use pubky::Keypair;
use pubky_app_specs::{PubkyAppUser, PubkyAppUserLink};

fn profile(name: &str) -> PubkyAppUser {
    PubkyAppUser {
        name: name.to_string(),
        bio: Some("Watcher:Light:Users bio".to_string()),
        image: Some("pubky://example/pub/pubky.app/files/0034AVATAR000".to_string()),
        links: Some(vec![PubkyAppUserLink {
            title: "site".to_string(),
            url: "https://example.com".to_string(),
        }]),
        status: Some("around".to_string()),
    }
}

/// A light Nexus keeps a user's links and counts but nothing they wrote: no name, bio,
/// links or status, in the graph or the cache, and no name-search entry. The avatar is
/// a link, so it is kept, and the profile hash follows every edit.
#[tokio_shared_rt::test(shared)]
async fn test_light_profile_keeps_no_user_written_fields() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;

    let user_kp = Keypair::random();
    let user = profile("Watcher:Light:Users:Name");
    let user_id = test.create_user(&user_kp, &user).await?;

    let graph = find_user_details(&user_id).await?;
    assert_eq!(graph.name, "");
    assert_eq!(graph.bio, None);
    assert!(graph.links.is_none());
    assert_eq!(graph.status, None);
    assert_eq!(graph.image, user.image, "the avatar link is kept");
    // Hashed after spec validation, which normalises the link, so only its presence is
    // checked here; `users::profile_hash` checks the value.
    let first_hash = graph
        .profile_hash
        .clone()
        .expect("the profile hash is kept");

    let cached = UserDetails::try_from_index_json(&[user_id.as_str()], None)
        .await?
        .expect("the user is cached");
    let json = serde_json::to_value(&cached)?;
    for field in ["name", "bio", "links", "status"] {
        assert!(json.get(field).is_none(), "no {field}: {json}");
    }
    assert_eq!(json["image"], serde_json::json!(user.image));

    let by_name = UserSearch::get_by_name("watcher:light:users", None, None).await?;
    assert!(
        by_name.is_none_or(|found| !found.0.contains(&user_id)),
        "no name-search entry"
    );
    let by_id = UserSearch::get_by_id(&user_id, None, None).await?;
    assert!(
        by_id.is_some_and(|found| found.0.contains(&user_id)),
        "the id search still finds the user"
    );

    // An edit changes the hash.
    let edited = PubkyAppUser {
        status: Some("away".to_string()),
        ..user.clone()
    };
    test.create_profile(&user_kp, &edited).await?;
    let graph = find_user_details(&user_id).await?;
    let edited_hash = graph.profile_hash.expect("the profile hash is kept");
    assert_ne!(first_hash, edited_hash, "an edit changes the hash");

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
