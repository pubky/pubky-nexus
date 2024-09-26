use anyhow::Result;
use chrono::Utc;
use pkarr::mainline::Testnet;
use pubky::PubkyClient;
use pubky_common::crypto::{Keypair, PublicKey};
use pubky_nexus::{
    models::pubky_app::{
        traits::{GenerateHashId, GenerateTimestampId},
        PostKind, PubkyAppPost, PubkyAppTag, PubkyAppUser,
    },
    setup, Config,
};
use rand::Rng;
use tokio::task;

static NUM_ITER: usize = 100;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env();
    setup(&config).await;

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

    let mut user_data: Vec<(Keypair, String)> = Vec::with_capacity(NUM_ITER);
    let mut tasks = vec![];

    // Pre-generate keypairs and user_ids
    for _ in 0..NUM_ITER {
        let keypair = Keypair::random();
        let pk = keypair.public_key().to_z32();
        user_data.push((keypair, pk));
    }

    // Spawn tasks for each user creation, signup, posting, following, and tagging
    for (i, (keypair, pk)) in user_data.iter().cloned().enumerate() {
        let client = client.clone();
        let homeserver = homeserver.clone();
        let _user_data = user_data.clone();

        tasks.push(task::spawn(async move {
            // Perform signup with the pre-generated keypair
            client.signup(&keypair, &homeserver).await.unwrap();

            // Create a user profile
            let user = PubkyAppUser {
                bio: Some(format!("User Bio {}", i)),
                image: None,
                links: None,
                name: format!("User{}", i),
                status: Some("Active".to_string()),
            };
            let user_profile_json = serde_json::to_vec(&user).unwrap();
            let profile_url = format!("pubky://{}/pub/pubky.app/profile.json", &pk);
            client
                .put(profile_url.as_str(), &user_profile_json)
                .await
                .unwrap();

            // Create a post
            let post = PubkyAppPost {
                content: format!("User {}'s post", i),
                kind: PostKind::Short,
                parent: None,
                embed: None,
            };
            let post_id = post.create_id();
            let post_url = format!("pubky://{}/pub/pubky.app/posts/{}", &pk, post_id);
            let post_json = serde_json::to_vec(&post).unwrap();
            client.put(post_url.as_str(), &post_json).await.unwrap();

            // Create a follow (randomly follow an earlier user)
            // Crashes Nexus if done concurrently with:
            // Error: An error was signalled by the server - ResponseError: could not perform this operation on a key that doesn't exist

            // if i > 0 {
            //     let random_user = &user_data[rand::thread_rng().gen_range(0..i)].1;
            //     let follow = PubkyAppFollow {
            //         created_at: Utc::now().timestamp_millis(),
            //     };
            //     let follow_url = format!("pubky://{}/pub/pubky.app/follows/{}", &pk, random_user);
            //     let follow_json = serde_json::to_vec(&follow).unwrap();
            //     client.put(follow_url.as_str(), &follow_json).await.unwrap();
            // }

            // Create a tag for the user
            let tag = PubkyAppTag {
                uri: format!("pubky://{}/pub/pubky.app/profile.json", &pk),
                label: format!("tag{}", rand::thread_rng().gen_range(1..100)),
                created_at: Utc::now().timestamp_millis(),
            };
            let tag_url = format!("pubky://{}/pub/pubky.app/tags/{}", &pk, tag.create_id());
            let tag_json = serde_json::to_vec(&tag).unwrap();
            client.put(tag_url.as_str(), &tag_json).await.unwrap();
        }));
    }

    // Await all tasks to complete
    for task in tasks {
        task.await?;
    }

    println!("Stress test complete with {} iterations", NUM_ITER);
    Ok(())
}
