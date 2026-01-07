use neo4rs::{query, Graph};
use once_cell::sync::OnceCell;
use std::fmt;
use std::sync::Arc;
use tracing::{debug, info};

use crate::db::setup::setup_graph;
use crate::db::Neo4JConfig;
use crate::types::DynError;

pub struct Neo4jConnector {
    pub graph: OnceCell<Arc<Graph>>,
}

impl Default for Neo4jConnector {
    fn default() -> Self {
        Self {
            graph: OnceCell::new(),
        }
    }
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
    async fn new_connection(uri: &str, user: &str, password: &str) -> Result<Self, DynError> {
        let neo4j_connector = Neo4jConnector::default();
        match neo4j_connector.connect(uri, user, password).await {
            Ok(_) => info!("Created Neo4j connector"),
            Err(e) => return Err(format!("Could not create Neo4J connector: {e}").into()),
        }
        Ok(neo4j_connector)
    }

    /// Dewfine a connection to the Neo4j database and store the graph instance
    async fn connect(
        &self,
        uri: &str,
        user: &str,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let graph = Graph::new(uri, user, password).await?;
        self.graph
            .set(Arc::new(graph))
            .map_err(|_| "Failed to set graph instance")?;
        Ok(())
    }

    /// Perform a health-check PING over the Bolt protocol to the Neo4j server
    async fn ping(&self, neo4j_uri: &str) -> Result<(), DynError> {
        let graph = self.graph.get().ok_or("Neo4jConnector not initialized")?;
        match graph.execute(query("RETURN 1")).await {
            Ok(_) => info!(
                "Bolt protocol health-check PING to Neo4j succeeded; server is responsive at {}",
                neo4j_uri
            ),
            Err(neo4j_err) => {
                return Err(format!("Failed to PING to Neo4j at {neo4j_uri}, {neo4j_err}").into())
            }
        };
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
pub fn get_neo4j_graph() -> Result<Arc<Graph>, &'static str> {
    let neo4j_connector = NEO4J_CONNECTOR
        .get()
        .ok_or("Neo4jConnector not initialized")?;
    let graph = neo4j_connector
        .graph
        .get()
        .ok_or("Not connected to Neo4j")?;
    Ok(graph.clone())
}

pub static NEO4J_CONNECTOR: OnceCell<Neo4jConnector> = OnceCell::new();
