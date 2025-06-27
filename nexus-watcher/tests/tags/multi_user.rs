use crate::tags::utils::find_user_tag;
use crate::users::utils::find_user_counts;
use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use chrono::Utc;
use nexus_common::models::tag::user::TagUser;
use nexus_common::{
    models::{
        notification::Notification,
        tag::traits::{TagCollection, TaggersCollection},
    },
    types::Pagination,
};
use pubky::Keypair;
use pubky_app_specs::{traits::HashId, PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_multi_user_tags() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Write in the homeserver and index in nexus
    let mut user_ids = Vec::with_capacity(4);
    // Create 5 users
    for index in 0..4 {
        let keypair = Keypair::random();

        let tagger = PubkyAppUser {
            bio: Some("test_homeserver_multi_user".to_string()),
            image: None,
            links: None,
            name: format!("Watcher:MultiUser:User{index}"),
            status: None,
        };
        let user_id = test.create_user(&keypair, &tagger).await?;
        user_ids.push(user_id);
    }
    let tagged_id = &user_ids[0];

    let tagger_a_id = &user_ids[1];
    let tagger_b_id = &user_ids[2];
    let tagger_c_id = &user_ids[3];

    let label_wind = "wind";
    let label_earth = "earth";

    // Step 2: Create tags
    let mut tag_urls = Vec::with_capacity(5);
    let wind_taggers = [tagger_a_id, tagger_b_id, tagger_c_id];

    for tagger_id in wind_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{tagged_id}/pub/pubky.app/profile.json"),
            label: label_wind.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_url = format!(
            "pubky://{}/pub/pubky.app/tags/{}",
            tagger_id,
            tag.create_id()
        );
        // Put tag
        test.put(&tag_url, tag).await?;
        tag_urls.push(tag_url)
    }

    let earth_taggers = [tagger_b_id, tagger_c_id];

    for tagger_id in earth_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{tagged_id}/pub/pubky.app/profile.json"),
            label: label_earth.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_url = format!(
            "pubky://{}/pub/pubky.app/tags/{}",
            tagger_id,
            tag.create_id()
        );
        // Put tag
        test.put(&tag_url, tag).await?;
        tag_urls.push(tag_url)
    }

    // Step 3: Assert all the PUT operations
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_water_tag = find_user_tag(tagged_id, label_wind)
        .await
        .unwrap()
        .expect("Failed to find user tag in graph database");

    assert_eq!(post_water_tag.label, label_wind);
    assert_eq!(post_water_tag.taggers_count, 3);
    assert!(post_water_tag.taggers.contains(tagger_a_id));
    assert!(post_water_tag.taggers.contains(tagger_b_id));
    assert!(post_water_tag.taggers.contains(tagger_c_id));

    let post_fire_tag = find_user_tag(tagged_id, label_earth)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_fire_tag.label, label_earth);
    assert_eq!(post_fire_tag.taggers_count, 2);
    assert!(post_fire_tag.taggers.contains(tagger_b_id));
    assert!(post_fire_tag.taggers.contains(tagger_c_id));

    // CACHE_OP: Check if the tag is correctly cached
    let cache_user_tag =
        <TagUser as TagCollection>::get_from_index(tagged_id, None, None, None, None, None, false)
            .await
            .unwrap();

    assert!(cache_user_tag.is_some());
    let cache_tag_details = cache_user_tag.unwrap();
    assert_eq!(cache_tag_details.len(), 2);

    // TagUser related
    assert_eq!(cache_tag_details[0].label, label_wind);
    assert_eq!(cache_tag_details[1].label, label_earth);
    // Count user tag taggers: Sorted:Users:Tag:user_id
    assert_eq!(cache_tag_details[0].taggers_count, 3);
    assert_eq!(cache_tag_details[1].taggers_count, 2);
    // Find user as tagger in the post: User:Taggers:user_id
    assert!(cache_tag_details[0].taggers.contains(tagger_a_id));
    assert!(cache_tag_details[0].taggers.contains(tagger_b_id));
    assert!(cache_tag_details[0].taggers.contains(tagger_c_id));
    assert!(cache_tag_details[1].taggers.contains(tagger_b_id));
    assert!(cache_tag_details[1].taggers.contains(tagger_c_id));

    // Check if user counts updated: User:Counts:user_id
    let tagger_a_user_counts = find_user_counts(tagged_id).await;
    assert_eq!(tagger_a_user_counts.tags, 5);
    assert_eq!(tagger_a_user_counts.unique_tags, 2);
    let tagger_a_user_counts = find_user_counts(tagger_a_id).await;
    assert_eq!(tagger_a_user_counts.tagged, 1);
    let tagger_b_user_counts = find_user_counts(tagger_b_id).await;
    assert_eq!(tagger_b_user_counts.tagged, 2);
    let tagger_c_user_counts = find_user_counts(tagger_c_id).await;
    assert_eq!(tagger_c_user_counts.tagged, 2);

    // Step 4: DEL tag from homeserver
    for tag_url in tag_urls {
        test.del(&tag_url).await?;
    }

    // Step 5: Assert all the DEL operations
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_tag = find_user_tag(tagged_id, label_wind).await.unwrap();
    assert!(post_tag.is_none());

    let post_tag = find_user_tag(tagged_id, label_earth).await.unwrap();
    assert!(post_tag.is_none());

    // CACHE_OP: Check if the tag is correctly deleted from cache
    // Sorted:Users:Tag:author_id
    let cache_post_tag =
        <TagUser as TagCollection>::get_from_index(tagged_id, None, None, None, None, None, false)
            .await
            .expect("Failed to get tag from cache");

    assert!(
        cache_post_tag.unwrap().is_empty(),
        "The SORTED SET index cannot exist for the tag"
    );

    // User:Taggers:author_id:label
    let wind_label_key = vec![tagged_id.as_str(), label_wind];
    let wind_tag_collection =
        <TagUser as TaggersCollection>::get_from_index(wind_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(wind_tag_collection.is_none());

    let earth_label_key = vec![tagged_id.as_str(), label_earth];
    let earth_tag_collection =
        <TagUser as TaggersCollection>::get_from_index(earth_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(earth_tag_collection.is_none());

    // Check if user counts updated: User:Counts:user_id:post_id
    let user_counts = find_user_counts(tagged_id).await;
    assert_eq!(user_counts.tags, 0);
    assert_eq!(user_counts.unique_tags, 0);

    // Check if user counts updated: User:Counts:user_id
    for tagger_id in wind_taggers {
        let user_counts = find_user_counts(tagger_id).await;
        assert_eq!(user_counts.tagged, 0);
    }

    for tagger_id in earth_taggers {
        let user_counts = find_user_counts(tagger_id).await;
        assert_eq!(user_counts.tagged, 0);
    }

    let notifications = Notification::get_by_id(tagged_id, Pagination::default())
        .await
        .unwrap();
    assert_eq!(
        notifications.len(),
        5,
        "Taggerd user should have 5 notification"
    );

    Ok(())
}
