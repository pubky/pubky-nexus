use crate::db::graph::Query;
use std::fmt;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tracing::{debug, info};

use crate::db::graph::error::{GraphError, GraphResult};
use crate::db::graph::{Graph, GraphOps, InstrumentedGraph};
use crate::db::setup::setup_graph;
use crate::db::Neo4JConfig;
use crate::types::DynError;

pub struct Neo4jConnector {
    graph: Arc<dyn GraphOps>,
}

impl Neo4jConnector {
    /// Initialize and register the global Neo4j connector and verify connectivity
    pub async fn init(neo4j_config: &Neo4JConfig) -> Result<(), DynError> {
        let neo4j_connector = Neo4jConnector::new_connection(neo4j_config).await?;

        neo4j_connector.ping(&neo4j_config.uri).await?;

        match NEO4J_CONNECTOR.set(neo4j_connector) {
            Err(e) => debug!("Neo4jConnector was already set: {:?}", e),
            Ok(()) => info!("Neo4jConnector successfully set up on {}", neo4j_config.uri),
        }

        // Set Neo4J graph data constraints
        setup_graph().await?;
        Ok(())
    }

    /// Create and return a new connector after defining a database connection
    async fn new_connection(config: &Neo4JConfig) -> GraphResult<Self> {
        let neo4j_graph = neo4rs::Graph::new(&config.uri, &config.user, &config.password).await?;
        let graph = Graph::new(neo4j_graph);

        // Always wrap with InstrumentedGraph to collect OpenTelemetry metrics.
        // slow_query_threshold is None when slow-query logging is disabled.
        let graph: Arc<dyn GraphOps> = Arc::new(
            InstrumentedGraph::new(graph)
                .with_slow_query_threshold(
                    config
                        .slow_query_logging_threshold_ms
                        .map(Duration::from_millis),
                )
                .with_log_cypher(config.slow_query_logging_include_cypher),
        );

        info!(
            slow_query_logging_threshold_ms = ?config.slow_query_logging_threshold_ms,
            slow_query_logging_include_cypher = config.slow_query_logging_include_cypher,
            "Created Neo4j connector"
        );
        Ok(Neo4jConnector { graph })
    }

    /// Perform a health-check PING over the Bolt protocol to the Neo4j server
    async fn ping(&self, neo4j_uri: &str) -> Result<(), DynError> {
        if let Err(neo4j_err) = self.graph.run(Query::new("ping", "RETURN 1")).await {
            return Err(format!("Failed to PING to Neo4j at {neo4j_uri}, {neo4j_err}").into());
        }

        info!("Bolt protocol health-check PING to Neo4j succeeded; server is responsive at {neo4j_uri}");
        Ok(())
    }
}

impl fmt::Debug for Neo4jConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Neo4jConnector")
            .field("graph", &"GraphOps instance")
            .finish()
    }
}

/// Helper to retrieve a Neo4j graph connection.
pub fn get_neo4j_graph() -> GraphResult<Arc<dyn GraphOps>> {
    NEO4J_CONNECTOR
        .get()
        .ok_or(GraphError::ConnectionNotInitialized)
        .map(|neo4j_connector| neo4j_connector.graph.clone())
}

pub static NEO4J_CONNECTOR: OnceLock<Neo4jConnector> = OnceLock::new();
