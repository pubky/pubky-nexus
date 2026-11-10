// File: ./tests/watcher/network/large_network_test.rs

use crate::event_processor::utils::watcher::{HomeserverHashIdPath, HomeserverIdPath, WatcherTest};
use anyhow::{anyhow, Result};
use nexus_common::models::user::UserCounts;
use nexus_watcher::service::TEventProcessorRunner;
use pubky::Keypair;
use pubky_social_specs::legacy_v0::{
    post_uri_builder, PubkyAppBookmark, PubkyAppFollow, PubkyAppPost, PubkyAppPostKind,
    PubkyAppTag, PubkyAppUser,
};
use rand::{rngs::StdRng, RngExt, SeedableRng};
use std::collections::{HashMap, HashSet};
use tracing::info;

// We can test two modalities:
// 1. Processing the events one by one. This is akin to a watcher that is always fully synced.
// 2. Processing all events at the end. This is akin to syncing an existing homeserver.
const PROCESS_EVENTS_ONE_BY_ONE: bool = true;

// Size of network
const NUM_USERS: usize = 10;

#[tokio_shared_rt::test(shared)]
async fn test_large_network_scenario_counts() -> Result<()> {
    let mut test = WatcherTest::setup(None).await?;

    if !PROCESS_EVENTS_ONE_BY_ONE {
        test = test.remove_event_processing().await;
    }

    // Seed for reproducibility
    let seed = 42;
    let mut rng = StdRng::seed_from_u64(seed);

    // Parameters for network size
    let max_posts_per_user = 10;
    let max_tags_per_user = 15;
    let max_follows_per_user = 20;
    let max_bookmarks_per_user = 15;

    // Containers to hold user data
    let mut user_kps_and_ids = Vec::new();
    let mut user_names = Vec::new();
    let mut user_posts: HashMap<String, Vec<String>> = HashMap::new(); // Map user_id to post IDs

    // Create users
    for i in 0..NUM_USERS {
        let keypair = Keypair::random();
        let user_name = format!("User{i}");
        let user = PubkyAppUser {
            bio: Some(format!("{user_name}'s bio")),
            image: None,
            links: None,
            name: user_name.clone(),
            status: None,
        };
        let user_id = test.create_user(&keypair, &user).await?;
        user_kps_and_ids.push((keypair, user_id.clone()));
        user_names.push(user_name);
        user_posts.insert(user_id, Vec::new()); // Initialize posts vector for this user
    }

    // Total event counters
    let mut total_posts = 0;
    let mut total_tags = 0;
    let mut total_bookmarks = 0;
    let mut total_follows = 0;
    let mut total_unfollows = 0;
    let mut total_tag_deletions = 0;

    // Users create posts
    for (i, (user_kp, user_id)) in user_kps_and_ids.iter().enumerate() {
        let num_posts = rng.random_range(1..=max_posts_per_user);
        for _ in 0..num_posts {
            let post = PubkyAppPost {
                content: format!("{}'s post content", user_names[i]),
                kind: PubkyAppPostKind::Short,
                parent: None,
                embed: None,
                attachments: None,
                lock: None,
            };
            let (post_id, _post_path) = test.create_post(user_kp, &post).await?;
            user_posts.get_mut(user_id).unwrap().push(post_id);
            total_posts += 1;
        }
    }

    // Users follow other users
    let mut user_followings: HashMap<String, HashSet<String>> = HashMap::new(); // Map user_id to set of following user_ids

    for (_user_kp, user_id) in user_kps_and_ids.iter() {
        user_followings.insert(user_id.clone(), HashSet::new());
    }

    for (i, (user_kp, user_id)) in user_kps_and_ids.iter().enumerate() {
        let num_follows = rng.random_range(1..=max_follows_per_user.min(NUM_USERS - 1));
        let follow_set = &mut user_followings.get_mut(user_id).unwrap();
        while follow_set.len() < num_follows {
            let target_index = rng.random_range(0..NUM_USERS);
            if target_index != i {
                let (_target_user_kp, target_user_id) = &user_kps_and_ids[target_index];
                if follow_set.insert(target_user_id.clone()) {
                    test.create_follow(user_kp, target_user_id).await?;
                    total_follows += 1;
                }
            }
        }
    }

    // Users bookmark posts. A repeated bookmark reuses its hashed path, so a user's
    // bookmarks are the distinct posts they bookmarked.
    let mut user_bookmarks: HashMap<String, HashSet<String>> = HashMap::new(); // Map user_id to bookmarked post URIs

    for (user_kp, user_id) in user_kps_and_ids.iter() {
        let num_bookmarks = rng.random_range(1..=max_bookmarks_per_user);
        let bookmark_set = user_bookmarks.entry(user_id.clone()).or_default();
        for _ in 0..num_bookmarks {
            let target_user_index = rng.random_range(0..NUM_USERS);
            let (_target_user_kp, target_user_id) = &user_kps_and_ids[target_user_index];
            if !user_posts[&target_user_id.clone()].is_empty() {
                let post_index = rng.random_range(0..user_posts[&target_user_id.clone()].len());
                let target_post_id = &user_posts[&target_user_id.clone()][post_index];

                let bookmark = PubkyAppBookmark {
                    uri: post_uri_builder(target_user_id.into(), target_post_id.into()),
                    created_at: chrono::Utc::now().timestamp_millis(),
                };

                let bookmark_path = bookmark.hs_path();

                test.put(user_kp, &bookmark_path, &bookmark).await?;
                bookmark_set.insert(bookmark.uri);
                total_bookmarks += 1;
            }
        }
    }

    // Users tag posts of other users. A repeated tag reuses its hashed path, so a
    // user's tags are the distinct (post, label) pairs they have not deleted.
    let mut user_tagged: HashMap<String, HashSet<(String, String)>> = HashMap::new(); // Map user_id to tagged (post URI, label) pairs

    for (user_kp, user_id) in user_kps_and_ids.iter() {
        let num_tags = rng.random_range(1..=max_tags_per_user);
        let tag_set = user_tagged.entry(user_id.clone()).or_default();
        for _ in 0..num_tags {
            let target_user_index = rng.random_range(0..NUM_USERS);
            let (_target_user_kp, target_user_id) = &user_kps_and_ids[target_user_index];
            if !user_posts[&target_user_id.clone()].is_empty() {
                let post_index = rng.random_range(0..user_posts[&target_user_id.clone()].len());
                let target_post_id = &user_posts[&target_user_id.clone()][post_index];

                let tag_label = format!("tag{}", rng.random_range(0..100));
                let tag = PubkyAppTag {
                    uri: post_uri_builder(target_user_id.into(), target_post_id.into()),
                    label: tag_label.clone(),
                    created_at: chrono::Utc::now().timestamp_millis(),
                };
                let tag_path = tag.hs_path();

                test.put(user_kp, &tag_path, &tag).await?;
                let post_label = (tag.uri, tag_label);
                tag_set.insert(post_label.clone());
                total_tags += 1;

                // Randomly decide to delete the tag
                if rng.random_bool(0.1) {
                    // 10% chance to delete the tag
                    test.del(user_kp, &tag_path).await?;
                    tag_set.remove(&post_label);
                    total_tag_deletions += 1;
                }
            }
        }
    }

    // Users unfollow other users
    for (user_kp, user_id) in user_kps_and_ids.iter() {
        // Get list of users this user is following
        let following_set = &mut user_followings.get_mut(user_id).unwrap();
        let following: Vec<String> = following_set.iter().cloned().collect();

        let num_unfollows = rng.random_range(0..=following.len());
        let mut unfollowed = HashSet::new();

        for _ in 0..num_unfollows {
            if following.is_empty() || unfollowed.len() == following.len() {
                break;
            }

            let target_index = rng.random_range(0..following.len());
            let target_user_id = &following[target_index];
            if unfollowed.insert(target_user_id.clone()) {
                let follow_path = PubkyAppFollow::hs_path(target_user_id);
                test.del(user_kp, &follow_path).await?;
                following_set.remove(target_user_id);
                total_unfollows += 1;
            }
        }
    }

    if !PROCESS_EVENTS_ONE_BY_ONE {
        for _ in 1..=100 {
            // Run the event processor
            // We do this because earlier, the runner's event processing has been turned off temporarily
            // but at this point we are ready to run the event processing
            test.event_processor_runner
                .build(test.homeserver_id.as_ref())
                .await
                .map_err(|e| anyhow!(e))?
                .run()
                .await
                .map_err(|e| anyhow!(e))?;
        }
    }

    // Each user's counts must match the posts, follows, bookmarks and tags this test sent.
    let mut total_posts_counted = 0;
    let mut total_tags_counted = 0;
    let mut total_bookmarks_counted = 0;
    let mut total_following_counted = 0;
    let mut total_followers_counted = 0;

    for (_user_kp, user_id) in user_kps_and_ids.iter() {
        let counts = UserCounts::get_by_id(user_id)
            .await
            .unwrap()
            .expect("Counts not found in graph");
        let followers = user_followings
            .values()
            .filter(|following| following.contains(user_id))
            .count();

        assert_eq!(
            counts.posts as usize,
            user_posts[user_id].len(),
            "posts of {user_id}"
        );
        assert_eq!(
            counts.following as usize,
            user_followings[user_id].len(),
            "following of {user_id}"
        );
        assert_eq!(
            counts.followers as usize, followers,
            "followers of {user_id}"
        );
        assert_eq!(
            counts.bookmarks as usize,
            user_bookmarks[user_id].len(),
            "bookmarks of {user_id}"
        );
        assert_eq!(
            counts.tagged as usize,
            user_tagged[user_id].len(),
            "tagged of {user_id}"
        );

        total_posts_counted += counts.posts as usize;
        total_tags_counted += counts.tagged as usize;
        total_following_counted += counts.following as usize;
        total_followers_counted += counts.followers as usize;
        total_bookmarks_counted += counts.bookmarks as usize;
    }

    info!("Total posts created: {}", total_posts);
    info!("Total posts counted in the graph: {}", total_posts_counted);
    info!(
        "Total tags created (excluding deletions): {}",
        total_tags - total_tag_deletions
    );
    info!("Total tags counted in the graph: {}", total_tags_counted);

    info!("Total bookmarks created: {}", total_bookmarks);
    info!("Total bookmarks counted in the graph: {total_bookmarks_counted}",);

    info!("Total follows created: {}", total_follows);
    info!("Total unfollows performed: {}", total_unfollows);
    let net_follows = total_follows - total_unfollows;
    info!("Net follows (should equal total following counts): {net_follows}",);
    info!("Total following counted in the graph: {total_following_counted}",);
    info!("Total followers counted in the graph: {total_followers_counted}",);

    Ok(())
}
