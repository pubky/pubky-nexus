use anyhow::Result;

use pubky_nexus::{
    get_neo4j_graph, models::post::{PostCounts, PostDetails, PostStream, POST_PER_USER_KEY_PARTS, POST_TIMELINE_KEY_PARTS, POST_TOTAL_ENGAGEMENT_KEY_PARTS}, queries::read::{get_posts_details_by_id, post_reply_relationships, post_repost_relationships}, RedisOps
};

pub async fn find_post_counts(post_key: &[&str]) -> PostCounts {
    PostCounts::try_from_index_json(&post_key)
        .await
        .unwrap()
        .expect("The post count was not served from Nexus cache")
}

pub async fn find_post_details(user_id: &str, post_id: &str) -> PostDetails {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = get_posts_details_by_id(&user_id, &post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<PostDetails>("details") {
        return result;
    }
    assert!(false, "Post node not found in Nexus graph");
    return PostDetails::default();
}

pub async fn check_member_global_timeline_user_post(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_key: &[&str] = &[&user_id, &post_id];
    let global_timeline = PostStream::check_sorted_set_member(&POST_TIMELINE_KEY_PARTS, post_key)
        .await
        .unwrap();
    Ok(global_timeline)
}

pub async fn check_member_user_post_timeline(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[&user_id]].concat();
    let post_timeline = PostStream::check_sorted_set_member(&post_stream_key_parts, &[&post_id])
        .await
        .unwrap();
    Ok(post_timeline)
}

pub async fn check_member_total_engagement_user_posts(
    post_key: &[&str],
) -> Result<Option<isize>> {
    let total_engagement = PostStream::check_sorted_set_member(&POST_TOTAL_ENGAGEMENT_KEY_PARTS, &post_key)
            .await
            .unwrap();
    Ok(total_engagement)
}

pub async fn find_reply_relationship_parent_uri(user_id: &str, post_id: &str) -> String {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = post_reply_relationships(&user_id, &post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(relationship) = row.unwrap().get::<Vec<(String, String)>>("details") {
        assert_eq!(relationship.len(), 1, "Reply relationship does not exist in the graph");
        return format!("pubky://{}/pub/pubky.app/posts/{}", relationship[0].0, relationship[0].1);
    }
    assert!(false, "Post relationship not found in Nexus graph");
    String::from("Uri")
}

pub async fn find_repost_relationship_parent_uri(user_id: &str, post_id: &str) -> String {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = post_repost_relationships(&user_id, &post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(relationship) = row.unwrap().get::<Vec<(String, String)>>("details") {
        assert_eq!(relationship.len(), 1, "Reply relationship does not exist in the graph");
        return format!("pubky://{}/pub/pubky.app/posts/{}", relationship[0].0, relationship[0].1);
    }
    assert!(false, "Post relationship not found in Nexus graph");
    String::from("Uri")
}
