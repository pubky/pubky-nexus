use std::{
    fs::{self, create_dir_all, remove_file, File},
    io::Write,
};

use anyhow::Result;
use pubky_nexus::models::tag::TagDetails;

const HOST_URL: &str = "http://localhost:8080";

#[tokio::test]
async fn test_info_endpoint() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/v0/info").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["name"], env!("CARGO_PKG_NAME"));
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    Ok(())
}

#[tokio::test]
async fn test_user_endpoint() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Look for Aldert pk user id
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client.do_get(&format!("/v0/user/{}", user_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["details"]["name"], "Aldert");
    assert_eq!(body["details"]["status"], "working");
    assert_eq!(body["details"]["id"], user_id);
    assert_eq!(body["counts"]["friends"], 8);
    assert_eq!(body["counts"]["posts"], 4);

    // Test tags on Ar's profile
    let ar_id = "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy";
    let res = client.do_get(&format!("/v0/user/{}", ar_id)).await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    //let user_profile: UserView = serde_json::from_value(body)?;
    if let Some(tags) = body.get("tags").and_then(|t| t.as_array()) {
        assert_eq!(tags.len(), 3);
        assert!(
            tags.iter().any(|tag| tag["label"] == "pkarr"),
            "Ar profile should tagged as 'pkarr'"
        );
        assert!(
            tags.iter().any(|tag| tag["label"] == "synonym"),
            "Ar profile should tagged as 'synonym'"
        );
        assert!(
            !tags.iter().any(|tag| tag["label"] == "nonsense"),
            "Ar profile should tagged as 'nonsense'"
        );
    }

    // Look for Aldert pk user id using Flavio's viewer id
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!("/v0/user/{}?viewer_id={}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(
        body["relationship"]["followed_by"], true,
        "Aldert should follow Flavio"
    );
    assert_eq!(
        body["relationship"]["following"], false,
        "Flavio should not follow Aldert"
    );

    // Look for a non existing pk
    let user_id = "bad_user_id";
    let res = client.do_get(&format!("/v0/user/{}", user_id)).await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_static_serving() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;
    let test_file_path = "static";
    let test_file_name = "foobar";

    let full_path = format!("{}/{}", test_file_path, test_file_name);

    let exists = match fs::metadata(test_file_path) {
        Err(_) => false,
        Ok(metadata) => metadata.is_dir(),
    };

    println!("file exists? {}", exists);

    if !exists {
        create_dir_all(test_file_path)?;
    }

    let mut file = File::create(full_path.as_str())?;
    file.write_all(b"Hello, world!")?;

    let res = client
        .do_get(format!("/{}", full_path.as_str()).as_str())
        .await?;

    assert_eq!(res.status(), 200);
    assert_eq!(
        res.header("content-length")
            .unwrap()
            .parse::<i32>()
            .unwrap(),
        13
    );

    remove_file(full_path.as_str())?;
    Ok(())
}

#[tokio::test]
async fn test_swagger_ui() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/swagger-ui").await?;
    assert_eq!(res.status(), 200);
    let body = res.text_body()?;
    assert!(body.contains("<html"));

    Ok(())
}

