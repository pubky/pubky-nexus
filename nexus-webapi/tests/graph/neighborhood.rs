use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;
use std::collections::HashSet;

const ALDERT: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
const NON_EXISTING_USER_ID: &str = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";

// Fixture facts (docker/test-graph): Aldert follows 15 users and is followed
// by 10, 8 of which are mutual, so his undirected FOLLOWS neighborhood is 17
// distinct users. He authored 4 posts; "pubky" is the hottest label on him and
// his posts. 9x86... and y4eu... are both friends of Aldert and follow each
// other, giving a neighbor-to-neighbor FOLLOWS edge.
const ALDERT_NEIGHBORS: usize = 17;
const ALDERT_POSTS: [&str; 4] = [
    "0RDY1Y34YPHG",
    "0RDXX1QHWJDG",
    "2Z9P8AN738C00",
    "2Z1NJPW2QHGG0",
];
const MUTUAL_A: &str = "9x86hgp5tya98csx4wfdj1aorcxszxq5mwa3rdeh8a7oz1u6sg3y";
const MUTUAL_B: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";

fn nodes(res: &Value) -> &Vec<Value> {
    res["nodes"].as_array().expect("nodes must be an array")
}

fn edges(res: &Value) -> &Vec<Value> {
    res["edges"].as_array().expect("edges must be an array")
}

fn node_ids(res: &Value) -> HashSet<String> {
    nodes(res)
        .iter()
        .map(|n| n["id"].as_str().expect("node id").to_string())
        .collect()
}

fn user_node_count(res: &Value) -> usize {
    nodes(res).iter().filter(|n| n["kind"] == "user").count()
}

