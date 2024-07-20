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

pub fn get_user_counts(user_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $id})
           OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
           OPTIONAL MATCH (follower:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
           OPTIONAL MATCH (u)-[:AUTHORED]->(post:Post)
           OPTIONAL MATCH (u)-[tag:TAGGED]->(:Post)
           RETURN COUNT(DISTINCT following) AS following_count,
                  COUNT(DISTINCT follower) AS followers_count,
                  COUNT(DISTINCT friend) AS friends_count,
                  COUNT(DISTINCT post) AS posts_count,
                  COUNT(DISTINCT tag) AS tags_count",
    )
    .param("id", user_id)
}

pub fn viewer_relationship(user_id: &str, viewer_id: &str) -> neo4rs::Query {
    query(
        "MATCH (u:User {id: $user_id})
         OPTIONAL MATCH (viewer:User {id: $viewer_id})-[:FOLLOWS]->(u)
         OPTIONAL MATCH (u)-[:FOLLOWS]->(viewer)
         RETURN COUNT(DISTINCT viewer) > 0 AS following,
                COUNT(DISTINCT u) > 0 AS followed_by",
    )
    .param("user_id", user_id)
    .param("viewer_id", viewer_id)
}

// Combine Profile queries into one (Not yet used)
pub fn _get_user_profile_data(user_id: &str, viewer_id: Option<&str>) -> Query {
    let viewer = viewer_id.unwrap_or("none");
    println!("{viewer}");

    query(
        "
        MATCH (u:User {{id: $id}})
        OPTIONAL MATCH (u)-[:TAGGED_AS]->(t:Tag)<-[:TAGGED_BY]-(author:User)
        OPTIONAL MATCH (u)-[:AUTHORED]->(p:Post)
        OPTIONAL MATCH (u)-[:FOLLOWS]->(following:User)
        OPTIONAL MATCH (follower:User)-[:FOLLOWS]->(u)
        OPTIONAL MATCH (u)-[:FOLLOWS]->(friend:User)-[:FOLLOWS]->(u)
        OPTIONAL MATCH (viewer:User {{id: $viewer_id}})-[f:FOLLOWS]->(u)
        OPTIONAL MATCH (u)-[fb:FOLLOWS]->(viewer)
        RETURN COUNT(f) > 0 AS following, COUNT(fb) > 0 AS followed_by
        RETURN u, COLLECT(DISTINCT t) AS tags, 
               COUNT(DISTINCT p) AS posts_count,
               COUNT(DISTINCT following) AS following_count,
               COUNT(DISTINCT follower) AS followers_count,
               COUNT(DISTINCT friend) AS friends_count,
               false AS following, false AS followed_by
        ",
    )
    .param("id", user_id)
    .param("viewer_id", viewer)
}
