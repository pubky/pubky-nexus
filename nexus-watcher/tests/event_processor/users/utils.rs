use anyhow::Result;
use nexus_common::{
    db::{
        exec_single_row, fetch_key_from_graph, fetch_row_from_graph, graph::Query, queries,
        RedisOps,
    },
    models::user::{
        UserCounts, UserDetails, UserStream, USER_INFLUENCERS_KEY_PARTS,
        USER_MOSTFOLLOWED_KEY_PARTS,
    },
};

pub async fn check_member_most_followed(user_id: &str) -> Result<Option<isize>> {
    let influencer_score =
        UserStream::check_sorted_set_member(None, &USER_MOSTFOLLOWED_KEY_PARTS, &[user_id])
            .await
            .unwrap();
    Ok(influencer_score)
}

pub async fn check_member_user_influencer(user_id: &str) -> Result<Option<isize>> {
    let influencer_score =
        UserStream::check_sorted_set_member(None, &USER_INFLUENCERS_KEY_PARTS, &[user_id])
            .await
            .unwrap();
    Ok(influencer_score)
}

pub async fn find_user_counts(user_id: &str) -> UserCounts {
    UserCounts::get_from_index(user_id)
        .await
        .expect("User count not found with that ID")
        .expect("User count not found with that ID")
}

pub async fn find_user_details(user_id: &str) -> Result<UserDetails> {
    let query = queries::get::get_users_details_by_ids(&[user_id]);

    // We always expect a row, even if no UserDetails are found
    // The row contains: id (user_id), record (optional UserDetails)
    let row = fetch_row_from_graph(query).await.unwrap().unwrap();

    if let Ok(result) = row.get::<UserDetails>("record") {
        return Ok(result);
    }
    anyhow::bail!("User node not found in Nexus graph");
}

/// The `uri` property stored on the `User` node, `None` when it is unset.
pub async fn find_user_uri(user_id: &str) -> Option<String> {
    let query = Query::new(
        "find_user_uri",
        "MATCH (u:User {id: $user_id}) RETURN u.uri AS uri",
    )
    .param("user_id", user_id);
    fetch_key_from_graph::<Option<String>>(query, "uri")
        .await
        .unwrap()
        .flatten()
}

/// Overwrite (or remove, with `None`) the `uri` property of a `User` node.
pub async fn set_user_uri(user_id: &str, uri: Option<&str>) -> Result<()> {
    let query = Query::new(
        "set_user_uri",
        "MATCH (u:User {id: $user_id}) SET u.uri = $uri",
    )
    .param("user_id", user_id)
    .param("uri", uri.map(str::to_string));
    exec_single_row(query).await?;
    Ok(())
}
