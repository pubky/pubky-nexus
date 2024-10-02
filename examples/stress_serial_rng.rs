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
use rand::rngs::StdRng;
use rand::{distributions::Alphanumeric, Rng, SeedableRng};
use rand_distr::{Distribution, LogNormal};
use std::collections::HashSet;
use std::time::Instant;

// Configuration constants
static NUM_USERS: usize = 2000;
static SEED: u64 = 42;

// Adjusted distribution parameters
const POSTS_MU: f64 = 1.7; // Adjust for mean ≈ 40
const POSTS_SIGMA: f64 = 2.0;
const FOLLOWS_MU: f64 = 2.3; // Adjust for mean ≈ 36
const FOLLOWS_SIGMA: f64 = 2.0;
const TAGS_MU: f64 = 3.3; // Adjust for mean ≈ 159
const TAGS_SIGMA: f64 = 2.0;

// Maximum values to cap the numbers
static MAX_POSTS: usize = 10000;
static MAX_FOLLOWS: usize = NUM_USERS - 1;
static MAX_TAGS: usize = 10000;
static MAX_FILES: usize = 10000;

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

    let mut rng = StdRng::seed_from_u64(SEED);
    println!("Using seed: {}", SEED);

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

    // Define LogNormal distributions with adjusted mu values
    let posts_dist = LogNormal::new(POSTS_MU, POSTS_SIGMA).unwrap();
    let follows_dist = LogNormal::new(FOLLOWS_MU, FOLLOWS_SIGMA).unwrap();
    let tags_dist = LogNormal::new(TAGS_MU, TAGS_SIGMA).unwrap();
    let files_dist = LogNormal::new(POSTS_MU, POSTS_SIGMA).unwrap(); // Same as posts_dist

    // Vectors to store per-user counts for analysis
    let mut posts_per_user: Vec<usize> = Vec::with_capacity(NUM_USERS);
    let mut follows_per_user: Vec<usize> = Vec::with_capacity(NUM_USERS);
    let mut tags_per_user: Vec<usize> = Vec::with_capacity(NUM_USERS);
    let mut files_per_user: Vec<usize> = Vec::with_capacity(NUM_USERS);

    // Loop to create users, posts, follows, files, and tags
    for i in 0..NUM_USERS {
        let iter_start = Instant::now(); // Start timing this iteration

        println!("Starting Iteration: {}", i);

        // Create and sign up user
        let (pk, keypair) = Keypair::random().into_keys();
        user_ids.push(pk.clone());
        user_posts.push(Vec::new());

        if !create_user(&client, &keypair, &homeserver, &pk, i).await {
            users_failed += 1;
            continue;
        }
        users_created_successfully += 1;

        // Generate and upload posts
        let (posts_success, posts_fail, num_posts) = create_posts(
            &client,
            &pk,
            &mut rng,
            &posts_dist,
            MAX_POSTS,
            &mut user_posts[i],
        )
        .await;
        posts_created_successfully += posts_success;
        posts_failed += posts_fail;
        posts_per_user.push(num_posts);

        // Create follow relationships
        let (follows_success, follows_fail, num_follows) = create_follows(
            &client,
            &pk,
            i,
            &user_ids,
            &mut rng,
            &follows_dist,
            MAX_FOLLOWS,
        )
        .await;
        follows_created_successfully += follows_success;
        follows_failed += follows_fail;
        follows_per_user.push(num_follows);

        // Generate and upload files
        let (files_success, files_fail, num_files) =
            create_files(&client, &pk, &mut rng, &files_dist, MAX_FILES).await;
        files_created_successfully += files_success;
        files_failed += files_fail;
        files_per_user.push(num_files);

        // Generate and upload tags
        let (tags_success, tags_fail, num_tags) = create_tags(
            &client,
            &pk,
            i,
            &user_ids,
            &user_posts,
            &mut rng,
            &tags_dist,
            MAX_TAGS,
        )
        .await;
        tags_created_successfully += tags_success;
        tags_failed += tags_fail;
        tags_per_user.push(num_tags);

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

    // Calculate and print actual averages
    calculate_actual_averages(
        users_created_successfully,
        posts_created_successfully,
        follows_created_successfully,
        files_created_successfully,
        tags_created_successfully,
    );

    // Calculate and print percentiles
    print_percentiles(&posts_per_user, "Posts");
    print_percentiles(&follows_per_user, "Follows");
    print_percentiles(&files_per_user, "Files");
    print_percentiles(&tags_per_user, "Tags");

    Ok(())
}

