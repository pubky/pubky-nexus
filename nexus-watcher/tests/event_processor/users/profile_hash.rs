use crate::event_processor::users::utils::find_user_details;
use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::db::RedisOps;
use nexus_common::models::user::UserDetails;
use pubky::Keypair;
use pubky_app_specs::PubkyAppUser;

/// A profile is stored with its hash in the graph and the cache, alongside the profile
/// itself, and an edit replaces the hash.
#[tokio_shared_rt::test(shared)]
async fn test_profile_stores_profile_hash() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        name: "Watcher:User:ProfileHash".to_string(),
        bio: Some("test_profile_stores_profile_hash".to_string()),
        image: None,
        links: None,
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let expected = UserDetails::hash_profile(&user);
    let graph = find_user_details(&user_id).await?;
    assert_eq!(graph.profile_hash.as_deref(), Some(expected.as_str()));
    assert_eq!(graph.name, user.name, "full mode keeps the name");
    let cached = UserDetails::try_from_index_json(&[user_id.as_str()], None)
        .await?
        .expect("the user is cached");
    assert_eq!(cached.profile_hash.as_deref(), Some(expected.as_str()));
    assert_eq!(cached.bio, user.bio);

    let edited = PubkyAppUser {
        status: Some("busy".to_string()),
        ..user.clone()
    };
    test.create_profile(&user_kp, &edited).await?;
    let graph = find_user_details(&user_id).await?;
    assert_eq!(
        graph.profile_hash.as_deref(),
        Some(UserDetails::hash_profile(&edited).as_str())
    );

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