#[tokio::test]
async fn test_openapi_schema() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/api-docs/openapi.json").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["openapi"].is_string());
    assert!(body["info"]["title"].is_string());
    assert_eq!(body["info"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(body["paths"].is_object());

    Ok(())
}

#[tokio::test]
async fn test_get_relationship() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let viewer_id = "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy";
    let res = client
        .do_get(&format!("/v0/user/{}/relationship/{}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["following"].is_boolean());
    assert!(body["followed_by"].is_boolean());

    // Test non-existing relationship
    let user_id = "bad_user_id";
    let viewer_id = "bad_viewer_id";
    let res = client
        .do_get(&format!("/v0/user/{}/relationship/{}", user_id, viewer_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_counts() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/counts", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body["tags"].is_number());
    assert!(body["posts"].is_number());
    assert!(body["followers"].is_number());
    assert!(body["following"].is_number());
    assert!(body["friends"].is_number());

    // Test non-existing user
    let user_id = "bad_user_id";
    let res = client
        .do_get(&format!("/v0/user/{}/counts", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_details() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;

    assert!(body["name"].is_string());
    assert!(body["bio"].is_string());
    assert!(body["id"].is_string());
    assert!(body["status"].is_string());
    assert!(body["links"].is_array());
    assert!(body["indexed_at"].is_number());

    // Test non-existing user
    let user_id = "bad_user_id";
    let res = client
        .do_get(&format!("/v0/user/{}/details", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_post() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let author_id = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
    let post_id = "2ZCW1TGR5BKG0";

    let res = client
        .do_get(&format!(
            "/v0/post/{}/{}?viewer_id={}",
            author_id, post_id, author_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert_eq!(body["details"]["content"], "I am told we can reply now!");
    assert_eq!(body["details"]["indexed_at"].as_u64(), Some(1718616844478));
    assert_eq!(body["details"]["id"], post_id);
    assert_eq!(body["details"]["author"], author_id);
    assert_eq!(
        body["details"]["uri"],
        "pubky://y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy/pub/pubky.app/posts/2ZCW1TGR5BKG0"
    );
    assert_eq!(body["counts"]["tags"].as_u64(), Some(5));
    assert_eq!(body["counts"]["replies"].as_u64(), Some(2));
    assert_eq!(body["counts"]["reposts"].as_u64(), Some(1));
    assert_eq!(body["bookmark"]["indexed_at"].as_u64(), Some(1721764200));
    assert_eq!(body["bookmark"]["id"], "2Z9PFGC3WWWW0");

    // Panic if tags vector is bigger that 1
    let post_tag_object = body["tags"][0].clone();
    let post_tag: TagDetails = serde_json::from_value(post_tag_object.clone())?;
    assert_eq!(post_tag.label, "pubky");

    // Test non-existing post
    let res = client
        .do_get(&format!("/v0/post/{}/{}", author_id, "no_post"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_followers() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/followers", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;

    assert!(body.is_array());
    let followers: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs expected to follow the user
    let specified_follower_ids = vec![
        "ywng83zf5paobxptt8ipkdgq5karppe3edxy1kigb7kgwai56uxo".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
    ];

    // Check if the user has the expected number of followers
    assert_eq!(
        followers.len(),
        specified_follower_ids.len(),
        "Unexpected number of followers"
    );

    // Check if all specified follower IDs are present in the followers list
    for id in &specified_follower_ids {
        assert!(followers.contains(id), "Missing follower ID: {}", id);
    }

    // Test non-existing user
    let res = client
        .do_get(&format!("/v0/user/{}/followers", "bad_user_id"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_following() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/following", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());
    let following: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs the user is expected to be following
    let specified_ids = vec![
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy".to_string(),
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
        "end1obs8cy3ssqzhm73hiojwpakb4ac1fiubbmk5zfuruaaumwso".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "hy1njbk65oh5jqdmq37rdh9xfmxojmn1jn93o7zuka5kx59fdd3o".to_string(),
        "ige6m13kshp3tyhq8ragfapdibixbkzetg8zcdztwwjy16r6erno".to_string(),
        "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro".to_string(),
        "kg671gqu6akiyuzdsqgqtupftuhbuwfx5zh6tbygmexuw6b55s4y".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
    ];

    // Check if the user is following the specified number of users
    assert_eq!(
        following.len(),
        specified_ids.len(),
        "Unexpected number of users followed"
    );

    // Check if all specified IDs are present in the following list
    for id in &specified_ids {
        assert!(following.contains(id), "Missing following ID: {}", id);
    }

    // Test non-existing user
    let res = client
        .do_get(&format!("/v0/user/{}/following", "bad_user_id"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_get_friends() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!("/v0/user/{}/friends", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());
    let following: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_str().unwrap().to_string())
        .collect();

    // List of specified IDs the user is expected to be following
    let specified_ids = vec![
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y".to_string(),
        "gxk8itzrnikrpshfsudgsgtxrz59ojp4iwmp4w9iff3ess6zfr4y".to_string(),
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o".to_string(),
        "hs8iszgmxharf4omxwr7zej196zr4rs4a53ks4tg1ya1efejupty".to_string(),
        "kt1ujy3zxs1tpxsxrqkdpmon5co959paiknw1s4r1rf1gsnqxnao".to_string(),
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo".to_string(),
        "uxni6dn45bbnd7mw6ypf3swoyey9wjntmjo4h1ph9xab1jfhp8do".to_string(),
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy".to_string(),
    ];

    // Check if the user friends the specified number of users
    assert_eq!(
        following.len(),
        specified_ids.len(),
        "Unexpected number of friends"
    );

    // Check if all specified IDs are present in the friend list
    for id in &specified_ids {
        assert!(following.contains(id), "Missing friend ID: {}", id);
    }

    // Test non-existing user
    let res = client
        .do_get(&format!("/v0/user/{}/friends", "bad_user_id"))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_stream_following() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
    let res = client
        .do_get(&format!(
            "/v0/stream/users?stream_type=Following&user_id={}",
            user_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let following = body.as_array().expect("User stream should be an array");

    // Check if the user is following the expected number of users
    assert_eq!(following.len(), 15, "Unexpected number of users followed");

    // List of expected following IDs
    let expected_following_ids = vec![
        "5g3fwnue819wfdjwiwm8qr35ww6uxxgbzrigrtdgmbi19ksioeoy",
        "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y",
        // Add more expected following IDs
    ];

    // Verify that each expected following ID is present in the response
    for id in &expected_following_ids {
        let exists = following.iter().any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected following ID not found: {}", id);
    }

    // Additional checks for specific user attributes (e.g., name, bio)
    for follow in following {
        assert!(
            follow["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(
            follow["details"]["bio"].is_string(),
            "Bio should be a string"
        );
        assert!(
            follow["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    // Test non-existing user
    let res = client
        .do_get(&format!(
            "/v0/stream/users?stream_type=Following&user_id={}",
            "bad_user_id"
        ))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_stream_most_followed() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Test retrieving the most followed users
    let res = client.do_get("/v0/stream/users/most-followed").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let most_followed_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert!(
        !most_followed_users.is_empty(),
        "There should be at least one user in the most followed stream"
    );

    // List of expected user IDs (replace with actual expected IDs from your test data)
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "hj6e38w9dkmpkdmb9c9n6k1yt85ekbqhh3s4aagksdj4zssxg36o",
        "ijfadmjkfxd6mng41jbuaqgm4adcesr5rcs1epnqtny9e43br4ro",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = most_followed_users
            .iter()
            .any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {}", id);
    }

    // Additional checks for specific user attributes (e.g., name, follower counts)
    for user in most_followed_users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(user["details"]["bio"].is_string(), "Bio should be a string");
        assert!(
            user["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    // Test limiting the results to 5 users
    let res = client
        .do_get("/v0/stream/users/most-followed?limit=5")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let limited_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        5,
        "Expected 5 users in the limited stream"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_pioneers() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Test retrieving the most followed users
    let res = client.do_get("/v0/stream/users/pioneers").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let pioneers_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert!(
        !pioneers_users.is_empty(),
        "There should be at least one user in the most followed stream"
    );

    // List of expected user IDs (replace with actual expected IDs from your test data)
    let expected_user_ids = vec![
        "pxnu33x7jtpx9ar1ytsi4yxbp6a5o36gwhffs8zoxmbuptici1jy",
        "o1gg96ewuojmopcjbz8895478wdtxtzzuxnfjjz8o8e77csa1ngo",
        "kzq3o8y8w1b7ffogpq73okop4gb3ahm31ytwwk1na8p6gpr4511o",
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "7hq56kap6exmhghyedrw1q3ar8b1wutomq8ax9eazhajcpdfx3so",
    ];

    // Verify that each expected user ID is present in the response
    for id in &expected_user_ids {
        let exists = pioneers_users.iter().any(|f| f["details"]["id"] == *id);
        assert!(exists, "Expected user ID not found: {}", id);
    }

    // Additional checks for specific user attributes (e.g., name, follower counts)
    for user in pioneers_users {
        assert!(
            user["details"]["name"].is_string(),
            "Name should be a string"
        );
        assert!(user["details"]["bio"].is_string(), "Bio should be a string");
        assert!(
            user["counts"]["followers"].is_number(),
            "Follower counts should be a number"
        );
    }

    // Test limiting the results to 5 users
    let res = client
        .do_get("/v0/stream/users/most-followed?limit=5")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let limited_users = body.as_array().expect("User stream should be an array");

    // Check if the response has the expected number of users
    assert_eq!(
        limited_users.len(),
        5,
        "Expected 5 users in the limited stream"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_timeline() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client.do_get("/v0/stream/posts?sorting=timeline").await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    // Check if the posts are in expected order of timeline
    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
        assert!(
            post["counts"]["tags"].is_number(),
            "tags count should be a number"
        );
        assert!(
            post["counts"]["replies"].is_number(),
            "replies count should be a number"
        );
        assert!(
            post["counts"]["reposts"].is_number(),
            "reposts count should be a number"
        );
    }

    // Additional validation to ensure posts are sorted by timeline
    let mut previous_indexed_at = None;
    for post in posts {
        let indexed_at = post["details"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            print!("{}  {}", indexed_at, prev);
            assert!(indexed_at <= prev, "Posts are not sorted by timeline");
        }
        previous_indexed_at = Some(indexed_at);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_total_engagement() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let res = client
        .do_get("/v0/stream/posts?sorting=totalengagement")
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    // Check if the posts are in expected order of total engagement
    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
        assert!(
            post["counts"]["tags"].is_number(),
            "tags count should be a number"
        );
        assert!(
            post["counts"]["replies"].is_number(),
            "replies count should be a number"
        );
        assert!(
            post["counts"]["reposts"].is_number(),
            "reposts count should be a number"
        );
    }

    // Additional validation to ensure posts are sorted by total engagement
    let mut previous_engagement = None;
    for post in posts {
        let tags = post["counts"]["tags"]
            .as_u64()
            .expect("tags should be a valid number");
        let replies = post["counts"]["replies"]
            .as_u64()
            .expect("replies should be a valid number");
        let reposts = post["counts"]["reposts"]
            .as_u64()
            .expect("reposts should be a valid number");
        let total_engagement = tags + replies + reposts;
        if let Some(prev) = previous_engagement {
            assert!(
                total_engagement <= prev,
                "Posts are not sorted by total engagement"
            );
        }
        previous_engagement = Some(total_engagement);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_user_posts() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Replace "user_id_example" with an actual user ID that exists in your test database
    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = client
        .do_get(&format!("/v0/stream/posts/user/{}", user_id))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user and are sorted by timeline
    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert_eq!(
            post["details"]["author"].as_str(),
            Some(user_id),
            "Post author should match the requested user_id"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
    }

    // Additional validation to ensure posts are sorted by timeline
    let mut previous_indexed_at = None;
    for post in posts {
        let indexed_at = post["details"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            assert!(indexed_at <= prev, "Posts are not sorted by timeline");
        }
        previous_indexed_at = Some(indexed_at);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_following_reach() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = client
        .do_get(&format!(
            "/v0/stream/posts/reach?reach=Following&viewer_id={}",
            viewer_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_followers_reach() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = client
        .do_get(&format!(
            "/v0/stream/posts/reach?reach=Followers&viewer_id={}",
            viewer_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to users who follow the viewer
    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_posts_friends_reach() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let viewer_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = client
        .do_get(&format!(
            "/v0/stream/posts/reach?reach=Friends&viewer_id={}",
            viewer_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_bookmarked_posts() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "h3fghnb3x59oh7r53x8y6a5x38oatqyjym9b31ybss17zqdnhcoy";

    let res = client
        .do_get(&format!(
            "/v0/stream/posts/bookmarks/{}?viewer_id={}",
            user_id, user_id
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let posts = body.as_array().expect("Post stream should be an array");

    // Validate that the posts belong to the specified user's bookmarks
    for post in posts {
        assert!(
            post["details"]["indexed_at"].is_number(),
            "indexed_at should be a number"
        );
        assert!(
            post["details"]["content"].is_string(),
            "content should be a string"
        );
        assert!(
            post["details"]["author"].is_string(),
            "author should be a string"
        );
    }

    // Additional validation to ensure the posts are sorted by when they were bookmarked
    let mut previous_indexed_at = None;
    for post in posts {
        let bookmark_indexed_at = post["bookmark"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            assert!(
                bookmark_indexed_at <= prev,
                "Posts are not sorted by when they were bookmarked"
            );
        }
        previous_indexed_at = Some(bookmark_indexed_at);
    }

    Ok(())
}

#[tokio::test]
async fn test_stream_bookmarked_posts_no_bookmarks() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let user_id = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";

    let res = client
        .do_get(&format!("/v0/stream/posts/bookmarks/{}", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_stream_bookmarked_posts_invalid_user() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Use an invalid or non-existing user ID
    let user_id = "invalid_user_id";

    let res = client
        .do_get(&format!("/v0/stream/posts/bookmarks/{}", user_id))
        .await?;
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_thread_replies() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    // Set the root post's author_id and post_id
    let author_id = "7oq5wj1adxk1u94ojh6eknwj3b4z88zcbb51dbam5zn7zeqnzoio";
    let post_id = "0RE51NMRZAQG";

    // Make a request to the thread endpoint
    let res = client
        .do_get(&format!("/v0/thread/{}/{}", author_id, post_id))
        .await?;

    // Assert that the response status is 200
    assert_eq!(res.status(), 200);

    // Parse the response body as JSON
    let body = res.json_body()?;

    // Ensure the root_post and replies are present in the response
    assert!(
        body["root_post"].is_object(),
        "root_post should be an object"
    );
    assert!(body["replies"].is_array(), "replies should be an array");

    // Validate the root_post fields
    let root_post = body["root_post"]
        .as_object()
        .expect("root_post should be an object");
    assert!(
        root_post["details"]["indexed_at"].is_number(),
        "root_post indexed_at should be a number"
    );
    assert!(
        root_post["details"]["content"].is_string(),
        "root_post content should be a string"
    );
    assert!(
        root_post["details"]["author"].is_string(),
        "root_post author should be a string"
    );

    // Validate the replies
    let replies = body["replies"]
        .as_array()
        .expect("replies should be an array");

    let mut previous_indexed_at = None;

    for reply in replies {
        assert!(
            reply["details"]["indexed_at"].is_number(),
            "reply indexed_at should be a number"
        );
        assert!(
            reply["details"]["content"].is_string(),
            "reply content should be a string"
        );
        assert!(
            reply["details"]["author"].is_string(),
            "reply author should be a string"
        );
        // Validate chronological order
        let indexed_at = reply["details"]["indexed_at"]
            .as_i64()
            .expect("indexed_at should be a valid number");

        if let Some(prev) = previous_indexed_at {
            assert!(
                indexed_at >= prev,
                "replies should be in chronological order"
            );
        }
        previous_indexed_at = Some(indexed_at);
    }

    // Make a request to the thread endpoint for non existing root post
    let res = client
        .do_get(&format!(
            "/v0/thread/{}/{}",
            "non_existing_author", "not_existing_id"
        ))
        .await?;

    // Assert that the response status is 404
    assert_eq!(res.status(), 404);

    Ok(())
}

#[tokio::test]
async fn test_search_users_by_username() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let username = "Jo";

    let res = client
        .do_get(&format!("/v0/search/users?username={}", username))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let users = body
        .as_array()
        .expect("User search results should be an array");

    // Define the expected user IDs
    let expected_users = vec![
        "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy",
        "oh8ku6csenwcyec6oaacz6xumydqjdaagh4ekr8jsm44rrdssjqo",
    ];

    // Convert the actual result to a Vec of strings
    let actual_users: Vec<String> = users
        .iter()
        .map(|user| {
            user.as_str()
                .expect("User ID should be a string")
                .to_string()
        })
        .collect();

    // Assert that the actual result matches the expected result
    assert_eq!(actual_users, expected_users);

    Ok(())
}

#[tokio::test]
async fn test_search_non_existing_user() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let non_existing_username = "idfjwfs8u9jfkoi"; // Username that doesn't exist

    let res = client
        .do_get(&format!(
            "/v0/search/users?username={}",
            non_existing_username
        ))
        .await?;

    // Assert that the status code is 404 Not Found
    assert_eq!(res.status(), 404);

    let body = res.json_body()?;
    assert!(
        body["error"].is_string(),
        "Error message should be a string"
    );

    // Optional: Check that the error message contains the correct details
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains(non_existing_username),
        "Error message should mention the non-existing username"
    );

    Ok(())
}

#[tokio::test]
async fn test_search_empty_username() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let empty_username = ""; // Empty username

    let res = client
        .do_get(&format!("/v0/search/users?username={}", empty_username))
        .await?;

    // Assert that the status code is 400 Bad Request
    assert_eq!(res.status(), 400);

    let body = res.json_body()?;
    assert!(
        body["error"].is_string(),
        "Error message should be a string"
    );

    // Optional: Check that the error message contains the correct details
    assert!(
        body["error"]
            .as_str()
            .unwrap_or("")
            .contains("Username cannot be empty"),
        "Error message should mention that the username cannot be empty"
    );

    Ok(())
}

#[tokio::test]
async fn test_stream_users_by_username_search() -> Result<()> {
    let client = httpc_test::new_client(HOST_URL)?;

    let username = "Jo";

    let res = client
        .do_get(&format!(
            "/v0/stream/users/username-search?username={}",
            username
        ))
        .await?;
    assert_eq!(res.status(), 200);

    let body = res.json_body()?;
    assert!(body.is_array());

    let users = body
        .as_array()
        .expect("User search results should be an array");

    // Validate the response as needed
    assert!(!users.is_empty(), "User search should return results");

    Ok(())
}
