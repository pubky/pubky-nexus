use anyhow::Result;
use nexus_common::{
    db::{fetch_row_from_graph, queries, RedisOps},
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
