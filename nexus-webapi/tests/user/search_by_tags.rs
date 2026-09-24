use crate::utils::search_reach::{
    D2, FOLLOWED, FOLLOWER, FRIEND, OBS, STRANGER, UNKNOWN_USER, USER_TAG, USER_TAG_2,
};
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
    // (multi label with an unknown second one) must return identical rows in
    // identical order: both paths break equal scores by user id descending
    let redis_body = get_request(&search_users_by_tags("tags=pubky&limit=200")).await?;
    let graph_body =
        get_request(&search_users_by_tags("tags=pubky,nonexistentzz&limit=200")).await?;
    assert_eq!(result_rows(&redis_body), result_rows(&graph_body));

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

// ── Reach-filtered search ─────────────────────────────────────────────────────

fn user_ids(body: &Value) -> Vec<&str> {
    result_rows(body)
        .iter()
        .map(|row| row["user_id"].as_str().expect("user_id should be a string"))
        .collect()
}

fn reach_query(tags: &str, observer: &str, reach: &str) -> String {
    search_users_by_tags(&format!("tags={tags}&user_id={observer}&reach={reach}"))
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_single_label() -> Result<()> {
    let unfiltered =
        scores_by_user(&get_request(&search_users_by_tags(&format!("tags={USER_TAG}"))).await?);

    // Equal scores break ties by user id descending
    let cases = [
        ("following", vec![FRIEND, FOLLOWED]),
        ("followers", vec![FOLLOWER, FRIEND]),
        // One-directional follows are not friends
        ("friends", vec![FRIEND]),
    ];
    for (reach, expected) in cases {
        let body = get_request(&reach_query(USER_TAG, OBS, reach)).await?;
        assert_eq!(user_ids(&body), expected, "reach={reach}");
        for (user_id, score) in scores_by_user(&body) {
            assert_eq!(
                unfiltered.get(&user_id),
                Some(&score),
                "reach={reach} must keep the unfiltered score of {user_id}"
            );
        }
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_wot() -> Result<()> {
    // Single-label WoT goes to the graph. D2 is only reachable at depth 2, and
    // OBS, reachable through FRIEND's follow back, is excluded
    let body = get_request(&reach_query(USER_TAG, OBS, "wot_2")).await?;
    assert_eq!(user_ids(&body), vec![FRIEND, D2, FOLLOWED]);
    assert_eq!(
        scores_by_user(&body),
        HashMap::from([
            (FRIEND.to_string(), 2),
            (D2.to_string(), 1),
            (FOLLOWED.to_string(), 1)
        ])
    );

    let body = get_request(&reach_query(USER_TAG, OBS, "wot_1")).await?;
    assert_eq!(user_ids(&body), vec![FRIEND, FOLLOWED]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_multi_label() -> Result<()> {
    // FOLLOWED carries both labels (1 + 1), STRANGER is out of reach
    let tags = format!("{USER_TAG},{USER_TAG_2}");
    let body = get_request(&reach_query(&tags, OBS, "following")).await?;
    assert_eq!(user_ids(&body), vec![FOLLOWED, FRIEND]);
    assert_eq!(
        scores_by_user(&body),
        HashMap::from([(FOLLOWED.to_string(), 2), (FRIEND.to_string(), 2)])
    );

    let body = get_request(&reach_query(&tags, OBS, "wot_2")).await?;
    assert_eq!(user_ids(&body), vec![FOLLOWED, FRIEND, D2]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_excludes_observer() -> Result<()> {
    let unfiltered =
        scores_by_user(&get_request(&search_users_by_tags(&format!("tags={USER_TAG}"))).await?);
    assert!(unfiltered.contains_key(OBS), "fixture tags the observer");

    // FRIEND's followers and following both contain OBS
    for reach in [
        "following",
        "followers",
        "friends",
        "wot_1",
        "wot_2",
        "wot_3",
    ] {
        let body = get_request(&reach_query(USER_TAG, OBS, reach)).await?;
        let ids = user_ids(&body);
        assert!(!ids.contains(&OBS), "reach={reach} returned the observer");
        assert!(
            !ids.contains(&STRANGER),
            "reach={reach} returned a stranger"
        );

        let body = get_request(&reach_query(USER_TAG, FRIEND, reach)).await?;
        assert!(
            !user_ids(&body).contains(&FRIEND),
            "reach={reach} returned the observer"
        );
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_other_observers() -> Result<()> {
    // FOLLOWED follows D2 only
    let body = get_request(&reach_query(USER_TAG, FOLLOWED, "following")).await?;
    assert_eq!(user_ids(&body), vec![D2]);

    // FRIEND and OBS follow each other
    let body = get_request(&reach_query(USER_TAG, FRIEND, "friends")).await?;
    assert_eq!(user_ids(&body), vec![OBS]);
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_pagination() -> Result<()> {
    for reach in ["following", "followers", "wot_2"] {
        let full = get_request(&format!("{}&limit=200", reach_query(USER_TAG, OBS, reach))).await?;
        let full = result_rows(&full).clone();
        assert!(full.len() >= 2, "reach={reach} needs two rows to page");

        for (skip, limit) in [(0, 1), (1, 1), (1, 5), (full.len(), 5)] {
            let page = get_request(&format!(
                "{}&skip={skip}&limit={limit}",
                reach_query(USER_TAG, OBS, reach)
            ))
            .await?;
            let end = (skip + limit).min(full.len());
            assert_eq!(
                result_rows(&page).as_slice(),
                &full[skip.min(end)..end],
                "reach={reach} skip={skip} limit={limit}"
            );
        }
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_unknown_user() -> Result<()> {
    for reach in ["following", "followers", "friends", "wot_2"] {
        let body = get_request(&reach_query(USER_TAG, UNKNOWN_USER, reach)).await?;
        assert!(result_rows(&body).is_empty(), "reach={reach}");
    }
    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_user_search_by_tags_reach_requires_both_params() -> Result<()> {
    invalid_get_request(
        &search_users_by_tags(&format!("tags={USER_TAG}&reach=following")),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request(
        &search_users_by_tags(&format!("tags={USER_TAG}&user_id={OBS}")),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request(
        &search_users_by_tags(&format!(
            "tags={USER_TAG}&user_id=not-a-pubky&reach=following"
        )),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    Ok(())
}
