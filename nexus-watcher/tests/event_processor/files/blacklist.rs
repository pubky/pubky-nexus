use crate::event_processor::utils::watcher::{assert_file_details, WatcherTest};
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::file::FileDetails;
use nexus_common::models::traits::Collection;
use nexus_common::models::user::UserIngestor;
use nexus_common::utils::test_utils::random_pubky_id;
use nexus_watcher::events::handlers::file::sync_put;
use nexus_watcher::EventProcessorError;
use pubky::Keypair;
use pubky_app_specs::traits::{HasIdPath, HashId, TimestampId};
use pubky_app_specs::{
    blob_uri_builder, file_uri_builder, PubkyAppBlob, PubkyAppFile, PubkyAppUser, PubkyId,
};

/// Creates a user on the test homeserver, uploads a blob and returns the
/// `PubkyAppFile` pointing at it, along with the ids needed for `sync_put`.
async fn setup_user_with_blob(
    test: &mut WatcherTest,
) -> Result<(PubkyId, PubkyAppFile, String, String)> {
    let user_kp = Keypair::random();
    let user = PubkyAppUser {
        bio: None,
        image: None,
        links: None,
        name: "Test User".to_string(),
        status: None,
    };
    let user_id = test.create_user(&user_kp, &user).await?;

    let blob = PubkyAppBlob::new("Hello World!".as_bytes().to_vec());
    let blob_id = blob.create_id();
    let blob_relative_url = PubkyAppBlob::create_path(&blob_id);
    let blob_absolute_url = blob_uri_builder(user_id.clone(), blob_id);

    test.create_file_from_body(&user_kp, blob_relative_url.as_str(), blob.0.clone())
        .await?;

    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src: blob_absolute_url.clone(),
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };

    let user_pubky_id = PubkyId::from(user_kp.public_key());
    Ok((user_pubky_id, file, user_id, blob_absolute_url))
}

/// A file whose `src` blob is hosted on a blacklisted HS must not be ingested:
/// `sync_put` returns [`EventProcessorError::HsBlacklisted`], the blob is not
/// written to disk and no `FileDetails` is indexed.
#[tokio_shared_rt::test(shared)]
async fn test_file_ingest_aborts_on_blacklisted_source_homeserver() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let (user_pubky_id, file, user_id, _) = setup_user_with_blob(&mut test).await?;
    let file_id = file.create_id();
    let file_uri = file_uri_builder(user_id.clone(), file_id.clone());

    // Ingestor blacklisting the HS that hosts the blob source (the test homeserver)
    let ingestor = UserIngestor::new([test.homeserver_id.clone()]);

    let err = sync_put(
        file,
        file_uri,
        user_pubky_id,
        file_id.clone(),
        test.temp_dir.path().to_path_buf(),
        &ingestor,
    )
    .await
    .expect_err("file ingest should be refused for a blacklisted source HS");
    assert!(
        matches!(&err, EventProcessorError::HsBlacklisted { hs_id } if *hs_id == *test.homeserver_id),
        "expected HsBlacklisted for {}, got {err:?}",
        test.homeserver_id
    );

    let files = FileDetails::get_by_ids(&[&[user_id.as_str(), file_id.as_str()]]).await?;
    assert!(
        files[0].is_none(),
        "file from blacklisted source HS must not be indexed"
    );
    assert!(
        !test.temp_dir.path().join(&user_id).join(&file_id).exists(),
        "blob from blacklisted source HS must not be written to disk"
    );

    Ok(())
}

/// A file whose `src` addresses a blacklisted HS PK *directly*
/// (`pubky://<blacklisted_hs_pk>/...`, rather than a user hosted on it) must also
/// be refused. An HS PK publishes no homeserver record, so `get_homeserver_of`
/// returns `None`; here it is the blacklist self-check — not the DHT lookup —
/// that blocks ingestion, before any request reaches the HS.
#[tokio_shared_rt::test(shared)]
async fn test_file_ingest_aborts_when_source_is_blacklisted_hs_pk_directly() -> Result<()> {
    let test = WatcherTest::setup().await?;

    // `src` blob is addressed to the HS PK itself, not to a user hosted on it.
    let blob = PubkyAppBlob::new("Hello World!".as_bytes().to_vec());
    let blob_id = blob.create_id();
    let src = blob_uri_builder(test.homeserver_id.to_string(), blob_id);

    let file = PubkyAppFile {
        name: "myfile".to_string(),
        content_type: "text/plain".to_string(),
        src,
        size: blob.0.len(),
        created_at: Utc::now().timestamp_millis(),
    };

    // The file owner is an unrelated, non-blacklisted user.
    let owner_id = random_pubky_id();
    let file_id = file.create_id();
    let file_uri = file_uri_builder(owner_id.to_string(), file_id.clone());

    // Ingestor blacklisting the HS PK used as the direct blob source.
    let ingestor = UserIngestor::new([test.homeserver_id.clone()]);

    let err = sync_put(
        file,
        file_uri,
        owner_id.clone(),
        file_id.clone(),
        test.temp_dir.path().to_path_buf(),
        &ingestor,
    )
    .await
    .expect_err("file ingest should be refused when src addresses a blacklisted HS PK directly");
    assert!(
        matches!(&err, EventProcessorError::HsBlacklisted { hs_id } if *hs_id == *test.homeserver_id),
        "expected HsBlacklisted for {}, got {err:?}",
        test.homeserver_id
    );

    let files = FileDetails::get_by_ids(&[&[owner_id.as_ref(), file_id.as_str()]]).await?;
    assert!(
        files[0].is_none(),
        "file with a blacklisted HS PK as direct source must not be indexed"
    );
    assert!(
        !test
            .temp_dir
            .path()
            .join(owner_id.as_ref())
            .join(&file_id)
            .exists(),
        "blob from a blacklisted HS PK source must not be written to disk"
    );

    Ok(())
}

/// Control: a blacklist that does not contain the source HS leaves file
/// ingestion untouched, proving the blacklist match (not some other failure)
/// is what blocked ingestion above.
#[tokio_shared_rt::test(shared)]
async fn test_file_ingest_proceeds_when_source_homeserver_not_blacklisted() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let (user_pubky_id, file, user_id, blob_absolute_url) = setup_user_with_blob(&mut test).await?;
    let file_id = file.create_id();
    let file_uri = file_uri_builder(user_id.clone(), file_id.clone());

    // Ingestor blacklisting an unrelated HS; the source HS is not in the list
    let ingestor = UserIngestor::new([random_pubky_id()]);

    sync_put(
        file.clone(),
        file_uri,
        user_pubky_id,
        file_id.clone(),
        test.temp_dir.path().to_path_buf(),
        &ingestor,
    )
    .await
    .expect("file ingest should proceed when the source HS is not blacklisted");

    let result_file = assert_file_details(&user_id, &file_id, &blob_absolute_url, &file).await;
    assert!(
        test.temp_dir.path().join(&result_file.urls.main).exists(),
        "blob should be written to disk after successful ingest"
    );

    Ok(())
}
