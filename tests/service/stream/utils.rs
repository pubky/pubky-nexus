use serde_json::Value;

// TODO: Check if it is in the cache
pub fn search_tag_in_post(mock_posts: &[Value], label: &str, post_order: Vec<&str>) {
    for (index, post) in mock_posts.iter().enumerate() {
        let mut exist = false;
        // Check if the order of the post is the right one
        assert_eq!(
            post["details"]["id"], post_order[index],
            "The post does not have the right ordering"
        );
        for tag in post["tags"].as_array().unwrap() {
            if tag["label"] == label {
                exist = true;
                break;
            }
        }
        assert!(exist, "The tag was not found in the post. Wrong search")
    }
}

pub fn verify_post_list(mock_posts: Vec<&str>, response: Value) {
    assert!(
        response.is_array(),
        "The response has to be an array of posts"
    );
    let fetched_posts = response.as_array().expect("Post stream should be an array");
    assert!(!fetched_posts.is_empty(), "Post stream should not be empty");
    assert_eq!(
        fetched_posts.len(),
        mock_posts.len(),
        "The endpoint result has to have the same lenght as mock data"
    );

    for (index, post) in fetched_posts.iter().enumerate() {
        assert_eq!(
            mock_posts[index], post["details"]["id"],
            "The post ids should be the same"
        );
    }
}

pub fn verify_post_list_kind(mock_posts: Vec<&str>, response: Value, kind: &str) {
    assert!(
        response.is_array(),
        "The response has to be an array of posts"
    );
    let fetched_posts = response.as_array().expect("Post stream should be an array");
    assert!(!fetched_posts.is_empty(), "Post stream should not be empty");
    assert_eq!(
        fetched_posts.len(),
        mock_posts.len(),
        "The endpoint result has to have the same lenght as mock data"
    );

    for (index, post) in fetched_posts.iter().enumerate() {
        assert_eq!(
            mock_posts[index], post["details"]["id"],
            "The post ids should be the same"
        );
        assert_eq!(
            kind, post["details"]["kind"],
            "The post ids should be the same"
        );
    }
}

pub fn verify_timeline_post_list(mock_posts: Vec<&str>, response: Value) {
    assert!(
        response.is_array(),
        "The response has to be an array of posts"
    );
    let fetched_posts = response.as_array().expect("Post stream should be an array");
    assert!(!fetched_posts.is_empty(), "Post stream should not be empty");
    assert_eq!(
        fetched_posts.len(),
        mock_posts.len(),
        "The endpoint result has to have the same lenght as mock data"
    );

    let mut previous_indexed_at = None;

    for (index, post) in fetched_posts.iter().enumerate() {
        let indexed_at = post["details"]["indexed_at"]
            .as_u64()
            .expect("indexed_at should be a valid number");
        if let Some(prev) = previous_indexed_at {
            assert!(indexed_at <= prev, "Posts are not sorted by timeline");
        }
        assert_eq!(
            mock_posts[index], post["details"]["id"],
            "The post ids should be the same"
        );
        previous_indexed_at = Some(indexed_at);
    }
}
