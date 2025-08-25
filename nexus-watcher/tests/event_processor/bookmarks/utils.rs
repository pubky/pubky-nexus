use anyhow::Result;
use nexus_common::db::{get_neo4j_graph, queries};
use nexus_common::models::post::Bookmark;

pub async fn find_post_bookmark(
    author: &str,
    post_id: &str,
    bookmarker_id: &str,
) -> Result<Bookmark> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = queries::get::post_bookmark(author, post_id, bookmarker_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<Bookmark>("b") {
        return Ok(result);
    }
    anyhow::bail!("Bookmarked relationship not found in Nexus graph");
}
