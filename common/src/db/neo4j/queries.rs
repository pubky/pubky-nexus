use neo4rs::{query, Query};

// Retrive user node by id (pk)
pub fn get_user_by_id(user_id: &str) -> Query {
    query("MATCH (u:User {id: $id}) RETURN u").param("id", user_id)
}

pub fn get_tagged_as(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})-[:TAGGED_AS]->(t:Tag)<-[:TAGGED_BY]-(author:User)
           RETURN t.tag AS tag, COUNT(t) AS count, author, COLLECT(author) AS authors",
    )
    .param("id", user_id)
}

pub fn get_follow_counts(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})
           OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
           OPTIONAL MATCH (follower:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
           RETURN COUNT(DISTINCT following) AS following_count,
                  COUNT(DISTINCT follower) AS followers_count,
                  COUNT(DISTINCT friend) AS friends_count",
    )
    .param("id", user_id)
}

pub fn is_following(user_id: &str, viewer_id: &str) -> neo4rs::Query {
    query(
        "MATCH (viewer:User {id: $viewer_id})-[:FOLLOWS]->(u:User {id: $user_id})
           RETURN COUNT(u) > 0 AS following",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id)
}

pub fn is_followed_by(user_id: &str, viewer_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $user_id})-[:FOLLOWS]->(viewer:User {id: $viewer_id})
           RETURN COUNT(u) > 0 AS followed_by",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id)
}
