use crate::utils::{get_request, invalid_get_request};
use anyhow::Result;
use axum::http::StatusCode;
use serde_json::Value;

const ALDERT: &str = "4snwyct86m383rsduhw5xgcxpw7c63j3pq8x4ycqikxgik8y64ro";
/// Direct friend of Aldert (mutual follow)
const FRIEND: &str = "y4euc58gnmxun9wo87gwmanu6kztt9pgw1zz1yp1azp7trrsjamy";
/// Fixture user exactly 4 FOLLOWS hops away from Aldert (undirected)
const DISTANT: &str = "ppc7wt1twskz95f58s9gxg67fd87fwurix4yzscsh8rh6xaxjy1y";
/// Fixture user with no FOLLOWS path to Aldert within 6 hops
const UNREACHABLE: &str = "18a49r4bu7zgu9p8wjcfhcs7q97q7u898ogs8m7k6375y8ixp17o";
const NON_EXISTING_USER_ID: &str = "qca6wzjg4okp6g1hwr9g8hmx1po1jpoirjfau9ejsws1qz3t7iiy";

fn node_ids_in_order(res: &Value) -> Vec<String> {
    res["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .map(|n| n["id"].as_str().expect("node id").to_string())
        .collect()
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_path_direct() -> Result<()> {
    let res = get_request(&format!("/v0/graph/path/{ALDERT}/{FRIEND}")).await?;

    let ids = node_ids_in_order(&res);
    assert_eq!(ids.len(), 2, "direct connection is a two-node path");
    // Nodes are path-ordered: from first, to last
    assert_eq!(ids.first().unwrap(), &format!("user:{ALDERT}"));
    assert_eq!(ids.last().unwrap(), &format!("user:{FRIEND}"));

    let edges = res["edges"].as_array().unwrap();
    assert!(!edges.is_empty());
    for edge in edges {
        assert_eq!(edge["type"], "FOLLOWS");
        assert!(
            edge["indexed_at"].is_number(),
            "path edges carry timestamps"
        );
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_path_distant() -> Result<()> {
    let res = get_request(&format!("/v0/graph/path/{ALDERT}/{DISTANT}")).await?;

    let ids = node_ids_in_order(&res);
    assert_eq!(ids.len(), 5, "4 hops means 5 path-ordered nodes");
    assert_eq!(ids.first().unwrap(), &format!("user:{ALDERT}"));
    assert_eq!(ids.last().unwrap(), &format!("user:{DISTANT}"));

    // One traversed relationship per hop, endpoints all within the path
    let edges = res["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 4);
    for edge in edges {
        for key in ["source", "target"] {
            let endpoint = edge[key].as_str().unwrap();
            assert!(ids.iter().any(|id| id == endpoint), "dangling {endpoint}");
        }
    }

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_path_same_user() -> Result<()> {
    let res = get_request(&format!("/v0/graph/path/{ALDERT}/{ALDERT}")).await?;

    assert_eq!(node_ids_in_order(&res), vec![format!("user:{ALDERT}")]);
    assert_eq!(res["edges"].as_array().unwrap().len(), 0);

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_path_not_found() -> Result<()> {
    // Unknown endpoint user, on either side
    invalid_get_request(
        &format!("/v0/graph/path/{ALDERT}/{NON_EXISTING_USER_ID}"),
        StatusCode::NOT_FOUND,
    )
    .await?;
    invalid_get_request(
        &format!("/v0/graph/path/{NON_EXISTING_USER_ID}/{ALDERT}"),
        StatusCode::NOT_FOUND,
    )
    .await?;
    // Both users exist but no FOLLOWS path within the hop cap
    invalid_get_request(
        &format!("/v0/graph/path/{ALDERT}/{UNREACHABLE}"),
        StatusCode::NOT_FOUND,
    )
    .await?;

    Ok(())
}

#[tokio_shared_rt::test(shared)]
async fn test_graph_path_invalid() -> Result<()> {
    invalid_get_request(
        &format!("/v0/graph/path/{ALDERT}/not-a-pubky"),
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(())
}
