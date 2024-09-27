use anyhow::Result;
use chrono::Utc;
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_common::{
    crypto::{Keypair, PublicKey},
    timestamp::Timestamp,
};
use pubky_nexus::{
    models::pubky_app::{
        traits::{GenerateHashId, GenerateTimestampId},
        PostKind, PubkyAppFile, PubkyAppFollow, PubkyAppPost, PubkyAppTag, PubkyAppUser,
    },
    Config,
};
use rand::{distributions::Alphanumeric, Rng};
use std::collections::HashSet;
use std::time::Instant;

static NUM_USERS: usize = 9500;

// Each user will PUT:
static MIN_POSTS: usize = 0;
static MAX_POSTS: usize = 50;

static MIN_TAGS: usize = 3;
static MAX_TAGS: usize = 20;

static MAX_FOLLOWS: usize = 50;

static MIN_FILES: usize = 0;
static MAX_FILES: usize = 20;

#[tokio::main]
async fn main() -> Result<()> {
    let total_start = Instant::now(); // Start timing the whole script

    let config = Config::from_env();

    // Initialize the PubkyClient based on configuration
    let client = match config.testnet {
        true => {
            let testnet = Testnet {
                bootstrap: vec![config.bootstrap.clone()],
                nodes: vec![],
            };
            PubkyClient::builder().testnet(&testnet).build()
        }
        false => PubkyClient::default(),
    };

    // Convert the homeserver from the config into a PublicKey
    let homeserver = PublicKey::try_from(config.homeserver.as_str())?;

    let mut rng = rand::thread_rng();

    // Counters for successes and failures
    let mut users_created_successfully = 0;
    let mut users_failed = 0;

    let mut posts_created_successfully = 0;
    let mut posts_failed = 0;

    let mut follows_created_successfully = 0;
    let mut follows_failed = 0;

    let mut files_created_successfully = 0;
    let mut files_failed = 0;

    let mut tags_created_successfully = 0;
    let mut tags_failed = 0;

    // Pre-allocate vectors for user IDs and their posts
    let mut user_ids: Vec<String> = Vec::with_capacity(NUM_USERS);
    let mut user_posts: Vec<Vec<String>> = Vec::with_capacity(NUM_USERS);

    // Loop to create users, posts, follows, files, and tags
    for i in 0..NUM_USERS {
        let iter_start = Instant::now(); // Start timing this iteration

        println!("Starting Iteration: {}", i);

        // Random keypair for each user
        let keypair = Keypair::random();
        let pk = keypair.public_key().to_z32();
        // Save the user public key (pk) to user_ids
        user_ids.push(pk.clone());
        user_posts.push(Vec::new()); // Initialize the posts vector for this user

        // Perform signup
        if let Err(e) = client.signup(&keypair, &homeserver).await {
            println!("ERROR: Failed to sign up user {}: {}", pk, e);
            users_failed += 1;
            continue; // Skip to the next user
        }

        // Create a user
        let user = PubkyAppUser {
            bio: Some(format!("User Bio {}", i)),
            image: None,
            links: None,
            name: format!("User{}", i),
            status: Some("Active".to_string()),
        };

        let user_profile_json = serde_json::to_vec(&user)?;
        let profile_url = format!("pubky://{}/pub/pubky.app/profile.json", &pk);
        println!("PUT PROFILE: {}", pk);
        if let Err(e) = client.put(profile_url.as_str(), &user_profile_json).await {
            println!("ERROR: Failed to PUT profile for user {}: {}", pk, e);
            users_failed += 1;
            continue; // Skip to the next user
        }

        users_created_successfully += 1;

        // Generate random number of posts (between MIN_POSTS and MAX_POSTS)
        let num_posts = rng.gen_range(MIN_POSTS..=MAX_POSTS);

        // Loop to create random number of posts for this user
        for _ in 0..num_posts {
            let post = PubkyAppPost {
                content: random_string(100),
                kind: PostKind::Short,
                parent: None,
                embed: None,
            };
            let post_id = post.create_id();
            let post_url = format!("pubky://{}/pub/pubky.app/posts/{}", &pk, post_id);
            let post_json = serde_json::to_vec(&post)?;
            println!("PUT POST: {} by user {}", post_id, pk);
            if let Err(e) = client.put(post_url.as_str(), &post_json).await {
                println!(
                    "ERROR: Failed to PUT post {} for user {}: {}",
                    post_id, pk, e
                );
                posts_failed += 1;
                continue; // Skip to the next post
            }

            // Store the post ID
            user_posts[i].push(post_id.clone());
            posts_created_successfully += 1;
        }

        // Generate random number of follows (up to MAX_FOLLOWS)
        let num_follows = if i > 0 {
            rng.gen_range(0..=MAX_FOLLOWS.min(i))
        } else {
            0
        }; // Cannot follow more users than exist
        let mut follow_set = HashSet::new();

        for _ in 0..num_follows {
            // Randomly select a user to follow, ensuring no duplicates and not self
            loop {
                let random_user_index = rng.gen_range(0..i); // Only existing users
                let random_user = &user_ids[random_user_index];
                if random_user != &pk && follow_set.insert(random_user.clone()) {
                    // Create a follow
                    let follow = PubkyAppFollow {
                        created_at: Utc::now().timestamp_millis(),
                    };
                    let follow_url =
                        format!("pubky://{}/pub/pubky.app/follows/{}", &pk, random_user);
                    let follow_json = serde_json::to_vec(&follow)?;
                    println!("PUT user {} follows {}", &pk, random_user);
                    if let Err(e) = client.put(follow_url.as_str(), &follow_json).await {
                        println!(
                            "ERROR: Failed to PUT follow from user {} to {}: {}",
                            pk, random_user, e
                        );
                        follows_failed += 1;
                    } else {
                        follows_created_successfully += 1;
                    }
                    break;
                }
            }
        }

        // Generate random number of files (between MIN_FILES and MAX_FILES)
        let num_files = rng.gen_range(MIN_FILES..=MAX_FILES);

        for _ in 0..num_files {
            // Step 1: Create a blob with random content
            let blob_content = random_string(256); // Random content
            let blob_id = Timestamp::now().to_string();
            let blob_url = format!("pubky://{}/pub/pubky.app/blobs/{}", &pk, blob_id);
            let blob_json = serde_json::to_vec(&blob_content)?;
            println!("PUT BLOB: {} by user {}", blob_id, pk);
            if let Err(e) = client.put(blob_url.as_str(), &blob_json).await {
                println!(
                    "ERROR: Failed to PUT blob {} for user {}: {}",
                    blob_id, pk, e
                );
                files_failed += 1;
                continue; // Skip to the next file
            }

            // Step 2: Create a PubkyAppFile that references the blob
            let file = PubkyAppFile {
                name: format!("file_{}", random_string(5)),
                content_type: "text/plain".to_string(),
                src: blob_url.clone(),
                size: blob_json.len() as u64,
                created_at: Utc::now().timestamp_millis(),
            };

            let file_id = file.create_id();
            let file_url = format!("pubky://{}/pub/pubky.app/files/{}", &pk, file_id);
            let file_json = serde_json::to_vec(&file)?;
            println!("PUT FILE: {} by user {}", file_id, pk);
            if let Err(e) = client.put(file_url.as_str(), &file_json).await {
                println!(
                    "ERROR: Failed to PUT file {} for user {}: {}",
                    file_id, pk, e
                );
                files_failed += 1;
                continue; // Skip to the next file
            } else {
                files_created_successfully += 1;
            }
        }

        // Generate random number of tags (between MIN_TAGS and MAX_TAGS)
        let num_tags = rng.gen_range(MIN_TAGS..=MAX_TAGS);

        for _ in 0..num_tags {
            let tag_label = random_string(rng.gen_range(4..=10));

            // Randomly decide whether to tag a user or a post (20% chance it's a user)
            let tag_target_user = rng.gen_bool(0.2);

            if tag_target_user && i > 0 {
                // Tag a user
                let random_user_index = rng.gen_range(0..i); // Only existing users
                let random_user = &user_ids[random_user_index];
                let tag = PubkyAppTag {
                    uri: format!("pubky://{}/pub/pubky.app/profile.json", random_user),
                    label: tag_label,
                    created_at: Utc::now().timestamp_millis(),
                };
                let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", &pk, tag.create_id());
                let tag_json = serde_json::to_vec(&tag)?;
                println!("PUT user {} tags user {}", &pk, random_user);
                if let Err(e) = client.put(tag_url.as_str(), &tag_json).await {
                    println!(
                        "ERROR: Failed to PUT tag on user {} by user {}: {}",
                        random_user, pk, e
                    );
                    tags_failed += 1;
                    continue; // Skip to the next tag
                } else {
                    tags_created_successfully += 1;
                }
            } else if !user_posts[i].is_empty() {
                // Tag a post
                let random_user_index = rng.gen_range(0..=i); // Including self
                let random_user = &user_ids[random_user_index];
                let user_post_ids = &user_posts[random_user_index];

                if !user_post_ids.is_empty() {
                    let random_post_index = rng.gen_range(0..user_post_ids.len());
                    let random_post_id = &user_post_ids[random_post_index];

                    let tag = PubkyAppTag {
                        uri: format!(
                            "pubky://{}/pub/pubky.app/posts/{}",
                            random_user, random_post_id
                        ),
                        label: tag_label,
                        created_at: Utc::now().timestamp_millis(),
                    };
                    let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", &pk, tag.create_id());
                    let tag_json = serde_json::to_vec(&tag)?;
                    println!(
                        "User {} tags post {} of user {}",
                        &pk, random_post_id, random_user
                    );
                    if let Err(e) = client.put(tag_url.as_str(), &tag_json).await {
                        println!(
                            "ERROR: Failed to PUT tag on post {} of user {} by user {}: {}",
                            random_post_id, random_user, pk, e
                        );
                        tags_failed += 1;
                        continue; // Skip to the next tag
                    } else {
                        tags_created_successfully += 1;
                    }
                }
            }
        }

        // Calculate the time taken for this iteration
        let iter_duration = iter_start.elapsed();
        println!(
            "Iteration {} completed in {:.2} seconds",
            i,
            iter_duration.as_secs_f64()
        );
    }

    // Calculate total time taken
    let total_duration = total_start.elapsed();
    println!(
        "Stress test complete with {} users in {:.2} seconds",
        NUM_USERS,
        total_duration.as_secs_f64()
    );

    // Print out the counts
    println!("\nSummary:");
    println!("Users created successfully: {}", users_created_successfully);
    println!("Users failed: {}", users_failed);

    println!("Posts created successfully: {}", posts_created_successfully);
    println!("Posts failed: {}", posts_failed);

    println!(
        "Follows created successfully: {}",
        follows_created_successfully
    );
    println!("Follows failed: {}", follows_failed);

    println!("Files created successfully: {}", files_created_successfully);
    println!("Files failed: {}", files_failed);

    println!("Tags created successfully: {}", tags_created_successfully);
    println!("Tags failed: {}", tags_failed);

    Ok(())
}

// Helper function to generate random string of given length
fn random_string(len: usize) -> String {
    let rng = rand::thread_rng();
    rng.sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}
