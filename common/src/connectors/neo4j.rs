pub use neo4rs::{query, Graph, Node};
use once_cell::sync::OnceCell;
use std::fmt;

pub struct Neo4jConnector {
    pub graph: OnceCell<Graph>,
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
            .set(graph)
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

pub static GLOBAL_NEO4J_CONNECTOR: OnceCell<Neo4jConnector> = OnceCell::new();
