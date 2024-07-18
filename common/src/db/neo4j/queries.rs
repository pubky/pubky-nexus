use neo4rs::{query, Query};

// Retrive user node by id (pk)
pub fn get_user_by_id(user_id: &str) -> Query {
    query("MATCH (u:User {id: $id}) RETURN u").param("id", user_id)
}
