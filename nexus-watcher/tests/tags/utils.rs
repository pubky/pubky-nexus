use anyhow::Result;
use neo4rs::{query, Query};
use nexus_common::{
    db::{fetch_key_from_graph, RedisOps},
    models::{
        post::search::{PostsByTagSearch, TAG_GLOBAL_POST_ENGAGEMENT, TAG_GLOBAL_POST_TIMELINE},
        tag::TagDetails,
    },
};

pub async fn find_post_tag(
    user_id: &str,
    post_id: &str,
    tag_name: &str,
) -> Result<Option<TagDetails>> {
    let query = post_tag_query(user_id, post_id, tag_name);
    let maybe_tag_details = fetch_key_from_graph(query, "tag_details").await.unwrap();

    Ok(maybe_tag_details)
}

pub async fn find_user_tag(user_id: &str, tag_name: &str) -> Result<Option<TagDetails>> {
    let query = user_tag_query(user_id, tag_name);
    let maybe_tag_details = fetch_key_from_graph(query, "tag_details").await.unwrap();

    Ok(maybe_tag_details)
}

pub async fn check_member_total_engagement_post_tag(
    post_key: &[&str],
    label: &str,
) -> Result<Option<isize>> {
    let total_engagement = PostsByTagSearch::check_sorted_set_member(
        None,
        &[&TAG_GLOBAL_POST_ENGAGEMENT[..], &[label]].concat(),
        post_key,
    )
    .await
    .unwrap();
    Ok(total_engagement)
}

pub async fn check_member_post_tag_global_timeline(
    post_key: &[&str],
    label: &str,
) -> Result<Option<isize>> {
    let exist_in_timeline = PostsByTagSearch::check_sorted_set_member(
        None,
        &[&TAG_GLOBAL_POST_TIMELINE[..], &[label]].concat(),
        post_key,
    )
    .await
    .unwrap();
    Ok(exist_in_timeline)
}

// Retrieve post related tag
fn post_tag_query(user_id: &str, post_id: &str, tag_name: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $user_id})-[:AUTHORED]->(p:Post {id: $post_id})<-[t:TAGGED {label: $tag_name}]-(tagger:User)
        WITH COUNT(tagger) as count, COLLECT(tagger.id) as list, t.label as label
        RETURN {
            taggers_count: count,
            taggers: list,
            label: label
        } AS tag_details
        ",
    )
    .param("user_id", user_id)
    .param("post_id", post_id)
    .param("tag_name", tag_name)
}

// Retrieve post related tag
fn user_tag_query(tagged_user_id: &str, tag_name: &str) -> Query {
    query(
        "
        MATCH (u:User {id: $tagged_user_id})<-[t:TAGGED {label: $tag_name}]-(tagger:User)
        WITH COUNT(tagger) as count, COLLECT(tagger.id) as list, t.label as label
        RETURN {
            taggers_count: count,
            taggers: list,
            label: label
        } AS tag_details
        ",
    )
    .param("tagged_user_id", tagged_user_id)
    .param("tag_name", tag_name)
}
