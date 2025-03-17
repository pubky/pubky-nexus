use neo4rs::Graph;
use once_cell::sync::OnceCell;
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Neo4jConnector {
    pub graph: OnceCell<Arc<Mutex<Graph>>>,
}

impl Default for Neo4jConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl Neo4jConnector {
    pub fn new() -> Self {
        Self {
            graph: OnceCell::new(),
        }
    }

    pub async fn connect(
        &self,
        uri: &str,
        user: &str,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let graph = Graph::new(uri, user, password).await?;
        self.graph
            .set(Arc::new(Mutex::new(graph)))
            .map_err(|_| "Failed to set graph instance")?;
        Ok(())
    }

    pub async fn new_connection(
        uri: &str,
        user: &str,
        password: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let neo4j_connector = Neo4jConnector::new();
        neo4j_connector.connect(uri, user, password).await?;
        Ok(neo4j_connector)
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
pub fn get_neo4j_graph() -> Result<Arc<Mutex<Graph>>, &'static str> {
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
