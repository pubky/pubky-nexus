use super::utils::find_user_tag;
use crate::event_processor::users::utils::find_user_counts;
use crate::event_processor::utils::watcher::{HomeserverHashIdPath, WatcherTest};
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
use pubky_app_specs::{PubkyAppTag, PubkyAppUser};

#[tokio_shared_rt::test(shared)]
async fn test_homeserver_multi_user_tags() -> Result<()> {
    let mut test = WatcherTest::setup().await?;

    // Step 1: Write in the homeserver and index in nexus
    let mut user_ids_and_kps = Vec::with_capacity(4);
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
        user_ids_and_kps.push((user_id, keypair));
    }
    let (tagged_id, _tagged_kp) = &user_ids_and_kps[0];

    let tagger_a_id_kp = &user_ids_and_kps[1];
    let tagger_b_id_kp = &user_ids_and_kps[2];
    let tagger_c_id_kp = &user_ids_and_kps[3];

    let label_wind = "wind";
    let label_earth = "earth";

    // Step 2: Create tags
    let mut tag_paths_and_tagger_kps = Vec::with_capacity(5);
    let wind_taggers = [tagger_a_id_kp, tagger_b_id_kp, tagger_c_id_kp];

    for (_tagger_id, tagger_kp) in wind_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{tagged_id}/pub/pubky.app/profile.json"),
            label: label_wind.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_path = tag.hs_path();
        // Put tag
        test.put(tagger_kp, &tag_path, tag).await?;
        tag_paths_and_tagger_kps.push((tag_path, tagger_kp))
    }

    let earth_taggers = [tagger_b_id_kp, tagger_c_id_kp];

    for (_tagger_id, tagger_kp) in earth_taggers {
        let tag = PubkyAppTag {
            uri: format!("pubky://{tagged_id}/pub/pubky.app/profile.json"),
            label: label_earth.to_string(),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_path = tag.hs_path();
        // Put tag
        test.put(tagger_kp, &tag_path, tag).await?;
        tag_paths_and_tagger_kps.push((tag_path, tagger_kp))
    }

    // Step 3: Assert all the PUT operations
    // GRAPH_OP: Check if the tag exists in the graph database
    let post_water_tag = find_user_tag(tagged_id, label_wind)
        .await
        .unwrap()
        .expect("Failed to find user tag in graph database");

    assert_eq!(post_water_tag.label, label_wind);
    assert_eq!(post_water_tag.taggers_count, 3);
    assert!(post_water_tag.taggers.contains(&tagger_a_id_kp.0));
    assert!(post_water_tag.taggers.contains(&tagger_b_id_kp.0));
    assert!(post_water_tag.taggers.contains(&tagger_c_id_kp.0));

    let post_fire_tag = find_user_tag(tagged_id, label_earth)
        .await
        .unwrap()
        .expect("Failed to find post tag in graph database");

    assert_eq!(post_fire_tag.label, label_earth);
    assert_eq!(post_fire_tag.taggers_count, 2);
    assert!(post_fire_tag.taggers.contains(&tagger_b_id_kp.0));
    assert!(post_fire_tag.taggers.contains(&tagger_c_id_kp.0));

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
    assert!(cache_tag_details[0].taggers.contains(&tagger_a_id_kp.0));
    assert!(cache_tag_details[0].taggers.contains(&tagger_b_id_kp.0));
    assert!(cache_tag_details[0].taggers.contains(&tagger_c_id_kp.0));
    assert!(cache_tag_details[1].taggers.contains(&tagger_b_id_kp.0));
    assert!(cache_tag_details[1].taggers.contains(&tagger_c_id_kp.0));

    // Check if user counts updated: User:Counts:user_id
    let tagger_a_user_counts = find_user_counts(tagged_id).await;
    assert_eq!(tagger_a_user_counts.tags, 5);
    assert_eq!(tagger_a_user_counts.unique_tags, 2);
    let tagger_a_user_counts = find_user_counts(&tagger_a_id_kp.0).await;
    assert_eq!(tagger_a_user_counts.tagged, 1);
    let tagger_b_user_counts = find_user_counts(&tagger_b_id_kp.0).await;
    assert_eq!(tagger_b_user_counts.tagged, 2);
    let tagger_c_user_counts = find_user_counts(&tagger_c_id_kp.0).await;
    assert_eq!(tagger_c_user_counts.tagged, 2);

    // Step 4: DEL tag from homeserver
    for (tag_url, tagger_kp) in tag_paths_and_tagger_kps {
        test.del(tagger_kp, &tag_url).await?;
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
    let (wind_taggers_result, _) =
        <TagUser as TaggersCollection>::get_from_index(wind_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(wind_taggers_result.is_empty());

    let earth_label_key = vec![tagged_id.as_str(), label_earth];
    let (earth_taggers_result, _) =
        <TagUser as TaggersCollection>::get_from_index(earth_label_key, None, None, None, None)
            .await
            .unwrap();
    assert!(earth_taggers_result.is_empty());

    // Check if user counts updated: User:Counts:user_id:post_id
    let user_counts = find_user_counts(tagged_id).await;
    assert_eq!(user_counts.tags, 0);
    assert_eq!(user_counts.unique_tags, 0);

    // Check if user counts updated: User:Counts:user_id
    for (tagger_id, _) in wind_taggers {
        let user_counts = find_user_counts(tagger_id).await;
        assert_eq!(user_counts.tagged, 0);
    }

    for (tagger_id, _) in earth_taggers {
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
