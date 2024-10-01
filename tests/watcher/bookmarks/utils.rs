use anyhow::Result;
use pubky_nexus::{get_neo4j_graph, models::post::Bookmark, queries};

pub async fn find_post_bookmark(user_id: &str, post_id: &str, viewer_id: &str) -> Result<Bookmark> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = queries::read::post_bookmark(user_id, post_id, viewer_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<Bookmark>("b") {
        return Ok(result);
    }
    anyhow::bail!("Bookmarked relationship not found in Nexus graph");
}
