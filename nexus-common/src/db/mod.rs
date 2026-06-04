mod config;
mod connectors;
pub mod graph;
pub mod kv;
pub mod reindex;

pub use config::*;
pub use connectors::{
    get_neo4j_graph, get_redis_conn, Neo4jConnector, PubkyClientError, PubkyConnector,
    RedisConnector, NEO4J_CONNECTOR, REDIS_CONNECTOR,
};
pub use graph::error::{GraphError, GraphResult};
pub use graph::exec::*;
pub use graph::queries;
pub use graph::setup;
pub use graph::GraphOps;
pub use kv::RedisOps;

/// Fetch the raw bytes for a `pubky://` URI via the pubky SDK.
///
/// Shared by the core social watcher and domain plugin handlers so both use
/// an identical fetch code path.  Returns an error (never silently drops) so
/// callers can propagate failures to the retry queue.
pub async fn fetch_blob(uri: &str) -> Result<Vec<u8>, crate::models::event::EventProcessorError> {
    let pubky = PubkyConnector::get()?;
    let response = pubky
        .public_storage()
        .get(uri)
        .await
        .map_err(|e| crate::models::event::EventProcessorError::client_error(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unable to read body>".to_string());
        return Err(crate::models::event::EventProcessorError::client_error(
            format!("Fetch resource failed {uri}: HTTP {status} - {body}"),
        ));
    }

    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| crate::models::event::EventProcessorError::client_error(e.to_string()))
}
