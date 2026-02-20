use neo4rs::{query, Graph};
use std::fmt;
use std::sync::OnceLock;
use tracing::{debug, info};

use crate::db::graph::error::{GraphError, GraphResult};
use crate::db::setup::setup_graph;
use crate::db::Neo4JConfig;
use crate::types::DynError;

pub struct Neo4jConnector {
    pub graph: Graph,
}

impl Neo4jConnector {
    /// Initialize and register the global Neo4j connector and verify connectivity
    pub async fn init(neo4j_config: &Neo4JConfig) -> Result<(), DynError> {
        let neo4j_connector = Neo4jConnector::new_connection(
            &neo4j_config.uri,
            &neo4j_config.user,
            &neo4j_config.password,
        )
        .await?;

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
    async fn new_connection(uri: &str, user: &str, password: &str) -> GraphResult<Self> {
        let graph = Graph::new(uri, user, password).await?;
        let neo4j_connector = Neo4jConnector { graph };
        info!("Created Neo4j connector");

        Ok(neo4j_connector)
    }

    /// Perform a health-check PING over the Bolt protocol to the Neo4j server
    async fn ping(&self, neo4j_uri: &str) -> Result<(), DynError> {
        if let Err(neo4j_err) = self.graph.execute(query("RETURN 1")).await {
            return Err(format!("Failed to PING to Neo4j at {neo4j_uri}, {neo4j_err}").into());
        }

        info!("Bolt protocol health-check PING to Neo4j succeeded; server is responsive at {neo4j_uri}");
        Ok(())
    }
}

impl fmt::Debug for Neo4jConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Neo4jConnector")
            .field("graph", &"Graph instance")
            .finish()
    }
}

/// Helper to retrieve a Neo4j graph connection.
pub fn get_neo4j_graph() -> GraphResult<Graph> {
    NEO4J_CONNECTOR
        .get()
        .ok_or(GraphError::ConnectionNotInitialized)
        .map(|neo4j_connector| neo4j_connector.graph.clone())
}

pub static NEO4J_CONNECTOR: OnceLock<Neo4jConnector> = OnceLock::new();
