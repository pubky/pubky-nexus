use serde_json::Value;

// TODO: Check if it is in the cache
pub fn search_tag_in_post(posts: &[Value], label: &str, post_order: Vec<&str>) {
    for (index, post) in posts.iter().enumerate() {
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
