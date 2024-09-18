use neo4rs::{query, Query};
use pubky_nexus::get_neo4j_graph;

pub async fn find_follow_relationship(follower: &str, followee: &str) -> bool {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = user_following_query(follower, followee);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<bool>("exist") {
        return result;
    }
    assert!(false, "Follow edge not found in Nexus graph");
    return false;
}


fn user_following_query(follower: &str, followee: &str) -> Query {
    query(
        " RETURN EXISTS((:User {id: $follower})-[:FOLLOWS]->(:User {id: $followee})) AS exist"
    )
    .param("followee", followee)
    .param("follower", follower)
}