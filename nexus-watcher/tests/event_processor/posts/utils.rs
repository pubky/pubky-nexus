use anyhow::Result;
use neo4rs::{query, Query};
use nexus_common::{
    db::{get_neo4j_graph, RedisOps},
    models::post::{
        PostCounts, PostDetails, PostStream, POST_PER_USER_KEY_PARTS,
        POST_REPLIES_PER_POST_KEY_PARTS, POST_REPLIES_PER_USER_KEY_PARTS, POST_TIMELINE_KEY_PARTS,
        POST_TOTAL_ENGAGEMENT_KEY_PARTS,
    },
};

pub async fn find_post_counts(user_id: &str, post_id: &str) -> PostCounts {
    PostCounts::get_from_index(user_id, post_id)
        .await
        .unwrap()
        .expect("The post count was not served from Nexus cache")
}

pub async fn find_post_details(user_id: &str, post_id: &str) -> Result<PostDetails> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = get_post_details_by_id(user_id, post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Some(row) = row {
        if let Ok(result) = row.get::<PostDetails>("details") {
            return Ok(result);
        }
    }
    anyhow::bail!("Post node not found in Nexus graph");
}

pub async fn check_member_global_timeline_user_post(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_key: &[&str] = &[user_id, post_id];
    let global_timeline_timestamp =
        PostStream::check_sorted_set_member(None, &POST_TIMELINE_KEY_PARTS, post_key)
            .await
            .unwrap();
    Ok(global_timeline_timestamp)
}

pub async fn check_member_user_post_timeline(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_stream_key_parts = [&POST_PER_USER_KEY_PARTS[..], &[user_id]].concat();
    let post_timeline_timestamp =
        PostStream::check_sorted_set_member(None, &post_stream_key_parts, &[post_id])
            .await
            .unwrap();
    Ok(post_timeline_timestamp)
}

pub async fn check_member_user_replies_timeline(
    user_id: &str,
    post_id: &str,
) -> Result<Option<isize>> {
    let post_stream_key_parts = [&POST_REPLIES_PER_USER_KEY_PARTS[..], &[user_id]].concat();
    let post_timeline_timestamp =
        PostStream::check_sorted_set_member(None, &post_stream_key_parts, &[post_id])
            .await
            .unwrap();
    Ok(post_timeline_timestamp)
}

pub async fn check_member_total_engagement_user_posts(post_key: &[&str]) -> Result<Option<isize>> {
    let total_engagement =
        PostStream::check_sorted_set_member(None, &POST_TOTAL_ENGAGEMENT_KEY_PARTS, post_key)
            .await
            .unwrap();
    Ok(total_engagement)
}

pub async fn check_member_post_replies(
    author_id: &str,
    post_id: &str,
    post_key: &[&str],
) -> Result<Option<isize>> {
    let key_parts = [&POST_REPLIES_PER_POST_KEY_PARTS[..], &[author_id, post_id]].concat();

    let post_replies = PostStream::check_sorted_set_member(None, &key_parts, post_key)
        .await
        .unwrap();
    Ok(post_replies)
}

pub async fn find_reply_relationship_parent_uri(user_id: &str, post_id: &str) -> Result<String> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = post_reply_relationships(user_id, post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(relationship) = row.unwrap().get::<Vec<(String, String)>>("details") {
        assert_eq!(
            relationship.len(),
            1,
            "Reply relationship does not exist in the graph"
        );
        return Ok(format!(
            "pubky://{}/pub/pubky.app/posts/{}",
            relationship[0].0, relationship[0].1
        ));
    }
    anyhow::bail!("Post relationship not found in Nexus graph");
}

pub async fn find_repost_relationship_parent_uri(user_id: &str, post_id: &str) -> Result<String> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = post_repost_relationships(user_id, post_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(relationship) = row.unwrap().get::<Vec<(String, String)>>("details") {
        assert_eq!(
            relationship.len(),
            1,
            "Reply relationship does not exist in the graph"
        );
        return Ok(format!(
            "pubky://{}/pub/pubky.app/posts/{}",
            relationship[0].0, relationship[0].1
        ));
    }
    anyhow::bail!("Post relationship not found in Nexus graph");
}

pub fn post_reply_relationships(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        OPTIONAL MATCH (p)-[:REPLIED]->(reply:Post)<-[:AUTHORED]-(reply_author:User)
        RETURN COLLECT([
            reply_author.id,
            reply.id ]) as details",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

pub fn post_repost_relationships(author_id: &str, post_id: &str) -> Query {
    query(
        "MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        OPTIONAL MATCH (p)-[:REPOSTED]->(repost:Post)<-[:AUTHORED]-(repost_author:User)
        RETURN collect([
          repost_author.id,
          repost.id]) as details",
    )
    .param("author_id", author_id)
    .param("post_id", post_id)
}

// Retrieve a post by id
pub fn get_post_details_by_id(user_id: &str, post_id: &str) -> Query {
    query(
        "
        MATCH (user:User {id: $user_id})-[:AUTHORED]->(post:Post {id: $post_id})
        RETURN {
            id: post.id,
            content: post.content,
            kind: post.kind,
            indexed_at: post.indexed_at,
            uri: 'pubky://' + user.id + '/pub/pubky.app/posts/' + post.id,
            author: user.id,
            attachments: post.attachments
        } AS details
        ",
    )
    .param("user_id", user_id)
    .param("post_id", post_id)
}