// Function to create a user
async fn create_user(
    client: &PubkyClient,
    keypair: &Keypair,
    homeserver: &PublicKey,
    pk: &String,
    user_index: usize,
) -> bool {
    // Perform signup
    if let Err(e) = client.signup(keypair, homeserver).await {
        println!("ERROR: Failed to sign up user {}: {}", pk, e);
        return false;
    }

    // Create a user profile
    let user = PubkyAppUser {
        bio: Some(format!("User Bio {}", user_index)),
        image: None,
        links: None,
        name: format!("User{}", user_index),
        status: Some("Active".to_string()),
    };

    let user_profile_json = match serde_json::to_vec(&user) {
        Ok(json) => json,
        Err(e) => {
            println!("ERROR: Failed to serialize profile for user {}: {}", pk, e);
            return false;
        }
    };
    let profile_url = format!("pubky://{}/pub/pubky.app/profile.json", pk);
    println!("PUT PROFILE: {}", pk);
    if let Err(e) = client.put(profile_url.as_str(), &user_profile_json).await {
        println!("ERROR: Failed to PUT profile for user {}: {}", pk, e);
        return false;
    }
    true
}

// Function to create posts
async fn create_posts(
    client: &PubkyClient,
    pk: &String,
    rng: &mut StdRng,
    posts_dist: &LogNormal<f64>,
    max_posts: usize,
    user_post_ids: &mut Vec<String>,
) -> (usize, usize, usize) {
    let mut posts_created_successfully = 0;
    let mut posts_failed = 0;

    let num_posts = posts_dist.sample(rng).round() as usize;
    let num_posts = num_posts.min(max_posts);

    for _ in 0..num_posts {
        let post = PubkyAppPost {
            content: random_string(rng, 100),
            kind: PostKind::Short,
            parent: None,
            embed: None,
        };
        let post_id = post.create_id();
        let post_url = format!("pubky://{}/pub/pubky.app/posts/{}", pk, post_id);
        let post_json = match serde_json::to_vec(&post) {
            Ok(json) => json,
            Err(e) => {
                println!("ERROR: Failed to serialize post {}: {}", post_id, e);
                posts_failed += 1;
                continue;
            }
        };
        println!("PUT POST: {}", post_id);
        if let Err(e) = client.put(post_url.as_str(), &post_json).await {
            println!(
                "ERROR: Failed to PUT post {} for user {}: {}",
                post_id, pk, e
            );
            posts_failed += 1;
            continue;
        }

        // Store the post ID
        user_post_ids.push(post_id.clone());
        posts_created_successfully += 1;
    }

    (posts_created_successfully, posts_failed, num_posts)
}

