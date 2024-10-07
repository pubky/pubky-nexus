use anyhow::Result;
use neo4rs::{query, Query};
use pubky_nexus::{
    get_neo4j_graph,
    models::user::{
        UserCounts, UserDetails, UserStream, USER_MOSTFOLLOWED_KEY_PARTS, USER_PIONEERS_KEY_PARTS,
    },
    RedisOps,
};

pub async fn check_member_most_followed(user_id: &str) -> Result<Option<isize>> {
    let pioneer_score =
        UserStream::check_sorted_set_member(&USER_MOSTFOLLOWED_KEY_PARTS, &[user_id])
            .await
            .unwrap();
    Ok(pioneer_score)
}

pub async fn check_member_user_pioneer(user_id: &str) -> Result<Option<isize>> {
    let pioneer_score = UserStream::check_sorted_set_member(&USER_PIONEERS_KEY_PARTS, &[user_id])
        .await
        .unwrap();
    Ok(pioneer_score)
}

pub async fn find_user_counts(user_id: &str) -> UserCounts {
    UserCounts::get_from_index(user_id)
        .await
        .expect("User count not found with that ID")
        .expect("User count not found with that ID")
}

pub async fn find_user_details(user_id: &str) -> Result<UserDetails> {
    let mut row_stream;
    {
        let graph = get_neo4j_graph().unwrap();
        let query = get_user_details_by_id(user_id);

        let graph = graph.lock().await;
        row_stream = graph.execute(query).await.unwrap();
    }

    let row = row_stream.next().await.unwrap();
    if let Ok(result) = row.unwrap().get::<UserDetails>("details") {
        return Ok(result);
    }
    anyhow::bail!("User node not found in Nexus graph");
}

// Retrieve a user by id
pub fn get_user_details_by_id(user_id: &str) -> Query {
    query(
        "
        OPTIONAL MATCH (record:User {id: $id})
        RETURN {
            id: record.id,
            name: record.name,
            bio: record.bio,
            status: record.status,
            links: record.links,
            indexed_at: record.indexed_at
        } AS details
        ",
    )
    .param("id", user_id)
}
