use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::db::{fetch_key_from_graph, graph::Query};
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::UserIngestor;
use nexus_common::DEFAULT_MAX_FILE_SIZE;
use nexus_watcher::events::handlers::file::sync_put;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId, TimestampId};
use pubky_app_specs::{
    blob_uri_builder, file_uri_builder, PubkyAppBlob, PubkyAppFile, PubkyAppUser, PubkyId,
};

/// Creates a user, uploads a blob and returns the user's keypair and id with a
/// `PubkyAppFile` pointing at the blob.
async fn user_with_blob(test: &mut WatcherTest) -> Result<(Keypair, String, PubkyAppFile)> {
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Watcher:Light:Files:User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let blob = PubkyAppBlob::new("Hello light Nexus!".as_bytes().to_vec());
    let blob_id = blob.create_id();
    test.create_file_from_body(
        &user_kp,
        PubkyAppBlob::create_path(&blob_id).as_str(),
        blob.0.clone(),
    )
    .await?;

    let file = PubkyAppFile {
        name: "holiday.txt".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_uri_builder(user_id.clone(), blob_id),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };
    Ok((user_kp, user_id, file))
}

async fn graph_file_name(owner_id: &str, file_id: &str) -> Result<Option<String>> {
    let query = Query::new(
        "test_light_file_name",
        "MATCH (f:File {owner_id: $owner_id, id: $id}) RETURN f.name AS name",
    )
    .param("owner_id", owner_id)
    .param("id", file_id);
    Ok(fetch_key_from_graph(query, "name").await?)
}

/// A light Nexus keeps a slim file record (`src`, type, size) and nothing a person wrote
/// or uploaded: no name, no bytes on disk, no variant URLs.
#[tokio_shared_rt::test(shared)]
async fn test_light_file_keeps_a_slim_record_and_no_bytes() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;
    let (user_kp, user_id, file) = user_with_blob(&mut test).await?;

    let (file_id, file_path) = test.create_file(&user_kp, &file).await?;

    let files = FileDetails::get_by_ids(&[&[user_id.as_str(), file_id.as_str()]]).await?;
    let details = files[0].as_ref().expect("the file record is indexed");
    assert_eq!(details.src, file.src);
    assert_eq!(details.content_type, file.content_type);
    assert_eq!(details.size, file.size as i64);
    assert!(!details.blocked);
    assert_eq!(details.name, "", "the cached record holds no name");
    assert_eq!(
        graph_file_name(&user_id, &file_id).await?.as_deref(),
        Some(""),
        "the graph holds no name"
    );

    let json = serde_json::to_value(details)?;
    assert!(json.get("name").is_none(), "no name in responses: {json}");
    assert!(
        json.get("urls").is_none(),
        "no variant URLs in responses: {json}"
    );
    assert_eq!(json["src"], file.src);

    assert!(
        !test.temp_dir.path().join(&user_id).join(&file_id).exists(),
        "a light Nexus writes no file bytes"
    );

    test.cleanup_file(&user_kp, &file_path).await?;
    let files = FileDetails::get_by_ids(&[&[user_id.as_str(), file_id.as_str()]]).await?;
    assert!(files[0].is_none(), "deleting the file removes its record");

    test.cleanup_user(&user_kp).await?;
    Ok(())
}

/// Where a full Nexus refuses a file whose `src` is on a blacklisted homeserver, a light
/// one indexes the record flagged `blocked`, so clients know not to fetch it.
#[tokio_shared_rt::test(shared)]
async fn test_light_file_on_a_blacklisted_homeserver_is_flagged_blocked() -> Result<()> {
    let mut test = WatcherTest::setup_light(None).await?;
    let (user_kp, user_id, file) = user_with_blob(&mut test).await?;
    let file_id = file.create_id();

    // An ingestor blacklisting the homeserver that hosts the blob.
    let ingestor = UserIngestor::new([test.homeserver_id.clone()]);
    sync_put(
        file,
        file_uri_builder(user_id.clone(), file_id.clone()),
        PubkyId::from(user_kp.public_key()),
        file_id.clone(),
        test.temp_dir.path(),
        DEFAULT_MAX_FILE_SIZE,
        &ingestor,
    )
    .await?;

    let files = FileDetails::get_by_ids(&[&[user_id.as_str(), file_id.as_str()]]).await?;
    let details = files[0].as_ref().expect("the file record is indexed");
    assert!(details.blocked, "the record is flagged blocked");
    assert!(!test.temp_dir.path().join(&user_id).join(&file_id).exists());

    test.cleanup_user(&user_kp).await?;
    Ok(())
}
