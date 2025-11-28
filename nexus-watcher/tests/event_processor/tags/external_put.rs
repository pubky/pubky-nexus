use crate::event_processor::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::link::ExternalLinkDetails;
use nexus_common::models::tag::external::TagExternal;
use nexus_common::models::user::UserCounts;
use nexus_common::types::Pagination;
use pubky::Keypair;
use pubky_app_specs::{tag_uri_builder, traits::HashId, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
#[ignore = "requires Redis and Pubky test stack"]
async fn test_put_external_link_tag() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    let keypair = Keypair::random();
    let tagger = PubkyAppUser {
        bio: Some("test_put_external_link_tag".to_string()),
        image: None,
        links: None,
        name: "Watcher:ExternalTag".to_string(),
        status: None,
    };
    let tagger_user_id = test.create_user(&keypair, &tagger).await?;

    let external_url = "https://Example.com/path/to/article?param=1#section";
    let tag = PubkyAppTag {
        uri: external_url.to_string(),
        label: "external_link".to_string(),
        created_at: Utc::now().timestamp_millis(),
    };
    let tag_url = tag_uri_builder(tagger_user_id.clone(), tag.create_id());
    let expected_link = ExternalLinkDetails::from_url(external_url, tag.created_at)?;

    test.put(&tag_url, tag).await?;

    let stored_link = ExternalLinkDetails::get(&expected_link.id)
        .await
        .unwrap()
        .expect("external link details should be stored");
    assert_eq!(stored_link.normalized_url, expected_link.normalized_url);
    assert_eq!(stored_link.scheme, expected_link.scheme);

    let tag_details = TagExternal::get_by_id(&expected_link.id, None, None, None, None)
        .await
        .unwrap()
        .expect("tag details should be available");
    assert_eq!(tag_details.len(), 1);
    assert_eq!(tag_details[0].label, "external_link");
    assert_eq!(tag_details[0].taggers_count, 1);
    assert_eq!(tag_details[0].taggers[0], tagger_user_id);

    let taggers = TagExternal::get_taggers_by_id(
        &expected_link.id,
        "external_link",
        Pagination::default(),
        None,
    )
    .await
    .unwrap()
    .expect("taggers should exist");
    assert_eq!(taggers.0.len(), 1);
    assert_eq!(taggers.0[0], tagger_user_id);

    let counts = UserCounts::get_by_id(&tagger_user_id)
        .await
        .unwrap()
        .expect("user counts should exist");
    assert_eq!(counts.tagged, 1);

    Ok(())
}
