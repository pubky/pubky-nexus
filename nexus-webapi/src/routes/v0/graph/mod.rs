use crate::models::{BoundedLimit, GlobalPostId, PubkyId, TagLabel};
use crate::routes::v0::endpoints::{GRAPH_PATH_ROUTE, GRAPH_ROUTE};
use crate::routes::AppState;
use crate::routes::Path;
use crate::routes::Query;
use crate::{Error, Result};
use axum::routing::get;
use axum::{Json, Router};
use nexus_common::models::graph::{
    GraphDepth, GraphEdge, GraphEdgeType, GraphKinds, GraphNode, GraphView, PostGraphNode,
    TagGraphNode, UserGraphNode, GRAPH_DEFAULT_LIMIT, GRAPH_MAX_LIMIT,
};
use serde::Deserialize;
use tracing::debug;
use utoipa::OpenApi;

#[derive(Deserialize, Debug)]
pub struct GraphQuery {
    pub depth: Option<GraphDepth>,
    pub limit: Option<BoundedLimit<GRAPH_DEFAULT_LIMIT, GRAPH_MAX_LIMIT>>,
    pub kinds: Option<GraphKinds>,
}

/// `depth` only shapes the FOLLOWS traversal of a user-centered graph; on any
/// other center it is a malformed request (mirrors `resolve_tag_wot_depth`).
fn reject_depth(query: &GraphQuery) -> Result<()> {
    if query.depth.is_some() {
        return Err(Error::invalid_input(
            "`depth` only applies to user-centered graphs",
        ));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = GRAPH_ROUTE,
    description = "Typed neighborhood graph (nodes + edges) around a user, post, or tag, for interactive graph exploration",
    tag = "Graph",
    params(
        ("kind" = String, Path, description = "Center kind: 'user', 'post' or 'tag'"),
        ("id" = String, Path, description = "Center id: user pubky, '{author_pubky}:{post_id}' composite, or tag label"),
        ("depth" = Option<u8>, Query, description = "FOLLOWS hops around a user center (1-2, default 1); rejected on other kinds"),
        ("limit" = Option<BoundedLimit<30, 50>>, Query, description = "Cap on the primary neighbor class (1-50, default 30)"),
        ("kinds" = Option<String>, Query, description = "CSV of node kinds to include, e.g. 'user' or 'user,post,tag' (default all)")
    ),
    responses(
        (status = 200, description = "Neighborhood graph", body = GraphView),
        (status = 400, description = "Invalid parameters"),
        (status = 404, description = "Center entity not found"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn graph_handler(
    Path((kind, id)): Path<(String, String)>,
    Query(query): Query<GraphQuery>,
) -> Result<Json<GraphView>> {
    debug!("GET {GRAPH_ROUTE} kind:{kind} id:{id}");

    let limit = query
        .limit
        .as_ref()
        .map(|l| l.value())
        .unwrap_or(GRAPH_DEFAULT_LIMIT);
    let kinds = query.kinds.unwrap_or_default();

    match kind.as_str() {
        "user" => {
            let depth = query.depth.unwrap_or_default();
            let user_id = PubkyId::try_from(id.as_str())
                .map_err(|e| Error::invalid_input(format!("Invalid PubkyId: {e}")))?;
            match GraphView::get_by_user(&user_id, depth, limit, kinds).await? {
                Some(view) => Ok(Json(view)),
                None => Err(Error::user_not_found(user_id)),
            }
        }
        "post" => {
            reject_depth(&query)?;
            let global_id = GlobalPostId::try_from(id)?;
            // Validated above: the composite always has a ':' separator
            let (author_id, post_id) = global_id
                .0
                .split_once(':')
                .ok_or_else(|| Error::invalid_input("malformed post id"))?;
            match GraphView::get_by_post(author_id, post_id, limit, kinds).await? {
                Some(view) => Ok(Json(view)),
                None => Err(Error::PostNotFound {
                    author_id: author_id.to_string(),
                    post_id: post_id.to_string(),
                }),
            }
        }
        "tag" => {
            reject_depth(&query)?;
            let label = TagLabel::try_from(id)?;
            match GraphView::get_by_tag(&label, limit, kinds).await? {
                Some(view) => Ok(Json(view)),
                None => Err(Error::TagsNotFound {
                    reach: label.to_string(),
                }),
            }
        }
        other => Err(Error::invalid_input(format!(
            "unknown graph center kind: '{other}' (expected 'user', 'post' or 'tag')"
        ))),
    }
}

#[utoipa::path(
    get,
    path = GRAPH_PATH_ROUTE,
    description = "Shortest FOLLOWS path between two users (undirected, max 6 hops). Nodes are path-ordered from `from` to `to`.",
    tag = "Graph",
    params(
        ("from" = PubkyId, Path, description = "Starting user pubky"),
        ("to" = PubkyId, Path, description = "Destination user pubky")
    ),
    responses(
        (status = 200, description = "Path graph, nodes ordered along the path", body = GraphView),
        (status = 400, description = "Invalid pubky"),
        (status = 404, description = "Unknown user or no path within 6 hops"),
        (status = 429, description = "Rate limit exceeded", headers(("Retry-After" = u64, description = "Seconds until retry"))),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn graph_path_handler(
    Path((from, to)): Path<(PubkyId, PubkyId)>,
) -> Result<Json<GraphView>> {
    debug!("GET {GRAPH_PATH_ROUTE} from:{from} to:{to}");

    match GraphView::get_path(&from, &to).await? {
        Some(view) => Ok(Json(view)),
        None => {
            // Three distinct 404 causes deserve distinct bodies; the existence
            // probes only run on the error path.
            if !GraphView::user_exists(&from).await? {
                return Err(Error::user_not_found(from));
            }
            if !GraphView::user_exists(&to).await? {
                return Err(Error::user_not_found(to));
            }
            Err(Error::PathNotFound {
                from: from.to_string(),
                to: to.to_string(),
            })
        }
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        // The static /path segment must not be swallowed by the {kind} matcher
        .route(GRAPH_PATH_ROUTE, get(graph_path_handler))
        .route(GRAPH_ROUTE, get(graph_handler))
}

#[derive(OpenApi)]
#[openapi(
    paths(graph_handler, graph_path_handler),
    components(schemas(
        GraphView,
        GraphNode,
        UserGraphNode,
        PostGraphNode,
        TagGraphNode,
        GraphEdge,
        GraphEdgeType,
        PubkyId
    ))
)]
pub struct GraphApiDoc;
