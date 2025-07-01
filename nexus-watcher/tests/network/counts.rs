// File: ./tests/watcher/network/large_network_test.rs

use crate::utils::watcher::WatcherTest;
use anyhow::Result;
use nexus_common::{
    db::{PubkyClient, RedisOps},
    models::{post::PostCounts, user::UserCounts},
};
use pubky::Keypair;
use pubky_app_specs::{
    traits::HashId, PubkyAppBookmark, PubkyAppMute, PubkyAppPost, PubkyAppPostKind, PubkyAppTag,
    PubkyAppUser,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
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
    let mut test = WatcherTest::setup().await?;

    if !PROCESS_EVENTS_ONE_BY_ONE {
        test = test.remove_event_processing().await;
    }

    let pubky_client = PubkyClient::get().unwrap();

    // Seed for reproducibility
    let seed = 42;
    let mut rng = StdRng::seed_from_u64(seed);

    // Parameters for network size
    let max_posts_per_user = 10;
    let max_tags_per_user = 15;
    let max_follows_per_user = 20;
    let max_mutes_per_user = 5;
    let max_bookmarks_per_user = 15;

    // Containers to hold user data
    let mut user_ids = Vec::new();
    let mut user_names = Vec::new();
    let mut keypairs = Vec::new();
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
        user_ids.push(user_id.clone());
        user_names.push(user_name);
        keypairs.push(keypair);
        user_posts.insert(user_id.clone(), Vec::new()); // Initialize posts vector for this user
    }

    // Total event counters
    let mut total_posts = 0;
    let mut total_tags = 0;
    let mut total_bookmarks = 0;
    let mut total_follows = 0;
    let mut total_unfollows = 0;
    let mut total_tag_deletions = 0;
    let mut _total_mutes = 0;
    let mut _total_unmutes = 0;

    // Users create posts
    for (i, user_id) in user_ids.iter().enumerate() {
        let num_posts = rng.random_range(1..=max_posts_per_user);
        for _ in 0..num_posts {
            let post = PubkyAppPost {
                content: format!("{}'s post content", user_names[i]),
                kind: PubkyAppPostKind::Short,
                parent: None,
                embed: None,
                attachments: None,
            };
            let post_id = test.create_post(user_id, &post).await?;
            user_posts.get_mut(user_id).unwrap().push(post_id);
            total_posts += 1;
        }
    }

    // Users follow other users
    let mut user_followings: HashMap<String, HashSet<String>> = HashMap::new(); // Map user_id to set of following user_ids

    for user_id in user_ids.iter() {
        user_followings.insert(user_id.clone(), HashSet::new());
    }

    for (i, user_id) in user_ids.iter().enumerate() {
        let num_follows = rng.random_range(1..=max_follows_per_user.min(NUM_USERS - 1));
        let follow_set = &mut user_followings.get_mut(user_id).unwrap();
        while follow_set.len() < num_follows {
            let target_index = rng.random_range(0..NUM_USERS);
            if target_index != i {
                let target_user_id = &user_ids[target_index];
                if follow_set.insert(target_user_id.clone()) {
                    test.create_follow(user_id, target_user_id).await?;
                    total_follows += 1;
                }
            }
        }
    }

    // Users mute other users
    let mut user_mutes: HashMap<String, HashSet<String>> = HashMap::new(); // Map user_id to set of muted user_ids

    for user_id in user_ids.iter() {
        user_mutes.insert(user_id.clone(), HashSet::new());
    }

    for (i, user_id) in user_ids.iter().enumerate() {
        let num_mutes = rng.random_range(0..=max_mutes_per_user.min(NUM_USERS - 1));
        let mute_set = &mut user_mutes.get_mut(user_id).unwrap();
        while mute_set.len() < num_mutes {
            let target_index = rng.random_range(0..NUM_USERS);
            if target_index != i {
                let target_user_id = &user_ids[target_index];
                if mute_set.insert(target_user_id.clone()) {
                    // Create mute
                    let mute = PubkyAppMute {
                        created_at: chrono::Utc::now().timestamp_millis(),
                    };
                    let mute_url =
                        format!("pubky://{user_id}/pub/pubky.app/mutes/{target_user_id}");
                    pubky_client
                        .put(mute_url.as_str())
                        .json(&mute)
                        .send()
                        .await?;
                    _total_mutes += 1;
                }
            }
        }
    }

    // Users bookmark posts
    for user_id in user_ids.iter() {
        let num_bookmarks = rng.random_range(1..=max_bookmarks_per_user);
        for _ in 0..num_bookmarks {
            let target_user_index = rng.random_range(0..NUM_USERS);
            let target_user_id = &user_ids[target_user_index];
            if !user_posts[&target_user_id.clone()].is_empty() {
                let post_index = rng.random_range(0..user_posts[&target_user_id.clone()].len());
                let target_post_id = &user_posts[&target_user_id.clone()][post_index];

                let bookmark = PubkyAppBookmark {
                    uri: format!("pubky://{target_user_id}/pub/pubky.app/posts/{target_post_id}"),
                    created_at: chrono::Utc::now().timestamp_millis(),
                };

                let bookmark_url = format!(
                    "pubky://{}/pub/pubky.app/bookmarks/{}",
                    user_id,
                    bookmark.create_id()
                );

                test.put(&bookmark_url, &bookmark).await?;
                total_bookmarks += 1;
            }
        }
    }

    // Users tag posts of other users
    for user_id in user_ids.iter() {
        let num_tags = rng.random_range(1..=max_tags_per_user);
        for _ in 0..num_tags {
            let target_user_index = rng.random_range(0..NUM_USERS);
            let target_user_id = &user_ids[target_user_index];
            if !user_posts[&target_user_id.clone()].is_empty() {
                let post_index = rng.random_range(0..user_posts[&target_user_id.clone()].len());
                let target_post_id = &user_posts[&target_user_id.clone()][post_index];

                let tag_label = format!("tag{}", rng.random_range(0..100)); // FAILs tag labels are repeated, the same, the counts do not match graph vs index. Graph does not duplicate tag, but index counts do increase.
                let tag = PubkyAppTag {
                    uri: format!("pubky://{target_user_id}/pub/pubky.app/posts/{target_post_id}"),
                    label: tag_label.clone(),
                    created_at: chrono::Utc::now().timestamp_millis(),
                };

                let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", user_id, tag.create_id());

                test.put(&tag_url, &tag).await?;
                total_tags += 1;

                // FAILS: possibly deletes a tag twice and decrements twice in index.
                // Tag counts mismatch for user tesnbp8rctyamkxuc4e1o51yd8zuxf7n9bc96zcwyomu8skrehao between cache and graph
                // left: 12
                // right: 11
                // Randomly decide to delete the tag
                if rng.random_bool(0.1) {
                    // 10% chance to delete the tag
                    test.del(&tag_url).await?;
                    total_tag_deletions += 1;
                }
            }
        }
    }

    // Users unfollow other users
    for user_id in user_ids.iter() {
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
                let follow_uri =
                    format!("pubky://{user_id}/pub/pubky.app/follows/{target_user_id}");
                test.del(&follow_uri).await?;
                following_set.remove(target_user_id);
                total_unfollows += 1;
            }
        }
    }

    // Users unmute other users
    for user_id in user_ids.iter() {
        // Get list of users this user has muted
        let mute_set = &mut user_mutes.get_mut(user_id).unwrap();
        let muted: Vec<String> = mute_set.iter().cloned().collect();

        let num_unmutes = rng.random_range(0..=muted.len());
        let mut unmuted = HashSet::new();

        for _ in 0..num_unmutes {
            if muted.is_empty() || unmuted.len() == muted.len() {
                break;
            }

            let target_index = rng.random_range(0..muted.len());
            let target_user_id = &muted[target_index];
            if unmuted.insert(target_user_id.clone()) {
                let mute_uri = format!("pubky://{user_id}/pub/pubky.app/mutes/{target_user_id}");
                pubky_client.delete(mute_uri.as_str()).send().await?;
                mute_set.remove(target_user_id);
                _total_unmutes += 1;
            }
        }
    }

    if !PROCESS_EVENTS_ONE_BY_ONE {
        for _ in 1..=100 {
            test.event_processor.run().await.unwrap();
        }
    }

    // Now, make assertions
    // For each user, compare counts from cache and graph
    for user_id in user_ids.iter() {
        let counts_cache = UserCounts::try_from_index_json(&[user_id], None)
            .await
            .unwrap()
            .expect("Counts not found in index");
        let counts_graph = UserCounts::get_from_graph(user_id)
            .await
            .unwrap()
            .expect("Counts not found in graph");

        assert_eq!(
            counts_cache.followers, counts_graph.followers,
            "Follower counts mismatch for user {user_id} between cache and graph"
        );
        assert_eq!(
            counts_cache.following, counts_graph.following,
            "Following counts mismatch for user {user_id} between cache and graph"
        );
        assert_eq!(
            counts_cache.posts, counts_graph.posts,
            "Post counts mismatch for user {user_id} between cache and graph"
        );
        // FAILS: Maybe tagging same user twice with same tag?
        // Tag counts mismatch for user 7jh4mieniunce1admx4xrgrd3mqabacms64ek8rredxr7fkkpbto between cache and graph
        // left: 11
        // right: 10
        assert_eq!(
            counts_cache.tagged, counts_graph.tagged,
            "Tag counts mismatch for user {user_id} between cache and graph"
        );
        // FAILS: possibly bookmarking twice?
        // Bookmarks counts mismatch for user taagfd54wqsm9erftpbi6q1tstgy1fbfca57jrk8dtkyodj483mo between cache and graph
        //  left: 15
        //  right: 13
        assert_eq!(
            counts_cache.bookmarks, counts_graph.bookmarks,
            "Bookmarks counts mismatch for user {user_id} between cache and graph"
        );
        assert_eq!(
            counts_cache.tags, counts_graph.tags,
            "Tag counts mismatch for user {user_id} between cache and graph"
        );
        assert_eq!(
            counts_cache.unique_tags, counts_graph.unique_tags,
            "Tag unique counts mismatch for user {user_id} between cache and graph"
        );
        assert_eq!(
            counts_cache.unique_tags, counts_graph.unique_tags,
            "Unique tag counts mismatch for user {user_id} between cache and graph"
        );

        // TODO: mute counts
    }
    //

    // Compare PostCounts for each post
    for (user_id, posts) in user_posts.iter() {
        for post_id in posts.iter() {
            let counts_cache = PostCounts::try_from_index_json(&[user_id, post_id], None)
                .await
                .unwrap()
                .expect("PostCounts not found in index");
            let counts_graph = PostCounts::get_from_graph(user_id, post_id)
                .await
                .unwrap()
                .expect("PostCounts not found in graph");

            assert_eq!(
                counts_cache.tags, counts_graph.0.tags,
                "Tag counts mismatch for post {post_id} by user {user_id} between cache and graph"
            );
            assert_eq!(
                counts_cache.unique_tags, counts_graph.0.unique_tags,
                "Tag unique counts mismatch for post {post_id} by user {user_id} between cache and graph"
            );
            assert_eq!(
                counts_cache.replies, counts_graph.0.replies,
                "Tag counts mismatch for post {post_id} by user {user_id} between cache and graph"
            );
            assert_eq!(
                counts_cache.reposts, counts_graph.0.reposts,
                "Tag counts mismatch for post {post_id} by user {user_id} between cache and graph"
            );
        }
    }

    // Optionally, verify total counts across all users
    let mut total_posts_cache = 0;
    let mut total_tags_cache = 0;
    let mut total_bookmarks_cache = 0;
    let mut total_following_cache = 0;
    let mut total_followers_cache = 0;

    for user_id in user_ids.iter() {
        let counts_cache = UserCounts::try_from_index_json(&[user_id], None)
            .await
            .unwrap()
            .expect("Counts not found in cache");

        total_posts_cache += counts_cache.posts as usize;
        total_tags_cache += counts_cache.tagged as usize;
        total_following_cache += counts_cache.following as usize;
        total_followers_cache += counts_cache.followers as usize;
        total_bookmarks_cache += counts_cache.bookmarks as usize;
    }

    info!("Total posts created: {}", total_posts);
    info!("Total posts counted in index: {}", total_posts_cache);
    info!(
        "Total tags created (excluding deletions): {}",
        total_tags - total_tag_deletions
    );
    info!("Total tags counted in index: {}", total_tags_cache);

    info!("Total bookmarks created: {}", total_bookmarks);
    info!(
        "Total bookmarks counted in index: {}",
        total_bookmarks_cache
    );

    info!("Total follows created: {}", total_follows);
    info!("Total unfollows performed: {}", total_unfollows);
    let net_follows = total_follows - total_unfollows;
    info!(
        "Net follows (should equal total following counts): {}",
        net_follows
    );
    info!(
        "Total following counted in index: {}",
        total_following_cache
    );
    info!(
        "Total followers counted in index: {}",
        total_followers_cache
    );

    assert_eq!(
        total_followers_cache, total_following_cache,
        "Total followers and following should match"
    );

    Ok(())
}
