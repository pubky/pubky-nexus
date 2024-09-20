use neo4rs::{query, Query};
use pubky_nexus::get_neo4j_graph;
use anyhow::Result;

pub async fn find_post_mentions(follower: &str, followee: &str) -> Result<Vec<String>> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = post_mention_query(follower, followee);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<Vec<String>>("mentioned_list") {
        return Ok(result);
    }
    anyhow::bail!("Follow edge not found in Nexus graph");
}

fn post_mention_query(user_id: &str, post_id: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $author_id})-[:AUTHORED]->(p:Post {id: $post_id})
        OPTIONAL MATCH (p)-[:MENTIONED]->(mentioned_user:User)
        RETURN COLLECT(
            mentioned_user.id
        ) as mentioned_list
        ",
    )
    .param("author_id", user_id)
    .param("post_id", post_id)
}
