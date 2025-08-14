use anyhow::Result;
use nexus_common::{
    db::{queries, retrieve_from_graph, RedisOps},
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
    let maybe_record = retrieve_from_graph(query, "record").await.unwrap();

    if let Some(result) = maybe_record {
        return Ok(result);
    }
    anyhow::bail!("User node not found in Nexus graph");
}