// Function to create follows
async fn create_follows(
    client: &PubkyClient,
    pk: &String,
    current_index: usize,
    user_ids: &Vec<String>,
    rng: &mut StdRng,
    follows_dist: &LogNormal<f64>,
    max_follows: usize,
) -> (usize, usize, usize) {
    let mut follows_created_successfully = 0;
    let mut follows_failed = 0;

    let num_follows = if current_index > 0 {
        let num = follows_dist.sample(rng).round() as usize;
        num.min(max_follows.min(current_index))
    } else {
        0
    };

    let mut follow_set = HashSet::new();

    for _ in 0..num_follows {
        // Randomly select a user to follow, ensuring no duplicates and not self
        loop {
            let random_user_index = rng.gen_range(0..current_index); // Only existing users
            let random_user = &user_ids[random_user_index];
            if random_user != pk && follow_set.insert(random_user.clone()) {
                // Create a follow
                let follow = PubkyAppFollow {
                    created_at: Utc::now().timestamp_millis(),
                };
                let follow_url = format!("pubky://{}/pub/pubky.app/follows/{}", pk, random_user);
                let follow_json = match serde_json::to_vec(&follow) {
                    Ok(json) => json,
                    Err(e) => {
                        println!(
                            "ERROR: Failed to serialize follow from user {} to {}: {}",
                            pk, random_user, e
                        );
                        follows_failed += 1;
                        break;
                    }
                };
                println!("PUT FOLLOW: {}", random_user);
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

    (follows_created_successfully, follows_failed, num_follows)
}

// Function to create files
async fn create_files(
    client: &PubkyClient,
    pk: &String,
    rng: &mut StdRng,
    files_dist: &LogNormal<f64>,
    max_files: usize,
) -> (usize, usize, usize) {
    let mut files_created_successfully = 0;
    let mut files_failed = 0;

    let num_files = files_dist.sample(rng).round() as usize;
    let num_files = num_files.min(max_files);

    for _ in 0..num_files {
        // Step 1: Create a blob with random content
        let blob_content = random_string(rng, 256); // Random content
        let blob_id = Timestamp::now().to_string();
        let blob_url = format!("pubky://{}/pub/pubky.app/blobs/{}", pk, blob_id);
        let blob_json = match serde_json::to_vec(&blob_content) {
            Ok(json) => json,
            Err(e) => {
                println!("ERROR: Failed to serialize blob {}: {}", blob_id, e);
                files_failed += 1;
                continue;
            }
        };
        println!("PUT BLOB: {}", blob_id);
        if let Err(e) = client.put(blob_url.as_str(), &blob_json).await {
            println!(
                "ERROR: Failed to PUT blob {} for user {}: {}",
                blob_id, pk, e
            );
            files_failed += 1;
            continue;
        }

        // Step 2: Create a PubkyAppFile that references the blob
        let file = PubkyAppFile {
            name: format!("file_{}", random_string(rng, 5)),
            content_type: "text/plain".to_string(),
            src: blob_url.clone(),
            size: blob_json.len() as u64,
            created_at: Utc::now().timestamp_millis(),
        };

        let file_id = file.create_id();
        let file_url = format!("pubky://{}/pub/pubky.app/files/{}", pk, file_id);
        let file_json = match serde_json::to_vec(&file) {
            Ok(json) => json,
            Err(e) => {
                println!("ERROR: Failed to serialize file {}: {}", file_id, e);
                files_failed += 1;
                continue;
            }
        };

        println!("PUT FILE: {}", file_id);
        if let Err(e) = client.put(file_url.as_str(), &file_json).await {
            println!(
                "ERROR: Failed to PUT file {} for user {}: {}",
                file_id, pk, e
            );
            files_failed += 1;
            continue;
        } else {
            files_created_successfully += 1;
        }
    }

    (files_created_successfully, files_failed, num_files)
}

// Function to create tags
async fn create_tags(
    client: &PubkyClient,
    pk: &String,
    current_index: usize,
    user_ids: &Vec<String>,
    user_posts: &Vec<Vec<String>>,
    rng: &mut StdRng,
    tags_dist: &LogNormal<f64>,
    max_tags: usize,
) -> (usize, usize, usize) {
    let mut tags_created_successfully = 0;
    let mut tags_failed = 0;

    let num_tags = tags_dist.sample(rng).round() as usize;
    let num_tags = num_tags.min(max_tags);

    for _ in 0..num_tags {
        let tag_length = rng.gen_range(4..=10);
        let tag_label = random_string(rng, tag_length);

        // Randomly decide whether to tag a user or a post
        let tag_target_user = rng.gen_bool(0.2);

        if tag_target_user && current_index > 0 {
            // Tag a user
            let random_user_index = rng.gen_range(0..current_index); // Only existing users
            let random_user = &user_ids[random_user_index];
            let tag = PubkyAppTag {
                uri: format!("pubky://{}/pub/pubky.app/profile.json", random_user),
                label: tag_label,
                created_at: Utc::now().timestamp_millis(),
            };
            let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", pk, tag.create_id());
            let tag_json = match serde_json::to_vec(&tag) {
                Ok(json) => json,
                Err(e) => {
                    println!(
                        "ERROR: Failed to serialize tag on user {}: {}",
                        random_user, e
                    );
                    tags_failed += 1;
                    continue;
                }
            };
            println!("PUT USER TAG: {}", tag.create_id());
            if let Err(e) = client.put(tag_url.as_str(), &tag_json).await {
                println!(
                    "ERROR: Failed to PUT tag on user {} by user {}: {}",
                    random_user, pk, e
                );
                tags_failed += 1;
                continue;
            } else {
                tags_created_successfully += 1;
            }
        } else {
            // Tag a post
            let total_users = if current_index == 0 {
                1
            } else {
                current_index + 1
            };
            let random_user_index = rng.gen_range(0..total_users); // Including self
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
                let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", pk, tag.create_id());
                let tag_json = match serde_json::to_vec(&tag) {
                    Ok(json) => json,
                    Err(e) => {
                        println!(
                            "ERROR: Failed to serialize tag on post {}: {}",
                            random_post_id, e
                        );
                        tags_failed += 1;
                        continue;
                    }
                };
                println!("PUT POST TAG: {}", tag.create_id());
                if let Err(e) = client.put(tag_url.as_str(), &tag_json).await {
                    println!(
                        "ERROR: Failed to PUT tag on post {} of user {} by user {}: {}",
                        random_post_id, random_user, pk, e
                    );
                    tags_failed += 1;
                    continue;
                } else {
                    tags_created_successfully += 1;
                }
            }
        }
    }

    (tags_created_successfully, tags_failed, num_tags)
}

// Function to calculate and print actual averages
fn calculate_actual_averages(
    users_created_successfully: usize,
    posts_created_successfully: usize,
    follows_created_successfully: usize,
    files_created_successfully: usize,
    tags_created_successfully: usize,
) {
    println!("\nActual Averages:");
    if users_created_successfully > 0 {
        println!(
            "Average posts per user: {:.2}",
            posts_created_successfully as f64 / users_created_successfully as f64
        );
        println!(
            "Average follows per user: {:.2}",
            follows_created_successfully as f64 / users_created_successfully as f64
        );
        println!(
            "Average files per user: {:.2}",
            files_created_successfully as f64 / users_created_successfully as f64
        );
        println!(
            "Average tags per user: {:.2}",
            tags_created_successfully as f64 / users_created_successfully as f64
        );
    }
}

// Function to print percentiles
fn print_percentiles(data: &Vec<usize>, label: &str) {
    let mut data = data.clone();
    data.sort_unstable();

    let percentiles = [25.0, 50.0, 75.0, 90.0, 95.0, 99.0, 99.5, 99.7, 99.9, 99.99];
    println!("\n{} per User Percentiles:", label);
    for &p in &percentiles {
        let rank = (p / 100.0) * (data.len() as f64 - 1.0);
        let idx = rank.round() as usize;
        let value = data.get(idx).unwrap_or(&0);
        println!("{:5.2}th percentile: {}", p, value);
    }
}

// Helper function to generate random string of given length
fn random_string(rng: &mut StdRng, len: usize) -> String {
    rng.sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

// Extension trait to convert Keypair into public key and keypair tuple
trait IntoKeys {
    fn into_keys(self) -> (String, Keypair);
}

impl IntoKeys for Keypair {
    fn into_keys(self) -> (String, Keypair) {
        let pk = self.public_key().to_z32();
        (pk, self)
    }
}
