use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use nexus_webapi::routes::v0::endpoints::SEARCH_USERS_BY_TAGS_ROUTE;
use serde_json::Value;
use std::collections::HashMap;

// User-profile tag fixtures from docker/test-graph/mocks/tags.cypher
const AURELIO: &str = "c4yotzcb76d31y44jsymtdcowqg7oyqej46jty3yy7ybtzt9x41o";
const ARST: &str = "5f4e8eoogmkhqeyo5ijdix3ma6rw9byj8m36yrjp78pnxxc379to";
const PETER: &str = "db6w58pd5h63fbhtd88y8zz7pai9rkjwqt9omg6i7dz31dynrgcy";

fn search_users_by_tags(query: &str) -> String {
    format!("{SEARCH_USERS_BY_TAGS_ROUTE}?{query}")
}

fn result_rows(body: &Value) -> &Vec<Value> {
    body.as_array().expect("Search results should be an array")
}

fn scores_by_user(body: &Value) -> HashMap<String, u64> {
    result_rows(body)
        .iter()
        .map(|row| {
            (
                row["user_id"]
                    .as_str()
                    .expect("user_id should be a string")
                    .to_string(),
                row["score"].as_u64().expect("score should be a number"),
            )
        })
        .collect()
}

fn assert_scores_descending(body: &Value) {
    let scores: Vec<u64> = result_rows(body)
        .iter()
        .map(|row| row["score"].as_u64().expect("score should be a number"))
        .collect();
    assert!(
        scores.windows(2).all(|w| w[0] >= w[1]),
        "Scores should be descending: {scores:?}"
    );
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_single_tag() -> Result<()> {
    // 5 taggers applied 'now' to aurelio (mocks/tags.cypher WoT block)
    let body = get_request(&search_users_by_tags("tags=now")).await?;
    let scores = scores_by_user(&body);
    assert_eq!(scores.get(AURELIO), Some(&5));

    let body = get_request(&search_users_by_tags("tags=pubky")).await?;
    assert_scores_descending(&body);
    let scores = scores_by_user(&body);
    assert_eq!(scores.get(ARST), Some(&3));
    assert_eq!(scores.get(PETER), Some(&3));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_union() -> Result<()> {
    // Two labels take the graph path; scores sum across them (5 'now' + 3 'athens')
    let body = get_request(&search_users_by_tags("tags=now,athens")).await?;
    assert_scores_descending(&body);
    let scores = scores_by_user(&body);
    assert_eq!(scores.get(AURELIO), Some(&8));

    let occurrences = result_rows(&body)
        .iter()
        .filter(|row| row["user_id"] == AURELIO)
        .count();
    assert_eq!(occurrences, 1, "A user matching both labels appears once");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_redis_graph_parity() -> Result<()> {
    // The same label served from the index (single label) and from the graph
    // (multi label with an unknown second one) must agree on members and
    // scores. Tie order may differ between paths, so compare as maps.
    let redis_body = get_request(&search_users_by_tags("tags=pubky&limit=200")).await?;
    let graph_body =
        get_request(&search_users_by_tags("tags=pubky,nonexistentzz&limit=200")).await?;
    assert_eq!(scores_by_user(&redis_body), scores_by_user(&graph_body));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_pagination() -> Result<()> {
    let full = get_request(&search_users_by_tags("tags=pubky&limit=200")).await?;
    let full = result_rows(&full).clone();
    assert!(
        full.len() >= 3,
        "Fixture should have at least 3 tagged users"
    );

    let page = get_request(&search_users_by_tags("tags=pubky&skip=1&limit=2")).await?;
    assert_eq!(result_rows(&page).as_slice(), &full[1..3]);

    let beyond = get_request(&search_users_by_tags("tags=pubky&skip=9999")).await?;
    assert!(result_rows(&beyond).is_empty());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_unknown_label() -> Result<()> {
    let body = get_request(&search_users_by_tags("tags=nonexistentzz")).await?;
    assert!(result_rows(&body).is_empty());

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_rejects_invalid() -> Result<()> {
    // More than 5 labels
    invalid_get_request(
        &search_users_by_tags("tags=a,b,c,d,e,f"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Over-length label
    let over_length = "a".repeat(21);
    invalid_get_request(
        &search_users_by_tags(&format!("tags={over_length}")),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Missing tags param
    invalid_get_request(SEARCH_USERS_BY_TAGS_ROUTE, StatusCode::BAD_REQUEST).await?;

    Ok(())
}
