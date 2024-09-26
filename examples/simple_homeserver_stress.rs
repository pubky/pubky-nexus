use anyhow::Result;
use chrono::Utc;
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_common::crypto::{Keypair, PublicKey};
use pubky_nexus::{
    models::pubky_app::{
        traits::{GenerateHashId, GenerateTimestampId},
        PostKind, PubkyAppFollow, PubkyAppPost, PubkyAppTag, PubkyAppUser,
    },
    Config,
};
use rand::Rng;
use tokio::time::{sleep, Duration};

static NUM_ITER: usize = 500;
static MIN_POSTS: usize = 5;
static MAX_POSTS: usize = 40;

#[tokio::main]
async fn main() -> Result<()> {
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

    // Pre-allocate vector for user_ids
    let mut user_ids: Vec<String> = Vec::with_capacity(NUM_ITER);

    // Loop to create users, posts, follows, and tags
    for i in 0..NUM_ITER {
        // Random keypair for each user
        let keypair = Keypair::random();
        let pk = keypair.public_key().to_z32();
        // Save the user public key (pk) to user_ids
        user_ids.push(pk);

        // Perform signup
        client.signup(&keypair, &homeserver).await?;

        // Create a user
        let user = PubkyAppUser {
            bio: Some(format!("User Bio {}", i)),
            image: None,
            links: None,
            name: format!("User{}", i),
            status: Some("Active".to_string()),
        };

        let user_profile_json = serde_json::to_vec(&user)?;
        let profile_url = format!("pubky://{}/pub/pubky.app/profile.json", &user_ids[i]);
        println!("PUT PROFILE: {}", user_ids[i]);
        client
            .put(profile_url.as_str(), &user_profile_json)
            .await
            .unwrap_or_default();

        // Generate random number of posts (between MIN_POSTS and MAX_POSTS)
        let num_posts = rng.gen_range(MIN_POSTS..=MAX_POSTS);

        // Loop to create random number of posts for this user
        for j in 0..num_posts {
            let post = PubkyAppPost {
                content: format!("User {}'s post number {}", i, j),
                kind: PostKind::Short,
                parent: None,
                embed: None,
            };
            let post_id = post.create_id();
            let post_url = format!("pubky://{}/pub/pubky.app/posts/{}", &user_ids[i], post_id);
            let post_json = serde_json::to_vec(&post)?;
            println!("PUT POST: {}", post_id);
            client.put(post_url.as_str(), &post_json).await?;

            // wait a bit
            sleep(Duration::from_millis(0)).await;
        }

        // Create a follow (randomly follow an earlier user)
        if i > 0 {
            let random_user = &user_ids[rng.gen_range(0..i)];
            let follow = PubkyAppFollow {
                created_at: Utc::now().timestamp_millis(),
            };
            let follow_url = format!(
                "pubky://{}/pub/pubky.app/follows/{}",
                &user_ids[i], random_user
            );
            let follow_json = serde_json::to_vec(&follow)?;
            client.put(follow_url.as_str(), &follow_json).await?;
        }

        // Create a tag for the user
        let tag = PubkyAppTag {
            uri: format!("pubky://{}/pub/pubky.app/profile.json", &user_ids[i]),
            label: format!("tag{}", rng.gen_range(1..100)),
            created_at: Utc::now().timestamp_millis(),
        };
        let tag_url = format!(
            "pubky://{}/pub/pubky.app/tags/{}",
            &user_ids[i],
            tag.create_id()
        );
        let tag_json = serde_json::to_vec(&tag)?;
        client.put(tag_url.as_str(), &tag_json).await?;
    }

    println!("Stress test complete with {} iterations", NUM_ITER);
    Ok(())
}