fn has_edge(res: &Value, source: &str, target: &str, edge_type: &str) -> bool {
    edges(res)
        .iter()
        .any(|e| e["source"] == source && e["target"] == target && e["type"] == edge_type)
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_user_depth1_shape() -> Result<()> {
    let res = get_request(&format!("/v0/graph/user/{ALDERT}")).await?;

    let ids = node_ids(&res);
    let center = format!("user:{ALDERT}");

    // Center present with hydration payload
    let center_node = nodes(&res)
        .iter()
        .find(|n| n["id"] == center.as_str())
        .expect("center node missing");
    assert_eq!(center_node["kind"], "user");
    assert_eq!(center_node["name"], "Aldert");
    assert_eq!(center_node["pubky"], ALDERT);

    // Every edge endpoint must reference a returned node
    for edge in edges(&res) {
        for key in ["source", "target"] {
            let endpoint = edge[key].as_str().expect("edge endpoint");
            assert!(ids.contains(endpoint), "dangling edge endpoint: {endpoint}");
        }
    }

    // The full undirected follow neighborhood fits under the default limit
    assert_eq!(
        user_node_count(&res),
        1 + ALDERT_NEIGHBORS,
        "expected center + 17 follow neighbors"
    );

    // Every FOLLOWS edge carries its indexed_at timestamp (time machine fuel)
    for edge in edges(&res).iter().filter(|e| e["type"] == "FOLLOWS") {
        assert!(
            edge["indexed_at"].is_number(),
            "FOLLOWS edge missing indexed_at: {edge}"
        );
    }
    // User-to-user TAGGED edges carry one too
    let user_tagged_ts = edges(&res).iter().any(|e| {
        e["type"] == "TAGGED"
            && e["source"].as_str().unwrap().starts_with("user:")
            && e["target"].as_str().unwrap().starts_with("user:")
            && e["indexed_at"].is_number()
    });
    assert!(
        user_tagged_ts,
        "expected timestamped user-user TAGGED edges"
    );

    // Directionality: mutuals have both directed edges; and at least one
    // neighbor-to-neighbor FOLLOWS edge exists (not incident to the center)
    let mutual_a = format!("user:{MUTUAL_A}");
    let mutual_b = format!("user:{MUTUAL_B}");
    assert!(has_edge(&res, &center, &mutual_a, "FOLLOWS"));
    assert!(has_edge(&res, &mutual_a, &center, "FOLLOWS"));
    assert!(
        has_edge(&res, &mutual_a, &mutual_b, "FOLLOWS"),
        "missing neighbor-to-neighbor FOLLOWS edge"
    );

    // Posts: all 4 fixture posts inline with content + AUTHORED edges
    for post_id in ALDERT_POSTS {
        let post_node_id = format!("post:{ALDERT}:{post_id}");
        let post_node = nodes(&res)
            .iter()
            .find(|n| n["id"] == post_node_id.as_str())
            .unwrap_or_else(|| panic!("missing post node {post_node_id}"));
        assert_eq!(post_node["kind"], "post");
        assert_eq!(post_node["author_id"], ALDERT);
        assert!(post_node["content"].is_string());
        assert!(post_node["indexed_at"].is_number());
        assert!(has_edge(&res, &center, &post_node_id, "AUTHORED"));
    }

    // Tags: "pubky" is the hottest label on Aldert + his posts; it must be a
    // node, with a labeled TAGGED hub edge into the neighborhood
    let tag_node = nodes(&res)
        .iter()
        .find(|n| n["id"] == "tag:pubky")
        .expect("missing tag:pubky node");
    assert_eq!(tag_node["kind"], "tag");
    assert_eq!(tag_node["label"], "pubky");
    assert!(tag_node["count"].as_u64().unwrap() >= 1);
    let tag_hub_edge = edges(&res)
        .iter()
        .any(|e| e["source"] == "tag:pubky" && e["type"] == "TAGGED" && e["label"] == "pubky");
    assert!(tag_hub_edge, "missing TAGGED hub edge from tag:pubky");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_user_depth2_grows_and_stays_bounded() -> Result<()> {
    let depth1 = get_request(&format!("/v0/graph/user/{ALDERT}?kinds=user")).await?;
    let depth2 = get_request(&format!("/v0/graph/user/{ALDERT}?kinds=user&depth=2")).await?;

    assert!(
        user_node_count(&depth2) > user_node_count(&depth1),
        "depth 2 should discover hop-2 users"
    );
    // 1 center + 50 hop-1 max + 120 hop-2 cap
    assert!(user_node_count(&depth2) <= 171, "depth-2 node budget blown");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_user_limit_bounds_hop1() -> Result<()> {
    let res = get_request(&format!("/v0/graph/user/{ALDERT}?kinds=user&limit=1")).await?;
    assert_eq!(user_node_count(&res), 2, "center + exactly one neighbor");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_kinds_filter_users_only() -> Result<()> {
    let res = get_request(&format!("/v0/graph/user/{ALDERT}?kinds=user")).await?;

    for node in nodes(&res) {
        assert_eq!(node["kind"], "user", "kinds=user must exclude other kinds");
    }
    for edge in edges(&res) {
        assert_eq!(
            edge["type"], "FOLLOWS",
            "kinds=user leaves no room for non-FOLLOWS edges"
        );
    }

    Ok(())
}

// Fixture facts: 14 distinct users tagged something "pubky"; post
// 1A1P4D8C9K0FF by emq37... has 6 replies.
const AMSTERDAM: &str = "emq37ky6fbnaun7q1ris6rx3mqmw3a33so1txfesg9jj3ak9ryoy";
const REPLIED_POST: &str = "1A1P4D8C9K0FF";

#[tokio_shared_rt::test(shared)]
async fn test_graph_tag_center() -> Result<()> {
    let res = get_request("/v0/graph/tag/pubky").await?;

    let center = nodes(&res)
        .iter()
        .find(|n| n["id"] == "tag:pubky")
        .expect("tag center node missing");
    assert_eq!(center["kind"], "tag");
    assert_eq!(center["label"], "pubky");
    assert!(center["count"].as_u64().unwrap() >= 14, "taggers count");

    // Taggers appear as user nodes connected via labeled TAGGED edges into the tag hub
    let tagger_edge = edges(&res).iter().any(|e| {
        e["target"] == "tag:pubky"
            && e["type"] == "TAGGED"
            && e["label"] == "pubky"
            && e["source"].as_str().unwrap().starts_with("user:")
    });
    assert!(tagger_edge, "missing user -> tag:pubky tagger edge");

    // FOLLOWS edges among returned taggers make the cluster explorable
    let follows_among_users = edges(&res).iter().any(|e| e["type"] == "FOLLOWS");
    assert!(follows_among_users, "expected FOLLOWS edges among taggers");

    // No dangling edges
    let ids = node_ids(&res);
    for edge in edges(&res) {
        for key in ["source", "target"] {
            let endpoint = edge[key].as_str().unwrap();
            assert!(ids.contains(endpoint), "dangling edge endpoint: {endpoint}");
        }
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_tag_center_kinds_user_keeps_center_edges() -> Result<()> {
    // kinds=user on a tag center must not orphan the center: edges incident to
    // it survive type gating (taggers stay connected to the tag hub)
    let res = get_request("/v0/graph/tag/pubky?kinds=user").await?;

    assert!(nodes(&res).iter().any(|n| n["id"] == "tag:pubky"));
    assert!(nodes(&res).iter().filter(|n| n["kind"] == "user").count() > 0);
    assert!(nodes(&res).iter().all(|n| n["kind"] != "post"));
    let center_edges = edges(&res)
        .iter()
        .filter(|e| {
            e["type"] == "TAGGED" && (e["source"] == "tag:pubky" || e["target"] == "tag:pubky")
        })
        .count();
    assert!(
        center_edges > 0,
        "center-incident TAGGED edges must survive"
    );
    // Non-center TAGGED edges stay filtered
    assert!(edges(&res).iter().all(|e| e["type"] != "TAGGED"
        || e["source"] == "tag:pubky"
        || e["target"] == "tag:pubky"));

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_post_center() -> Result<()> {
    let res = get_request(&format!("/v0/graph/post/{AMSTERDAM}:{REPLIED_POST}")).await?;

    let center_id = format!("post:{AMSTERDAM}:{REPLIED_POST}");
    let center = nodes(&res)
        .iter()
        .find(|n| n["id"] == center_id.as_str())
        .expect("post center node missing");
    assert_eq!(center["kind"], "post");
    assert_eq!(center["author_id"], AMSTERDAM);

    // Author present with an AUTHORED edge to the center
    let author_id = format!("user:{AMSTERDAM}");
    assert!(node_ids(&res).contains(&author_id), "author node missing");
    assert!(has_edge(&res, &author_id, &center_id, "AUTHORED"));

    // The 6 fixture replies show up as post nodes with REPLIED edges into the center
    let reply_edges = edges(&res)
        .iter()
        .filter(|e| e["target"] == center_id.as_str() && e["type"] == "REPLIED")
        .count();
    assert_eq!(reply_edges, 6, "expected 6 REPLIED edges");

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_invalid_params() -> Result<()> {
    // BoundedLimit<30, 50>: 0 and >50 rejected
    invalid_get_request(
        &format!("/v0/graph/user/{ALDERT}?limit=0"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request(
        &format!("/v0/graph/user/{ALDERT}?limit=51"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // depth restricted to 1..=2, and only meaningful for user centers
    invalid_get_request(
        &format!("/v0/graph/user/{ALDERT}?depth=0"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request(
        &format!("/v0/graph/user/{ALDERT}?depth=3"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request("/v0/graph/tag/pubky?depth=2", StatusCode::BAD_REQUEST).await?;

    // unknown kinds entry and unknown center kind
    invalid_get_request(
        &format!("/v0/graph/user/{ALDERT}?kinds=user,banana"),
        StatusCode::BAD_REQUEST,
    )
    .await?;
    invalid_get_request("/v0/graph/banana/whatever", StatusCode::BAD_REQUEST).await?;

    // malformed post composite id (no colon separator)
    invalid_get_request("/v0/graph/post/no-colon-here", StatusCode::BAD_REQUEST).await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_not_found() -> Result<()> {
    invalid_get_request(
        &format!("/v0/graph/user/{NON_EXISTING_USER_ID}"),
        StatusCode::NOT_FOUND,
    )
    .await?;
    // Well-formed labels/post ids that simply don't exist (malformed ones are 400s)
    invalid_get_request("/v0/graph/tag/zzznoexist", StatusCode::NOT_FOUND).await?;
    invalid_get_request(
        &format!("/v0/graph/post/{NON_EXISTING_USER_ID}:0032FNCGXE3R0"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}
